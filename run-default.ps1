param(
    [ValidateSet("sync", "rebuild")]
    [string]$Mode = "sync",

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs,

    [switch]$Release = $true
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$runner = Join-Path $repoRoot "run.ps1"

if (-not (Test-Path -LiteralPath $runner -PathType Leaf)) {
    Write-Error "run.ps1 not found at $runner"
    exit 1
}

$defaultThisRepoPath = $repoRoot
$defaultReadRepoPath = "D:\Repos\Minecraft\SFM\repos2\1.19.2"

if ($Mode -eq "rebuild") {
    Write-Warning "Mode 'rebuild' is not implemented in the template-aligned CLI; running 'sync' instead."
    $Mode = "sync"
}

if ($Release) {
    & $runner -Release this-repo set "$defaultThisRepoPath"
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $runner -Release read-repo add "$defaultReadRepoPath"
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $runner -Release $Mode @ExtraArgs
}
else {
    & $runner this-repo set "$defaultThisRepoPath"
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $runner read-repo add "$defaultReadRepoPath"
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $runner $Mode @ExtraArgs
}

exit $LASTEXITCODE