use crate::cli::ToArgs;
use crate::vanity::VanityConfig;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;
use std::ffi::OsString;

/// Synchronize vanity commits from configured source repositories.
///
/// What this command reads:
/// - `this-repo` from config (target repository where vanity commits are written)
/// - all repositories listed in `read-repo list` (source commit history)
///
/// What this command writes:
/// - only to `this-repo` (unless `--dry_run`)
/// - creates empty commits whose messages contain source metadata, including
///   `Vanity-Source-Commit: <sha>` and source commit URL when derivable
///
/// Idempotency:
/// - if a source sha marker already exists in current `this-repo` HEAD history,
///   that source commit is skipped
/// - command is safe to run repeatedly
///
/// Important behavior:
/// - does not rewrite existing commits
/// - does not modify read-repo history
/// - normal mode enforces origin safety (must match TeamDman/Vanity)
#[derive(Facet, Arbitrary, Debug, PartialEq, Default)]
pub struct SyncArgs {
    /// Preview mode. Computes pending vanity commits but does not create any commits.
    #[facet(args::named, default)]
    pub dry_run: bool,

    /// Optional cap on number of new vanity commits to create in this run.
    ///
    /// Useful for smoke tests or incremental backfills.
    #[facet(args::named)]
    pub limit: Option<usize>,

    /// Bypass origin safety check that normally restricts mutation to TeamDman/Vanity.
    ///
    /// Use only for intentional testing in another repo.
    #[facet(args::named, default)]
    pub allow_non_vanity_target: bool,
}

impl SyncArgs {
    /// # Errors
    ///
    /// Returns an error if config is invalid or synchronization fails.
    pub async fn invoke(self) -> Result<()> {
        let summary = tokio::task::spawn_blocking(move || -> Result<_> {
            let config = VanityConfig::load()?;
            crate::vanity::sync(
                &config,
                self.dry_run,
                self.allow_non_vanity_target,
                self.limit,
            )
        })
        .await
        .map_err(|err| eyre::eyre!("sync task failed: {err}"))??;
        let mode = if self.dry_run { "DRY RUN" } else { "APPLY" };
        println!(
            "[{mode}] source_commits={} mirrored_markers={} newly_created={}",
            summary.total_source_commits, summary.existing_markers, summary.created
        );
        Ok(())
    }
}

impl ToArgs for SyncArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        if self.dry_run {
            args.push("--dry-run".into());
        }
        if let Some(limit) = self.limit {
            args.push("--limit".into());
            args.push(limit.to_string().into());
        }
        if self.allow_non_vanity_target {
            args.push("--allow-non-vanity-target".into());
        }
        args
    }
}
