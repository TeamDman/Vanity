#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from tqdm import tqdm


MARKER_PREFIX = "Vanity-Source-Commit: "
EXPECTED_VANITY_REMOTE = "https://github.com/TeamDman/Vanity"


@dataclass(frozen=True)
class SourceCommit:
    sha: str
    author_name: str
    author_email: str
    author_date_iso: str
    subject: str


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


def run_git(args: list[str], cwd: Path, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=str(cwd),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed in {cwd}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )
    return completed.stdout


def ensure_git_repo(path: Path) -> None:
    run_git(["rev-parse", "--is-inside-work-tree"], cwd=path)


def normalize_remote_url(url: str) -> str:
    normalized = url.strip()
    if normalized.startswith("git@github.com:"):
        normalized = "https://github.com/" + normalized.removeprefix("git@github.com:")
    if normalized.endswith(".git"):
        normalized = normalized[:-4]
    return normalized.rstrip("/").lower()


def get_origin_url(repo: Path) -> str | None:
    try:
        return run_git(["remote", "get-url", "origin"], cwd=repo).strip()
    except RuntimeError:
        return None


def assert_vanity_target_repo(vanity_repo: Path, allow_non_vanity_target: bool) -> None:
    if allow_non_vanity_target:
        return

    origin_url = get_origin_url(vanity_repo)
    if not origin_url:
        raise RuntimeError(
            "Mutation blocked: missing origin remote on vanity target repo. "
            "Use --allow-non-vanity-target to bypass intentionally."
        )

    if normalize_remote_url(origin_url) != normalize_remote_url(EXPECTED_VANITY_REMOTE):
        raise RuntimeError(
            "Mutation blocked: target repo origin does not match TeamDman/Vanity. "
            f"Found origin={origin_url!r}, expected={EXPECTED_VANITY_REMOTE!r}. "
            "Use --allow-non-vanity-target to bypass intentionally."
        )


def existing_mirrored_shas(vanity_repo: Path) -> set[str]:
    out = run_git(
        [
            "log",
            "--all",
            "--extended-regexp",
            "--grep",
            rf"^{re.escape(MARKER_PREFIX)}[0-9a-f]{{40}}$",
            "--format=%B%x00",
        ],
        cwd=vanity_repo,
    )
    shas: set[str] = set()
    for message in out.split("\x00"):
        for line in message.splitlines():
            if line.startswith(MARKER_PREFIX):
                shas.add(line.removeprefix(MARKER_PREFIX).strip().lower())
    return shas


def load_authored_commits(
    source_repo: Path,
    author_name: str | None,
    author_email: str | None,
) -> list[SourceCommit]:
    if not author_name and not author_email:
        raise ValueError("At least one of --source-author-name or --source-author-email is required.")

    raw = run_git(
        [
            "log",
            "--all",
            "--reverse",
            "--pretty=format:%H%x1f%an%x1f%ae%x1f%aI%x1f%s%x1e",
        ],
        cwd=source_repo,
    )

    expected_name = author_name.lower() if author_name else None
    expected_email = author_email.lower() if author_email else None

    commits: list[SourceCommit] = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        parts = record.split("\x1f")
        if len(parts) != 5:
            continue
        sha, name, email, date_iso, subject = parts
        name_matches = expected_name is None or name.lower() == expected_name
        email_matches = expected_email is None or email.lower() == expected_email
        if name_matches and email_matches:
            commits.append(
                SourceCommit(
                    sha=sha.lower(),
                    author_name=name,
                    author_email=email,
                    author_date_iso=date_iso,
                    subject=subject,
                )
            )
    return commits


def fetch_source_repo_into_temp(source_url: str) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    temp_dir = tempfile.TemporaryDirectory(prefix="vanity-source-")
    repo_dir = Path(temp_dir.name)
    run_git(["init"], cwd=repo_dir)
    run_git(["remote", "add", "origin", source_url], cwd=repo_dir)
    run_git(
        [
            "fetch",
            "--quiet",
            "--filter=blob:none",
            "--tags",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
        cwd=repo_dir,
    )
    return temp_dir, repo_dir


def derive_github_web_base(source_repo_hint: str) -> str | None:
    hint = source_repo_hint.strip()
    if not hint:
        return None

    if hint.startswith("git@github.com:"):
        repo_path = hint.removeprefix("git@github.com:")
        if repo_path.endswith(".git"):
            repo_path = repo_path[:-4]
        return f"https://github.com/{repo_path}"

    if hint.startswith("https://github.com/") or hint.startswith("http://github.com/"):
        base = hint[:-4] if hint.endswith(".git") else hint
        return base.rstrip("/")

    return None


def source_commit_url(source_repo_hint: str, source_sha: str, source_web_base_url: str | None) -> str | None:
    base = source_web_base_url or derive_github_web_base(source_repo_hint)
    if not base:
        return None
    return f"{base.rstrip('/')}/commit/{source_sha}"


def build_commit_message(
    source_repo_hint: str,
    source_commit: SourceCommit,
    source_web_base_url: str | None,
) -> str:
    lines = [
        f"Vanity mirror: {source_commit.sha[:12]}",
        "",
        f"Source-Repo: {source_repo_hint}",
        f"{MARKER_PREFIX}{source_commit.sha}",
    ]

    commit_url = source_commit_url(source_repo_hint, source_commit.sha, source_web_base_url)
    if commit_url:
        lines.append(f"Source-Commit-URL: {commit_url}")

    lines.extend(
        [
            f"Source-Author: {source_commit.author_name} <{source_commit.author_email}>",
            f"Source-Date: {source_commit.author_date_iso}",
            f"Source-Subject: {source_commit.subject}",
        ]
    )
    return "\n".join(lines)


def parse_prefixed_value(message: str, prefix: str) -> str | None:
    for line in message.splitlines():
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    return None


def parse_author_line(author_line: str) -> tuple[str, str] | None:
    match = re.fullmatch(r"(.+)\s+<([^<>]+)>", author_line.strip())
    if not match:
        return None
    return match.group(1), match.group(2)


def recompute_message_from_existing(message: str, source_web_base_url: str | None) -> str:
    source_sha = parse_prefixed_value(message, MARKER_PREFIX)
    if not source_sha:
        return message

    source_repo_hint = parse_prefixed_value(message, "Source-Repo: ") or ""
    source_author_line = parse_prefixed_value(message, "Source-Author: ")
    source_date = parse_prefixed_value(message, "Source-Date: ")
    source_subject = parse_prefixed_value(message, "Source-Subject: ")

    if not source_author_line or not source_date or source_subject is None:
        return message

    parsed_author = parse_author_line(source_author_line)
    if not parsed_author:
        return message

    author_name, author_email = parsed_author
    source_commit = SourceCommit(
        sha=source_sha.lower(),
        author_name=author_name,
        author_email=author_email,
        author_date_iso=source_date,
        subject=source_subject,
    )
    return build_commit_message(
        source_repo_hint=source_repo_hint,
        source_commit=source_commit,
        source_web_base_url=source_web_base_url,
    )


def message_filter_mode(source_web_base_url: str | None) -> int:
    original = sys.stdin.read()
    rewritten = recompute_message_from_existing(original, source_web_base_url)
    sys.stdout.write(rewritten)
    return 0


def rewrite_history_messages(
    vanity_repo: Path,
    source_web_base_url: str | None,
    rewrite_range: str,
    allow_non_vanity_target: bool,
) -> int:
    ensure_git_repo(vanity_repo)
    assert_vanity_target_repo(vanity_repo, allow_non_vanity_target)
    status = run_git(["status", "--porcelain"], cwd=vanity_repo)
    if status.strip():
        raise RuntimeError("Working tree must be clean before history rewrite.")

    script_path = str(Path(__file__).resolve())
    message_filter_cmd = (
        f"{shell_quote(sys.executable)} {shell_quote(script_path)} --message-filter"
    )
    if source_web_base_url:
        message_filter_cmd += f" --source-web-base-url {shell_quote(source_web_base_url)}"

    command = [
        "git",
        "filter-branch",
        "--force",
        "--msg-filter",
        message_filter_cmd,
        "--",
        rewrite_range,
    ]

    process = subprocess.Popen(
        command,
        cwd=str(vanity_repo),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )

    progress: tqdm | None = None
    rewrite_pattern = re.compile(r"Rewrite\s+[0-9a-f]{40}\s+\((\d+)/(\d+)\)", re.IGNORECASE)

    assert process.stdout is not None
    for line in process.stdout:
        match = rewrite_pattern.search(line)
        if match:
            current = int(match.group(1))
            total = int(match.group(2))
            if progress is None:
                progress = tqdm(total=total, desc="Rewriting vanity history", unit="commit")
            if progress.total != total:
                progress.total = total
            if current > progress.n:
                progress.update(current - progress.n)
            continue
        print(line, end="")

    exit_code = process.wait()
    if progress is not None:
        progress.close()
    if exit_code != 0:
        raise RuntimeError(f"git filter-branch failed with exit code {exit_code}")
    return 0


def create_empty_commit(
    vanity_repo: Path,
    message: str,
    date_iso: str,
    vanity_author_name: str | None,
    vanity_author_email: str | None,
    dry_run: bool,
) -> None:
    env = os.environ.copy()
    env["GIT_AUTHOR_DATE"] = date_iso
    env["GIT_COMMITTER_DATE"] = date_iso
    if vanity_author_name:
        env["GIT_AUTHOR_NAME"] = vanity_author_name
        env["GIT_COMMITTER_NAME"] = vanity_author_name
    if vanity_author_email:
        env["GIT_AUTHOR_EMAIL"] = vanity_author_email
        env["GIT_COMMITTER_EMAIL"] = vanity_author_email

    if dry_run:
        return

    run_git(["commit", "--allow-empty", "-m", message], cwd=vanity_repo, env=env)


def sync_vanity_commits(
    vanity_repo: Path,
    source_repo_dir: Path | None,
    source_repo_url: str | None,
    source_author_name: str | None,
    source_author_email: str | None,
    vanity_author_name: str | None,
    vanity_author_email: str | None,
    source_web_base_url: str | None,
    allow_non_vanity_target: bool,
    dry_run: bool,
    limit: int | None,
) -> tuple[int, int, int]:
    ensure_git_repo(vanity_repo)
    if not dry_run:
        assert_vanity_target_repo(vanity_repo, allow_non_vanity_target)

    temp_repo_handle: tempfile.TemporaryDirectory[str] | None = None
    source_repo_hint = ""
    if source_repo_dir:
        source_repo = source_repo_dir
        ensure_git_repo(source_repo)
        source_repo_hint = str(source_repo_dir)
    elif source_repo_url:
        temp_repo_handle, source_repo = fetch_source_repo_into_temp(source_repo_url)
        source_repo_hint = source_repo_url
    else:
        raise ValueError("Provide either --source-repo-dir or --source-repo-url.")

    try:
        authored = load_authored_commits(source_repo, source_author_name, source_author_email)
        existing = existing_mirrored_shas(vanity_repo)
        pending = [commit for commit in authored if commit.sha not in existing]
        if limit is not None:
            pending = pending[:limit]

        pending_iterable = tqdm(pending, desc="Creating vanity commits", unit="commit")
        for commit in pending_iterable:
            message = build_commit_message(
                source_repo_hint=source_repo_hint,
                source_commit=commit,
                source_web_base_url=source_web_base_url,
            )
            create_empty_commit(
                vanity_repo=vanity_repo,
                message=message,
                date_iso=commit.author_date_iso,
                vanity_author_name=vanity_author_name,
                vanity_author_email=vanity_author_email,
                dry_run=dry_run,
            )

        return len(authored), len(existing), len(pending)
    finally:
        if temp_repo_handle:
            temp_repo_handle.cleanup()


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Mirror authored commits from another repo as idempotent empty commits in this vanity repo."
        )
    )
    parser.add_argument("--vanity-repo-dir", default=".", help="Path to this vanity repository.")
    parser.add_argument(
        "--source-web-base-url",
        help="Optional source repository web base URL used to build Source-Commit-URL.",
    )
    parser.add_argument(
        "--rewrite-history",
        action="store_true",
        help="Rewrite commit messages in history for mirrored commits (uses git filter-branch).",
    )
    parser.add_argument(
        "--rewrite-range",
        default="HEAD",
        help="Revision range passed to git filter-branch when --rewrite-history is used.",
    )
    parser.add_argument(
        "--message-filter",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--allow-non-vanity-target",
        action="store_true",
        help=(
            "Bypass safety guard that blocks mutation when target repo origin is not "
            "https://github.com/TeamDman/Vanity"
        ),
    )

    source_group = parser.add_mutually_exclusive_group(required=False)
    source_group.add_argument(
        "--source-repo-dir",
        help="Path to local source repository (use this OR --source-repo-url).",
    )
    source_group.add_argument(
        "--source-repo-url",
        help="Source repository URL for ephemeral fetch (use this OR --source-repo-dir).",
    )
    parser.add_argument("--source-author-name", help="Exact source commit author name to mirror.")
    parser.add_argument("--source-author-email", help="Exact source commit author email to mirror.")
    parser.add_argument("--vanity-author-name", help="Author name to use for vanity commits.")
    parser.add_argument("--vanity-author-email", help="Author email to use for vanity commits.")
    parser.add_argument("--limit", type=int, help="Create at most N new vanity commits.")
    parser.add_argument("--dry-run", action="store_true", help="Compute pending commits without creating them.")
    return parser.parse_args(list(argv))


def main(argv: Iterable[str]) -> int:
    args = parse_args(argv)

    if args.message_filter:
        return message_filter_mode(args.source_web_base_url)

    vanity_repo = Path(args.vanity_repo_dir).resolve()

    if args.rewrite_history:
        return rewrite_history_messages(
            vanity_repo=vanity_repo,
            source_web_base_url=args.source_web_base_url,
            rewrite_range=args.rewrite_range,
            allow_non_vanity_target=args.allow_non_vanity_target,
        )

    if not args.source_repo_dir and not args.source_repo_url:
        raise ValueError("Provide either --source-repo-dir or --source-repo-url.")

    source_repo_dir = Path(args.source_repo_dir).resolve() if args.source_repo_dir else None

    total_authored, total_existing, total_created = sync_vanity_commits(
        vanity_repo=vanity_repo,
        source_repo_dir=source_repo_dir,
        source_repo_url=args.source_repo_url,
        source_author_name=args.source_author_name,
        source_author_email=args.source_author_email,
        vanity_author_name=args.vanity_author_name,
        vanity_author_email=args.vanity_author_email,
        source_web_base_url=args.source_web_base_url,
        allow_non_vanity_target=args.allow_non_vanity_target,
        dry_run=args.dry_run,
        limit=args.limit,
    )

    mode = "DRY RUN" if args.dry_run else "APPLY"
    print(
        f"[{mode}] authored={total_authored} mirrored_markers={total_existing} newly_created={total_created}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))