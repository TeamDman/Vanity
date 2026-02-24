param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ScriptArgs,

    [switch]$Release = $true
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo is required but was not found in PATH. Install Rust from https://rustup.rs"
    exit 1
}

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $repoRoot
try {
    if ($Release) {
        & cargo run --release -- @ScriptArgs
    }
    else {
        & cargo run -- @ScriptArgs
    }
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}