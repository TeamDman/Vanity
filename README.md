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
  --source-author-name "TeamDman" \
  --source-author-email "you@example.com" \
  --vanity-author-name "TeamDman" \
  --vanity-author-email "you@example.com"
```

Dry-run preview:

```bash
./run.ps1 \
  --source-repo-dir "D:/Repos/Minecraft/SFM/repos2/1.19.2" \
  --source-author-name "TeamDman" \
  --source-author-email "you@example.com" \
  --dry-run
```

`run.ps1` uses `uv run` under the hood.

## GitHub Actions automation

Workflow file: `.github/workflows/vanity-sync.yml`

1. Ensure `TeamDman9201@gmail.com` is associated with your GitHub account.
2. Run the workflow manually once via **Actions** → **Vanity Commit Sync** → **Run workflow**.
3. After bootstrap, it runs daily.

If your source-author email differs from your vanity commit email, update the workflow env values.

Troubleshooting: GitHub can auto-disable scheduled workflows after long inactivity (about 60 days). If that happens, open **Actions** and re-enable the workflow; `workflow_dispatch` (manual run) remains available.