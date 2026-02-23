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


MARKER_PREFIX = "Vanity-Source-Commit: "


@dataclass(frozen=True)
class SourceCommit:
    sha: str
    author_name: str
    author_email: str
    author_date_iso: str
    subject: str


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


def build_commit_message(source_repo_hint: str, source_commit: SourceCommit) -> str:
    return "\n".join(
        [
            f"Vanity mirror: {source_commit.sha[:12]}",
            "",
            f"Source-Repo: {source_repo_hint}",
            f"{MARKER_PREFIX}{source_commit.sha}",
            f"Source-Author: {source_commit.author_name} <{source_commit.author_email}>",
            f"Source-Date: {source_commit.author_date_iso}",
            f"Source-Subject: {source_commit.subject}",
        ]
    )


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
    dry_run: bool,
    limit: int | None,
) -> tuple[int, int, int]:
    ensure_git_repo(vanity_repo)

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

        for commit in pending:
            message = build_commit_message(source_repo_hint, commit)
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
    source_group = parser.add_mutually_exclusive_group(required=True)
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
    vanity_repo = Path(args.vanity_repo_dir).resolve()
    source_repo_dir = Path(args.source_repo_dir).resolve() if args.source_repo_dir else None

    total_authored, total_existing, total_created = sync_vanity_commits(
        vanity_repo=vanity_repo,
        source_repo_dir=source_repo_dir,
        source_repo_url=args.source_repo_url,
        source_author_name=args.source_author_name,
        source_author_email=args.source_author_email,
        vanity_author_name=args.vanity_author_name,
        vanity_author_email=args.vanity_author_email,
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