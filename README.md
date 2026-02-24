# Vanity

CLI for creating idempotent empty commits in a target repo based on commits in configured source repos.

## Foreword

This repository exists as an empty mirror of the commits from [TeamDman/SuperFactoryManager](https://github.com/TeamDman/SuperFactoryManager).

[TeamDman/SuperFactoryManager](https://github.com/TeamDman/SuperFactoryManager) is a fork of [gigabit101/SuperFactoryManager](https://github.com/gigabit101/StevesFactoryManager) which is a fork of [Vswe/ModJam3](https://github.com/Vswe/ModJam3).

I think that's beautiful but I also want my activity there to be reflected on my GitHub profile, which being a fork precludes.

Therefore, this repository serves to create empty copies of the commits in [TeamDman/SuperFactoryManager](https://github.com/TeamDman/SuperFactoryManager).

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
# Installation/update
cargo install --path . --locked
```

```powershell
vanity.exe this-repo set "G:\Programming\Repos\Vanity"
vanity.exe read-repo add "D:\Repos\Minecraft\SFM\repos2\1.19.2"
```

Verify configuration:

```powershell
vanity.exe this-repo show
vanity.exe read-repo list
```

## Sync

```powershell
vanity.exe sync
```

Dry run:

```powershell
vanity.exe sync --dry-run
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