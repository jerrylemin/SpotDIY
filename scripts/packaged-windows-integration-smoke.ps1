[CmdletBinding()]
param(
    [int]$TimeoutSeconds = 45
)

$ErrorActionPreference = "Stop"

if ($env:SPOTDIY_PACKAGED_SMOKE -ne "1") {
    Write-Output "SKIP: set SPOTDIY_PACKAGED_SMOKE=1 to run the packaged Windows integration smoke"
    exit 0
}

$playbackSmoke = Join-Path $PSScriptRoot "packaged-playback-smoke.ps1"
if (-not (Test-Path -LiteralPath $playbackSmoke -PathType Leaf)) {
    throw "the shared packaged smoke runner is missing: $playbackSmoke"
}

& $playbackSmoke -TimeoutSeconds $TimeoutSeconds -Plan12Windows
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
