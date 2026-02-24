# Vanity

CLI for creating idempotent empty commits in a target repo based on commits in configured source repos.

## Command model

```text
read-repo list
read-repo add <path>
this-repo show
this-repo set <path>
sync
```

All repository paths are canonicalized before storing.

## Setup

```powershell
./run.ps1 this-repo set "G:\Programming\Repos\Vanity"
./run.ps1 read-repo add "D:\Repos\Minecraft\SFM\repos2\1.19.2"
```

Verify configuration:

```powershell
./run.ps1 this-repo show
./run.ps1 read-repo list
```

## Sync

```powershell
./run.ps1 sync
```

Dry run:

```powershell
./run.ps1 sync --dry-run
```

Default helper with baked paths:

```powershell
./run-default.ps1
```

## Commit metadata

Generated commits include:

- `Vanity-Source-Commit: <sha>` marker
- source repo hint
- source commit URL (when derivable from GitHub remote URL)

## Safety

Mutation is blocked unless configured `this-repo` has `origin` pointing at `https://github.com/TeamDman/Vanity`.

Use `--allow-non-vanity-target` only if intentional.