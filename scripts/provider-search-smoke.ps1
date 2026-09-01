[CmdletBinding()]
param(
    [switch]$RunLiveProviders,
    [switch]$RunPackaged,
    [int]$TimeoutSeconds = 45
)

$ErrorActionPreference = "Stop"

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\")).Path
$targetRoot = "C:\CargoTarget\SpotDIY"
$env:CARGO_TARGET_DIR = $targetRoot

$repositoryFullPath = ([System.IO.Path]::GetFullPath($repositoryRoot)).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
$targetFullPath = ([System.IO.Path]::GetFullPath($targetRoot)).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if ($targetFullPath.StartsWith($repositoryFullPath, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "CARGO_TARGET_DIR must remain outside the repository"
}

function Invoke-ExactRustTest([string]$filter) {
    & cargo test --manifest-path (Join-Path $repositoryRoot "src-tauri\Cargo.toml") $filter -- --exact --nocapture
    if ($LASTEXITCODE -ne 0) {
        throw "focused native smoke failed: $filter"
    }
}

$nativeSmokeTests = @(
    "sources::local::tests::local_exact_title_ranks_first",
    "sources::local::tests::local_artist_match_returns_track",
    "sources::local::tests::local_album_match_returns_track",
    "playback::tests::clear_queue_stops_without_treating_stop_as_eof",
    "playback::tests::local_play_waits_for_file_loaded_and_restores_a_queued_clamped_seek"
)

foreach ($testFilter in $nativeSmokeTests) {
    Invoke-ExactRustTest $testFilter
}
Write-Output "PASS: native synthetic Local title, artist, album, clear, and Play Now coverage ($($nativeSmokeTests.Count) focused tests)"

function Invoke-StructuredProviderSmoke([string]$label, [string]$searchPrefix, [string]$executable) {
    $arguments = @(
        "--no-config",
        "--dump-single-json",
        "--flat-playlist",
        "--skip-download",
        "--no-warnings",
        "--socket-timeout",
        "10",
        "$($searchPrefix):signal"
    )
    $temporaryRoot = [System.IO.Path]::GetTempPath()
    $suffix = [guid]::NewGuid().ToString("N")
    $stdoutPath = Join-Path $temporaryRoot "spotdiy-provider-$suffix.stdout"
    $stderrPath = Join-Path $temporaryRoot "spotdiy-provider-$suffix.stderr"
    $process = $null
    try {
        $process = Start-Process -FilePath $executable -ArgumentList $arguments -WindowStyle Hidden `
            -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -PassThru
        if (-not $process.WaitForExit(20000)) {
            try {
                $process.Kill()
            } catch {
                # The process may have exited between the timeout and cleanup.
            }
            [void]$process.WaitForExit(3000)
            Write-Output "SKIPPED - $label metadata smoke exceeded the 20 second smoke bound"
            return
        }
        $stdout = if (Test-Path -LiteralPath $stdoutPath) { Get-Content -LiteralPath $stdoutPath -Raw } else { "" }
        if ($process.ExitCode -ne 0) {
            Write-Output "SKIPPED - $label upstream returned exit code $($process.ExitCode); no provider output was recorded"
            return
        }
        try {
            $payload = $stdout | ConvertFrom-Json
            $entries = @($payload.entries)
            Write-Output "PASS: $label metadata-only structured search ($($entries.Count) entries)"
        } catch {
            Write-Output "SKIPPED - $label returned no parseable structured response; raw provider output was not recorded"
        }
    } finally {
        if ($null -ne $process -and -not $process.HasExited) {
            try {
                $process.Kill()
            } catch {
                # Cleanup is best effort after the bounded wait path.
            }
        }
        if ($null -ne $process) {
            $process.Dispose()
        }
        Remove-Item -LiteralPath $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue
    }
}

$liveRequested = $RunLiveProviders -or $env:SPOTDIY_LIVE_PROVIDER_SMOKE -eq "1"
if (-not $liveRequested) {
    Write-Output "SKIPPED - set SPOTDIY_LIVE_PROVIDER_SMOKE=1 or pass -RunLiveProviders for opt-in YouTube/SoundCloud metadata smoke"
} else {
    $ytDlpCommand = Get-Command yt-dlp -ErrorAction SilentlyContinue
    if ($null -eq $ytDlpCommand) {
        Write-Output "SKIPPED - yt-dlp is not available; YouTube and SoundCloud metadata smoke did not run"
    } else {
        $ytDlpPath = $ytDlpCommand.Source
        Invoke-StructuredProviderSmoke "YouTube" "ytsearch25" $ytDlpPath
        Invoke-StructuredProviderSmoke "SoundCloud" "scsearch25" $ytDlpPath
    }
}

Write-Output "SKIPPED - Spotify smoke: no developer authorization"

function New-SilentWav([string]$path, [int]$frequency) {
    $sampleRate = 44100
    $seconds = 4
    $channels = 1
    $bitsPerSample = 16
    $sampleCount = $sampleRate * $seconds
    $blockAlign = $channels * ($bitsPerSample / 8)
    $byteRate = $sampleRate * $blockAlign
    $dataSize = $sampleCount * $blockAlign

    $stream = [System.IO.File]::Open($path, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    try {
        $writer = [System.IO.BinaryWriter]::new($stream)
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("RIFF"))
        $writer.Write([int](36 + $dataSize))
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("WAVE"))
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("fmt "))
        $writer.Write([int]16)
        $writer.Write([int16]1)
        $writer.Write([int16]$channels)
        $writer.Write([int]$sampleRate)
        $writer.Write([int]$byteRate)
        $writer.Write([int16]$blockAlign)
        $writer.Write([int16]$bitsPerSample)
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("data"))
        $writer.Write([int]$dataSize)
        $amplitude = 9000
        for ($sample = 0; $sample -lt $sampleCount; $sample++) {
            $value = [int16]([math]::Sin(2 * [math]::PI * $frequency * $sample / $sampleRate) * $amplitude)
            $writer.Write($value)
        }
        $writer.Flush()
    } finally {
        $stream.Dispose()
    }
}

function Get-SearchSmokeProcesses {
    @(
        Get-CimInstance Win32_Process -Filter "Name = 'spotdiy.exe'" |
            Select-Object Name, ProcessId, ParentProcessId, CommandLine
        Get-CimInstance Win32_Process -Filter "Name = 'yt-dlp.exe'" |
            Select-Object Name, ProcessId, ParentProcessId, CommandLine
        Get-CimInstance Win32_Process -Filter "Name = 'mpv.exe'" |
            Select-Object Name, ProcessId, ParentProcessId, CommandLine
    )
}

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

$packagedRequested = $RunPackaged -or $env:SPOTDIY_PACKAGED_SMOKE -eq "1"
if (-not $packagedRequested) {
    Write-Output "SKIPPED - set SPOTDIY_PACKAGED_SMOKE=1 or pass -RunPackaged for isolated packaged search smoke"
    exit 0
}

$releaseExecutable = Join-Path $targetRoot "release\spotdiy.exe"
$nodeHarness = Join-Path $repositoryRoot "scripts\packaged-search-smoke.mjs"
foreach ($requiredPath in @($releaseExecutable, $nodeHarness)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "required packaged search smoke input is missing: $requiredPath"
    }
}

$profileRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("SpotDIY-Plan05-Packaged-" + [guid]::NewGuid().ToString("N"))
$fixtureFolder = Join-Path $profileRoot "Synthetic Music"
$webViewRoot = Join-Path $profileRoot "WebView2"
$packagedApp = $null
$ownedHelperIds = [System.Collections.Generic.List[int]]::new()

try {
    New-Item -ItemType Directory -Path $fixtureFolder -Force | Out-Null
    New-SilentWav (Join-Path $fixtureFolder "01-night-drive.wav") 440
    New-SilentWav (Join-Path $fixtureFolder "02-static-bloom.wav") 660

    $beforeLaunch = Get-SearchSmokeProcesses
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $debugPort = $listener.LocalEndpoint.Port
    $listener.Stop()

    $stdoutLog = Join-Path $profileRoot "spotdiy.stdout.log"
    $stderrLog = Join-Path $profileRoot "spotdiy.stderr.log"
    $previousPackagedSmoke = $env:SPOTDIY_PACKAGED_SMOKE
    $previousDataRoot = $env:SPOTDIY_PACKAGED_DATA_ROOT
    $previousYtDlpPath = $env:SPOTDIY_YTDLP_PATH
    $previousMpvPath = $env:SPOTDIY_MPV_PATH
    $previousWebViewRoot = $env:WEBVIEW2_USER_DATA_FOLDER
    $previousWebViewArgs = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    $env:SPOTDIY_PACKAGED_SMOKE = "1"
    $env:SPOTDIY_PACKAGED_DATA_ROOT = $profileRoot
    $env:SPOTDIY_YTDLP_PATH = Join-Path $profileRoot "missing-yt-dlp.exe"
    $env:SPOTDIY_MPV_PATH = Join-Path $profileRoot "missing-mpv.exe"
    $env:WEBVIEW2_USER_DATA_FOLDER = $webViewRoot
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$debugPort"
    try {
        $packagedApp = Start-Process -FilePath $releaseExecutable `
            -WorkingDirectory $repositoryRoot `
            -WindowStyle Hidden `
            -RedirectStandardOutput $stdoutLog `
            -RedirectStandardError $stderrLog `
            -PassThru
        $cdp = Wait-ForCdp $debugPort ($TimeoutSeconds * 1000)
        $env:SPOTDIY_PACKAGED_CDP_URL = $cdp
        $env:SPOTDIY_PACKAGED_FIXTURE = $fixtureFolder
        & pnpm exec node $nodeHarness
        if ($LASTEXITCODE -ne 0) {
            throw "packaged search harness failed with exit code $LASTEXITCODE"
        }
    } finally {
        $env:SPOTDIY_PACKAGED_SMOKE = $previousPackagedSmoke
        $env:SPOTDIY_PACKAGED_DATA_ROOT = $previousDataRoot
        $env:SPOTDIY_YTDLP_PATH = $previousYtDlpPath
        $env:SPOTDIY_MPV_PATH = $previousMpvPath
        $env:WEBVIEW2_USER_DATA_FOLDER = $previousWebViewRoot
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousWebViewArgs
        Remove-Item Env:SPOTDIY_PACKAGED_CDP_URL -ErrorAction SilentlyContinue
        Remove-Item Env:SPOTDIY_PACKAGED_FIXTURE -ErrorAction SilentlyContinue
    }

    if ($null -ne $packagedApp -and -not $packagedApp.HasExited) {
        [void]$packagedApp.CloseMainWindow()
        if (-not $packagedApp.WaitForExit($TimeoutSeconds * 1000)) {
            $packagedApp.Kill()
            [void]$packagedApp.WaitForExit(5000)
            throw "packaged SpotDIY did not exit within the smoke timeout"
        }
    }

    Start-Sleep -Milliseconds 500
    $afterExit = Get-SearchSmokeProcesses
    foreach ($process in @($afterExit)) {
        $wasPresent = @($beforeLaunch | Where-Object { [int]$_.ProcessId -eq [int]$process.ProcessId }).Count -gt 0
        if (-not $wasPresent -and [int]$process.ProcessId -ne [int]$packagedApp.Id) {
            $ownedHelperIds.Add([int]$process.ProcessId)
        }
    }
    if ($ownedHelperIds.Count -gt 0) {
        throw "packaged search left new helper processes: $($ownedHelperIds -join ', ')"
    }
    Write-Output "PASS: isolated packaged search, provider failure isolation, cancellation, Spotify gate, and process cleanup"
} finally {
    if ($null -ne $packagedApp -and -not $packagedApp.HasExited) {
        try {
            $packagedApp.CloseMainWindow() | Out-Null
            if (-not $packagedApp.WaitForExit(3000)) {
                $packagedApp.Kill()
                $packagedApp.WaitForExit(3000)
            }
        } catch {
            # Preserve the original smoke failure while attempting exact app cleanup.
        }
    }
    foreach ($processId in $ownedHelperIds) {
        Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $profileRoot) {
        for ($attempt = 0; $attempt -lt 10 -and (Test-Path -LiteralPath $profileRoot); $attempt++) {
            try {
                Remove-Item -LiteralPath $profileRoot -Recurse -Force -ErrorAction Stop
            } catch {
                Start-Sleep -Milliseconds 500
            }
        }
        if (Test-Path -LiteralPath $profileRoot) {
            throw "packaged search smoke profile could not be cleaned up"
        }
    }
}
