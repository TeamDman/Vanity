use crate::paths::APP_HOME;
use chrono::FixedOffset;
use chrono::TimeZone;
use chrono::Utc;
use eyre::Context;
use eyre::Result;
use eyre::bail;
use git2::Oid;
use git2::Repository;
use git2::Signature;
use git2::Sort;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

const MARKER_PREFIX: &str = "Vanity-Source-Commit: ";
const EXPECTED_VANITY_REMOTE: &str = "https://github.com/TeamDman/Vanity";
const CONFIG_FILENAME: &str = "vanity-config.txt";

#[derive(Clone, Debug, Default)]
pub struct VanityConfig {
    pub this_repo: Option<PathBuf>,
    pub read_repos: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct SyncSummary {
    pub total_source_commits: usize,
    pub existing_markers: usize,
    pub created: usize,
}

#[derive(Clone, Debug)]
struct SourceCommit {
    sha: String,
    source_repo_hint: String,
    source_web_base_url: Option<String>,
    author_date_seconds: i64,
    author_offset_minutes: i32,
    subject: String,
}

impl VanityConfig {
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created or config file parsing fails.
    pub fn load() -> Result<Self> {
        APP_HOME.ensure_dir()?;
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .wrap_err_with(|| format!("Failed to read config file {}", path.display()))?;

        let mut config = Self::default();
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("this=") {
                config.this_repo = Some(PathBuf::from(value));
                continue;
            }
            if let Some(value) = line.strip_prefix("read=") {
                config.read_repos.push(PathBuf::from(value));
            }
        }
        Ok(config)
    }

    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created or config file cannot be written.
    pub fn save(&self) -> Result<()> {
        APP_HOME.ensure_dir()?;
        let path = config_path();
        let mut lines = Vec::new();
        if let Some(path) = &self.this_repo {
            lines.push(format!("this={}", path.display()));
        }
        for read_repo in &self.read_repos {
            lines.push(format!("read={}", read_repo.display()));
        }
        std::fs::write(&path, lines.join("\n"))
            .wrap_err_with(|| format!("Failed to write config file {}", path.display()))
    }

    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized or is not a git repository.
    pub fn set_this_repo(&mut self, path: &Path) -> Result<PathBuf> {
        let canonical = canonicalize_git_repo(path)?;
        self.this_repo = Some(canonical.clone());
        Ok(canonical)
    }

    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized or is not a git repository.
    pub fn add_read_repo(&mut self, path: &Path) -> Result<PathBuf> {
        let canonical = canonicalize_git_repo(path)?;
        if !self
            .read_repos
            .iter()
            .any(|existing| existing == &canonical)
        {
            self.read_repos.push(canonical.clone());
        }
        Ok(canonical)
    }
}

/// # Errors
///
/// Returns an error if repositories cannot be opened or commit creation fails.
pub fn sync(
    config: &VanityConfig,
    dry_run: bool,
    allow_non_vanity_target: bool,
    limit: Option<usize>,
) -> Result<SyncSummary> {
    let Some(this_repo_path) = &config.this_repo else {
        bail!("this-repo is not configured. Run: this-repo set <path>");
    };
    if config.read_repos.is_empty() {
        bail!("read-repo list is empty. Run: read-repo add <path>");
    }

    let this_repo = Repository::open(this_repo_path)
        .wrap_err_with(|| format!("Failed to open this-repo at {}", this_repo_path.display()))?;

    if !dry_run {
        assert_vanity_target_repo(&this_repo, allow_non_vanity_target)?;
    }

    let existing_markers = existing_mirrored_shas(&this_repo)?;
    let source_commits = gather_source_commits(&config.read_repos)?;

    let mut pending: Vec<SourceCommit> = source_commits
        .iter()
        .filter(|commit| !existing_markers.contains(&commit.sha))
        .cloned()
        .collect();

    if let Some(limit) = limit {
        pending.truncate(limit);
    }

    let progress = progress_bar(pending.len() as u64, "Creating vanity commits");
    for commit in &pending {
        let message = build_commit_message(commit);
        if !dry_run {
            create_empty_commit(&this_repo, &message, commit)?;
        }
        progress.inc(1);
    }
    progress.finish_and_clear();

    Ok(SyncSummary {
        total_source_commits: source_commits.len(),
        existing_markers: existing_markers.len(),
        created: pending.len(),
    })
}

fn config_path() -> PathBuf {
    APP_HOME.file_path(CONFIG_FILENAME)
}

fn canonicalize_git_repo(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .wrap_err_with(|| format!("Failed to canonicalize path {}", path.display()))?;
    Repository::open(&canonical)
        .wrap_err_with(|| format!("Path is not a git repository: {}", canonical.display()))?;
    Ok(canonical)
}

fn existing_mirrored_shas(repo: &Repository) -> Result<HashSet<String>> {
    let mut result = HashSet::new();
    let Ok(head) = repo.head() else {
        return Ok(result);
    };
    let Some(head_oid) = head.target() else {
        return Ok(result);
    };

    let mut walk = repo.revwalk()?;
    walk.push(head_oid)?;
    for oid in walk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let Some(message) = commit.message() else {
            continue;
        };
        for line in message.lines() {
            if let Some(sha) = line.strip_prefix(MARKER_PREFIX) {
                let normalized = sha.trim().to_lowercase();
                if normalized.len() == 40 && normalized.chars().all(|c| c.is_ascii_hexdigit()) {
                    result.insert(normalized);
                }
            }
        }
    }
    Ok(result)
}

fn gather_source_commits(read_repos: &[PathBuf]) -> Result<Vec<SourceCommit>> {
    let per_repo_results: Vec<Result<Vec<SourceCommit>>> = read_repos
        .par_iter()
        .map(|repo_path| gather_source_commits_for_repo(repo_path.as_path()))
        .collect();

    let mut all = Vec::new();
    let mut seen_shas: HashSet<String> = HashSet::new();

    for per_repo in per_repo_results {
        for commit in per_repo? {
            if seen_shas.insert(commit.sha.clone()) {
                all.push(commit);
            }
        }
    }

    all.sort_by(|left, right| {
        left.author_date_seconds
            .cmp(&right.author_date_seconds)
            .then_with(|| left.sha.cmp(&right.sha))
    });

    Ok(all)
}

fn gather_source_commits_for_repo(repo_path: &Path) -> Result<Vec<SourceCommit>> {
    let repo = Repository::open(repo_path)
        .wrap_err_with(|| format!("Failed to open read-repo at {}", repo_path.display()))?;
    let source_hint = repo_origin_url(&repo).unwrap_or_else(|| repo_path.display().to_string());
    let source_web = derive_github_web_base(&source_hint);

    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    for reference in repo.references()? {
        let reference = reference?;
        if let Some(target) = reference.target() {
            let _ = walk.push(target);
        }
    }

    let mut commits = Vec::new();
    let mut seen_oids = HashSet::new();
    for oid in walk {
        let oid = oid?;
        if !seen_oids.insert(oid) {
            continue;
        }
        let commit = repo.find_commit(oid)?;
        let author_time = commit.author().when();
        commits.push(SourceCommit {
            sha: oid.to_string(),
            source_repo_hint: source_hint.clone(),
            source_web_base_url: source_web.clone(),
            author_date_seconds: author_time.seconds(),
            author_offset_minutes: author_time.offset_minutes(),
            subject: commit.summary().unwrap_or("").to_owned(),
        });
    }

    Ok(commits)
}

fn repo_origin_url(repo: &Repository) -> Option<String> {
    repo.find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().map(ToOwned::to_owned))
}

fn derive_github_web_base(source_hint: &str) -> Option<String> {
    let hint = source_hint.trim();
    if let Some(path) = hint.strip_prefix("git@github.com:") {
        return Some(format!(
            "https://github.com/{}",
            path.trim_end_matches(".git")
        ));
    }
    if hint.starts_with("https://github.com/") || hint.starts_with("http://github.com/") {
        return Some(
            hint.trim_end_matches(".git")
                .trim_end_matches('/')
                .to_owned(),
        );
    }
    None
}

fn source_commit_url(commit: &SourceCommit) -> Option<String> {
    commit
        .source_web_base_url
        .as_ref()
        .map(|base| format!("{}/commit/{}", base.trim_end_matches('/'), commit.sha))
}

fn format_source_date(seconds: i64, offset_minutes: i32) -> String {
    let offset_seconds = offset_minutes.saturating_mul(60);
    let Some(offset) = FixedOffset::east_opt(offset_seconds).or_else(|| FixedOffset::east_opt(0))
    else {
        return seconds.to_string();
    };
    if let Some(datetime_utc) = Utc.timestamp_opt(seconds, 0).single() {
        datetime_utc.with_timezone(&offset).to_rfc3339()
    } else {
        seconds.to_string()
    }
}

fn build_commit_message(commit: &SourceCommit) -> String {
    let mut lines = vec![
        format!("Vanity mirror: {}", &commit.sha[..12]),
        String::new(),
        format!("Source-Repo: {}", commit.source_repo_hint),
        format!("{MARKER_PREFIX}{}", commit.sha),
    ];

    if let Some(url) = source_commit_url(commit) {
        lines.push(format!("Source-Commit-URL: {url}"));
    }
    lines.push(format!(
        "Source-Date: {}",
        format_source_date(commit.author_date_seconds, commit.author_offset_minutes)
    ));
    lines.push(format!("Source-Subject: {}", commit.subject));
    lines.join("\n")
}

fn create_empty_commit(repo: &Repository, message: &str, source: &SourceCommit) -> Result<Oid> {
    let head_commit = repo
        .head()
        .wrap_err("this-repo must have at least one commit")?
        .peel_to_commit()
        .wrap_err("Failed to resolve this-repo HEAD commit")?;
    let tree = head_commit.tree()?;

    let (name, email) = resolve_repo_identity(repo);

    let signature_time = git2::Time::new(source.author_date_seconds, source.author_offset_minutes);
    let author = Signature::new(&name, &email, &signature_time)?;
    let committer = Signature::new(&name, &email, &signature_time)?;

    repo.commit(
        Some("HEAD"),
        &author,
        &committer,
        message,
        &tree,
        &[&head_commit],
    )
    .wrap_err("Failed to create empty vanity commit")
}

fn resolve_repo_identity(repo: &Repository) -> (String, String) {
    let Ok(config) = repo.config() else {
        return ("Vanity".to_owned(), "vanity@example.invalid".to_owned());
    };
    let name = config
        .get_string("user.name")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Vanity".to_owned());
    let email = config
        .get_string("user.email")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "vanity@example.invalid".to_owned());
    (name, email)
}

fn normalize_remote_url(url: &str) -> String {
    let mut normalized = url.trim().to_lowercase();
    if let Some(path) = normalized.strip_prefix("git@github.com:") {
        normalized = format!("https://github.com/{path}");
    }
    if let Some(stripped) = normalized.strip_suffix(".git") {
        normalized = stripped.to_owned();
    }
    normalized.trim_end_matches('/').to_owned()
}

fn assert_vanity_target_repo(repo: &Repository, allow_non_vanity_target: bool) -> Result<()> {
    if allow_non_vanity_target {
        return Ok(());
    }
    let origin = repo
        .find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().map(ToOwned::to_owned))
        .ok_or_else(|| eyre::eyre!("Missing origin remote in this-repo"))?;

    if normalize_remote_url(&origin) != normalize_remote_url(EXPECTED_VANITY_REMOTE) {
        bail!(
            "Mutation blocked: target repo origin does not match {EXPECTED_VANITY_REMOTE}. Found: {origin}"
        );
    }
    Ok(())
}

fn progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    if let Ok(style) = ProgressStyle::with_template(
        "{msg}: {wide_bar} {pos}/{len} [{elapsed_precise}<{eta_precise}]",
    ) {
        pb.set_style(style);
    }
    pb.set_message(message.to_owned());
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_empty_commit_uses_fallback_identity_when_repo_signature_missing() {
        let repo_dir = std::env::temp_dir().join(format!(
            "vanity-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&repo_dir).expect("temp repo directory should be created");
        let repo = Repository::init(&repo_dir).expect("repo should initialize");

        let base_signature = Signature::new(
            "Initial User",
            "initial@example.invalid",
            &git2::Time::new(1_700_000_000, 0),
        )
        .expect("base signature should be valid");
        let tree_id = repo
            .index()
            .and_then(|mut index| index.write_tree())
            .expect("empty tree should be writable");
        {
            let tree = repo.find_tree(tree_id).expect("tree should exist");
            repo.commit(Some("HEAD"), &base_signature, &base_signature, "initial", &tree, &[])
                .expect("initial commit should be created");
        }

        let source = SourceCommit {
            sha: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            source_repo_hint: "source".to_owned(),
            source_web_base_url: None,
            author_date_seconds: 1_700_000_001,
            author_offset_minutes: 0,
            subject: "subject".to_owned(),
        };
        let oid =
            create_empty_commit(&repo, "test vanity message", &source).expect("commit should work");
        {
            let commit = repo.find_commit(oid).expect("new commit should exist");
            assert_eq!(commit.author().name(), Some("Vanity"));
            assert_eq!(commit.author().email(), Some("vanity@example.invalid"));
        }

        drop(repo);
        let _ = std::fs::remove_dir_all(&repo_dir);
    }
}
