param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ScriptArgs
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    Write-Error "uv is required but was not found in PATH. Install uv from https://docs.astral.sh/uv/"
    exit 1
}

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $repoRoot
try {
    & uv run --with tqdm python .\scripts\vanity_sync.py @ScriptArgs
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}