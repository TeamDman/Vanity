# Vanity

Creates one idempotent empty commit in this repo for each commit you authored in another repo.

This is useful because I want my activity in a forked repo (https://github.com/TeamDman/SuperFactoryManager.git) to show up on my GitHub profile.

## How idempotency works

Each generated commit includes a unique marker line:

`Vanity-Source-Commit: <source_sha>`

Before creating new commits, the script scans existing markers and skips any source commit that is already mirrored.

## Local usage

Run from this repository:

```bash
./run.ps1 \
  --source-repo-dir "D:/Repos/Minecraft/SFM/repos2/1.19.2" \
  --source-web-base-url "https://github.com/TeamDman/SuperFactoryManager" \
  --source-author-name "TeamDman" \
  --source-author-email "you@example.com" \
  --vanity-author-name "TeamDman" \
  --vanity-author-email "you@example.com"
```

Dry-run preview:

```bash
./run.ps1 \
  --source-repo-dir "D:/Repos/Minecraft/SFM/repos2/1.19.2" \
  --source-web-base-url "https://github.com/TeamDman/SuperFactoryManager" \
  --source-author-name "TeamDman" \
  --source-author-email "you@example.com" \
  --dry-run
```

`run.ps1` uses `uv run` under the hood.

Generated mirror commits include `Source-Commit-URL` when a GitHub web base is known.

Safety guard: by default, mutation operations are blocked unless the target repo `origin` is `https://github.com/TeamDman/Vanity`.
Use `--allow-non-vanity-target` only if you intentionally need to bypass that guard.

## GitHub Actions automation

Workflow file: `.github/workflows/vanity-sync.yml`

1. Ensure `TeamDman9201@gmail.com` is associated with your GitHub account.
2. Run the workflow manually once via **Actions** → **Vanity Commit Sync** → **Run workflow**.
3. After bootstrap, it runs daily.

If your source-author email differs from your vanity commit email, update the workflow env values.

Troubleshooting: GitHub can auto-disable scheduled workflows after long inactivity (about 60 days). If that happens, open **Actions** and re-enable the workflow; `workflow_dispatch` (manual run) remains available.

## Rewrite existing commit messages

If you want to recompute mirrored commit messages (for example, to add `Source-Commit-URL` to old commits), run:

```bash
./run.ps1 \
  --rewrite-history \
  --rewrite-range "main" \
  --source-web-base-url "https://github.com/TeamDman/SuperFactoryManager"
```

Then force-push rewritten history:

```bash
git push --force-with-lease origin main
```

Notes:

- Working tree must be clean before rewrite.
- This rewrites commit hashes for the selected range.
- Rewrite is also protected by the same Vanity-origin safety guard.