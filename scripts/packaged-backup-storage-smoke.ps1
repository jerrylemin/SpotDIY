[CmdletBinding()]
param(
    [int]$TimeoutSeconds = 45
)

$ErrorActionPreference = "Stop"

if ($env:SPOTDIY_PACKAGED_SMOKE -ne "1") {
    Write-Output "SKIP: set SPOTDIY_PACKAGED_SMOKE=1 to run the packaged Plan 13 backup/storage smoke"
    exit 0
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\")).Path
$targetRoot = $env:CARGO_TARGET_DIR
if ([string]::IsNullOrWhiteSpace($targetRoot)) {
    throw "CARGO_TARGET_DIR must point outside the repository before packaged verification"
}

$releaseExecutable = Join-Path $targetRoot "release\spotdiy.exe"
$nodeHarness = Join-Path $repositoryRoot "scripts\packaged-backup-storage-smoke.mjs"
foreach ($requiredPath in @($releaseExecutable, $nodeHarness)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "required packaged Plan 13 smoke input is missing: $requiredPath"
    }
}

$profileRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("SpotDIY-Plan13-Packaged-" + [guid]::NewGuid().ToString("N"))
$appRoot = Join-Path $profileRoot "App"
$isolatedExecutable = Join-Path $appRoot "spotdiy.exe"
$webViewRoot = Join-Path $profileRoot "WebView2"
$portableMarker = Join-Path $appRoot "SpotDIY.portable"
$standardDatabase = Join-Path $profileRoot "SpotDIY\spotdiy.sqlite3"
$portableDatabase = Join-Path $appRoot "Database\spotdiy.sqlite3"
$firstApp = $null
$secondApp = $null
$thirdApp = $null

function Wait-ForCdp([int]$debugPort, [int]$timeoutMs) {
    $deadline = [datetime]::UtcNow.AddMilliseconds($timeoutMs)
    $uri = "http://127.0.0.1:$debugPort/json/version"
    do {
        try {
            $response = Invoke-RestMethod -Uri $uri -TimeoutSec 2
            if ($response.webSocketDebuggerUrl) {
                return "http://127.0.0.1:$debugPort"
            }
        } catch {
            Start-Sleep -Milliseconds 250
        }
    } while ([datetime]::UtcNow -lt $deadline)
    throw "packaged WebView2 remote debugging endpoint did not become ready"
}

function Start-PackagedApp([int]$debugPort, [string]$label) {
    $stdoutLog = Join-Path $profileRoot ("$label.stdout.log")
    $stderrLog = Join-Path $profileRoot ("$label.stderr.log")
    $previousLocalAppData = $env:LOCALAPPDATA
    $previousPackagedDataRoot = $env:SPOTDIY_PACKAGED_DATA_ROOT
    $previousWebViewRoot = $env:WEBVIEW2_USER_DATA_FOLDER
    $previousWebViewArgs = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    $env:LOCALAPPDATA = $profileRoot
    $env:SPOTDIY_PACKAGED_DATA_ROOT = $profileRoot
    $env:WEBVIEW2_USER_DATA_FOLDER = $webViewRoot
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$debugPort"
    try {
        $process = Start-Process -FilePath $isolatedExecutable `
            -WorkingDirectory $appRoot `
            -WindowStyle Hidden `
            -RedirectStandardOutput $stdoutLog `
            -RedirectStandardError $stderrLog `
            -PassThru
    } finally {
        $env:LOCALAPPDATA = $previousLocalAppData
        $env:SPOTDIY_PACKAGED_DATA_ROOT = $previousPackagedDataRoot
        $env:WEBVIEW2_USER_DATA_FOLDER = $previousWebViewRoot
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousWebViewArgs
    }
    $process | Add-Member -MemberType NoteProperty -Name SpotDIYStdoutLog -Value $stdoutLog
    $process | Add-Member -MemberType NoteProperty -Name SpotDIYStderrLog -Value $stderrLog
    return $process
}

function Close-PackagedApp([System.Diagnostics.Process]$process) {
    if ($null -eq $process) {
        return
    }
    $process.Refresh()
    if (-not $process.HasExited) {
        [void]$process.CloseMainWindow()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            throw "packaged Plan 13 smoke process did not exit within the timeout"
        }
    }
}

function Assert-Directory([string]$path) {
    if (-not (Test-Path -LiteralPath $path -PathType Container)) {
        throw "expected packaged Plan 13 storage directory is missing: $path"
    }
}

function Assert-File([string]$path) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "expected packaged Plan 13 storage file is missing: $path"
    }
}

function Assert-PackagedPortableLayout {
    if (-not (Test-Path -LiteralPath $portableMarker -PathType Leaf)) {
        throw "portable marker was not created beside the isolated executable"
    }
    foreach ($directory in @("Data", "Music", "Covers", "Lyrics", "Database", "Cache", "Config")) {
        Assert-Directory (Join-Path $appRoot $directory)
    }
    Assert-Directory (Join-Path $appRoot "Cache\artwork")
    Assert-Directory (Join-Path $appRoot "Cache\downloads")
    Assert-Directory (Join-Path $appRoot "Data\restore")
    Assert-Directory (Join-Path $appRoot "Data\restore\rollback")
    Assert-File $portableDatabase
    Assert-File $standardDatabase
}

try {
    New-Item -ItemType Directory -Path $appRoot -Force | Out-Null
    Copy-Item -LiteralPath $releaseExecutable -Destination $isolatedExecutable
    if (Test-Path -LiteralPath $portableMarker) {
        throw "isolated Plan 13 smoke executable unexpectedly already has a portable marker"
    }

    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()

    $firstApp = Start-PackagedApp $port "plan13-first"
    $env:SPOTDIY_PACKAGED_CDP_URL = Wait-ForCdp $port ($TimeoutSeconds * 1000)
    & pnpm exec node $nodeHarness "standard"
    if ($LASTEXITCODE -ne 0) {
        throw "packaged Plan 13 Standard phase failed with exit code $LASTEXITCODE"
    }
    Close-PackagedApp $firstApp
    Assert-PackagedPortableLayout

    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    $secondApp = Start-PackagedApp $port "plan13-second"
    $env:SPOTDIY_PACKAGED_CDP_URL = Wait-ForCdp $port ($TimeoutSeconds * 1000)
    & pnpm exec node $nodeHarness "portable"
    if ($LASTEXITCODE -ne 0) {
        throw "packaged Plan 13 Portable phase failed with exit code $LASTEXITCODE"
    }
    Close-PackagedApp $secondApp
    if (Test-Path -LiteralPath $portableMarker) {
        throw "portable marker remained after preparing the Standard transition"
    }
    Assert-File $standardDatabase
    Assert-File $portableDatabase

    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    $thirdApp = Start-PackagedApp $port "plan13-third"
    $env:SPOTDIY_PACKAGED_CDP_URL = Wait-ForCdp $port ($TimeoutSeconds * 1000)
    & pnpm exec node $nodeHarness "standard-final"
    if ($LASTEXITCODE -ne 0) {
        throw "packaged Plan 13 final Standard phase failed with exit code $LASTEXITCODE"
    }
    Close-PackagedApp $thirdApp
    Write-Output "PASS: packaged Plan 13 backup/storage smoke verified Standard, Portable, restart selection, exact directories, and retained databases"
} finally {
    Close-PackagedApp $thirdApp
    Close-PackagedApp $secondApp
    Close-PackagedApp $firstApp
    Remove-Item Env:SPOTDIY_PACKAGED_CDP_URL -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $profileRoot) {
        Remove-Item -LiteralPath $profileRoot -Recurse -Force
    }
}
