[CmdletBinding()]
param(
    [int]$TimeoutSeconds = 45,
    [switch]$Plan08Persistence,
    [switch]$Plan09Lyrics,
    [switch]$Plan11Shell,
    [switch]$Plan12Windows
)

$ErrorActionPreference = "Stop"

if ($env:SPOTDIY_PACKAGED_SMOKE -ne "1") {
    Write-Output "SKIP: set SPOTDIY_PACKAGED_SMOKE=1 to run the packaged playback smoke"
    exit 0
}

if (@($Plan08Persistence, $Plan09Lyrics, $Plan11Shell, $Plan12Windows).Where({ $_ }).Count -gt 1) {
    throw "choose one packaged smoke mode"
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\")).Path
$smokeLabel = if ($Plan12Windows) { "Plan12" } elseif ($Plan11Shell) { "Plan11" } elseif ($Plan09Lyrics) { "Plan09" } elseif ($Plan08Persistence) { "Plan08" } else { "Plan04" }
$flowMode = if ($Plan12Windows) { "plan12" } elseif ($Plan11Shell) { "plan11" } elseif ($Plan09Lyrics) { "plan09" } elseif ($Plan08Persistence) { "plan08" } else { "flow" }
$restartMode = if ($Plan12Windows) { "plan12-restart" } elseif ($Plan11Shell) { "plan11-restart" } elseif ($Plan09Lyrics) { "plan09-restart" } elseif ($Plan08Persistence) { "plan08-restart" } else { "restart" }
$targetRoot = $env:CARGO_TARGET_DIR
if ([string]::IsNullOrWhiteSpace($targetRoot)) {
    throw "CARGO_TARGET_DIR must point outside the repository before packaged verification"
}

$releaseExecutable = Join-Path $targetRoot "release\spotdiy.exe"
$mpvExecutable = Join-Path $repositoryRoot ".tools\mpv\v0.41.0\mpv.exe"
$nodeHarness = Join-Path $repositoryRoot "scripts\packaged-playback-smoke.mjs"
foreach ($requiredPath in @($releaseExecutable, $mpvExecutable, $nodeHarness)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "required packaged smoke input is missing: $requiredPath"
    }
}

function Get-MpvProcesses {
    @(Get-CimInstance Win32_Process -Filter "Name = 'mpv.exe'" | Select-Object ProcessId, ParentProcessId, CommandLine)
}

function Get-OwnedMpvProcessIds([int]$parentId, [object[]]$before) {
    $knownIds = @($before | ForEach-Object { [int]$_.ProcessId })
    @(Get-MpvProcesses | Where-Object {
        $commandLine = [string]$_.CommandLine
        [int]$_.ParentProcessId -eq $parentId -and
            $knownIds -notcontains [int]$_.ProcessId -and
            $commandLine -match "spotdiy-mpv-"
    } | ForEach-Object { [int]$_.ProcessId })
}

function Wait-ForProcessExit([System.Diagnostics.Process]$process, [int]$timeoutMs) {
    if (-not $process.WaitForExit($timeoutMs)) {
        throw "SpotDIY did not exit within the timeout"
    }
}

function Close-PackagedApp([string]$label, [System.Diagnostics.Process]$process) {
    $process.Refresh()
    Write-Output "$label before close: pid=$($process.Id) exited=$($process.HasExited) handle=$($process.MainWindowHandle) title=$($process.MainWindowTitle)"
    $closeResult = $process.CloseMainWindow()
    Write-Output "$label close requested: result=$closeResult"
}

function Stop-ExactOwnedMpv([int[]]$processIds) {
    foreach ($processId in @($processIds)) {
        $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
        if ($null -ne $process) {
            Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
        }
    }
}

function New-SilentWav([string]$path, [int]$frequency) {
    $sampleRate = 44100
    $seconds = 30
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

$profileRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("SpotDIY-$smokeLabel-Packaged-" + [guid]::NewGuid().ToString("N"))
$fixtureFolder = Join-Path $profileRoot "Synthetic Music"
$webViewRoot = Join-Path $profileRoot "WebView2"
$port = $null
$firstApp = $null
$secondApp = $null
$ownedMpvIds = [System.Collections.Generic.List[int]]::new()

function Initialize-LegacySchema6Database {
    $sqliteCommand = Get-Command sqlite3.exe -ErrorAction SilentlyContinue
    if ($null -eq $sqliteCommand) {
        $sqliteCommand = Get-Command sqlite3 -ErrorAction SilentlyContinue
    }
    if ($null -eq $sqliteCommand) {
        throw "sqlite3 is required to seed the packaged Plan 11 schema-6 fixture"
    }

    $databaseDirectory = Join-Path $profileRoot "SpotDIY"
    New-Item -ItemType Directory -Path $databaseDirectory -Force | Out-Null
    $databasePath = Join-Path $databaseDirectory "spotdiy.sqlite3"
    $migrationPaths = @(
        (Join-Path $repositoryRoot "src-tauri\migrations\fixtures\legacy_schema6_initial.sql"),
        (Join-Path $repositoryRoot "src-tauri\migrations\0002_local_library.sql"),
        (Join-Path $repositoryRoot "src-tauri\migrations\0003_source_fusion.sql"),
        (Join-Path $repositoryRoot "src-tauri\migrations\0004_downloads.sql"),
        (Join-Path $repositoryRoot "src-tauri\migrations\0005_collections_and_queue.sql"),
        (Join-Path $repositoryRoot "src-tauri\migrations\0006_lyrics_bookmarks.sql")
    )

    foreach ($migrationPath in $migrationPaths) {
        $output = Get-Content -LiteralPath $migrationPath -Raw | & $sqliteCommand.Source $databasePath 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "could not seed packaged schema-6 fixture from $migrationPath`n$output"
        }
    }

    $seedSql = @"
INSERT OR REPLACE INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at) VALUES ('theme', '"light"', 'theme', 1, '1970-01-01T00:00:00Z');
INSERT OR REPLACE INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at) VALUES ('source_preference_order', '["local","youtube","soundcloud","spotify"]', 'source_preference_order', 1, '1970-01-01T00:00:00Z');
UPDATE settings_metadata SET value_json = 'false', value_type = 'boolean' WHERE setting_key = 'first_run';
PRAGMA user_version = 6;
"@
    $output = $seedSql | & $sqliteCommand.Source $databasePath 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "could not finish packaged schema-6 fixture setup`n$output"
    }
}

function Assert-Schema8Database {
    $sqliteCommand = Get-Command sqlite3.exe -ErrorAction SilentlyContinue
    if ($null -eq $sqliteCommand) {
        $sqliteCommand = Get-Command sqlite3 -ErrorAction SilentlyContinue
    }
    if ($null -eq $sqliteCommand) {
        throw "sqlite3 is required to verify the packaged Plan 12 schema-8 database"
    }

    $databasePath = Join-Path $profileRoot "SpotDIY\spotdiy.sqlite3"
    if (-not (Test-Path -LiteralPath $databasePath -PathType Leaf)) {
        throw "the packaged Plan 12 database was not created: $databasePath"
    }
    $schemaVersion = (& $sqliteCommand.Source $databasePath "PRAGMA user_version;" 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $schemaVersion -ne "8") {
        throw "the packaged database schema is not version 8: $schemaVersion"
    }
}

function Start-PackagedApp([int]$debugPort) {
    $stdoutLog = Join-Path $profileRoot ("spotdiy-$debugPort.stdout.log")
    $stderrLog = Join-Path $profileRoot ("spotdiy-$debugPort.stderr.log")
    $previousLocalAppData = $env:LOCALAPPDATA
    $previousMpvPath = $env:SPOTDIY_MPV_PATH
    $previousPackagedDataRoot = $env:SPOTDIY_PACKAGED_DATA_ROOT
    $previousWebViewRoot = $env:WEBVIEW2_USER_DATA_FOLDER
    $previousWebViewArgs = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    $env:LOCALAPPDATA = $profileRoot
    $env:SPOTDIY_MPV_PATH = $mpvExecutable
    $env:SPOTDIY_PACKAGED_DATA_ROOT = $profileRoot
    $env:WEBVIEW2_USER_DATA_FOLDER = $webViewRoot
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$debugPort"
    try {
        $process = Start-Process -FilePath $releaseExecutable `
            -WorkingDirectory $repositoryRoot `
            -WindowStyle Hidden `
            -RedirectStandardOutput $stdoutLog `
            -RedirectStandardError $stderrLog `
            -PassThru
    } finally {
        $env:LOCALAPPDATA = $previousLocalAppData
        $env:SPOTDIY_MPV_PATH = $previousMpvPath
        $env:SPOTDIY_PACKAGED_DATA_ROOT = $previousPackagedDataRoot
        $env:WEBVIEW2_USER_DATA_FOLDER = $previousWebViewRoot
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousWebViewArgs
    }
    $process | Add-Member -MemberType NoteProperty -Name SpotDIYStdoutLog -Value $stdoutLog
    $process | Add-Member -MemberType NoteProperty -Name SpotDIYStderrLog -Value $stderrLog
    return $process
}

function Write-AppDiagnostics([string]$label, [System.Diagnostics.Process]$process) {
    if ($null -eq $process) {
        return
    }
    if (-not $process.HasExited) {
        return
    }
    Write-Output "$label exited with code $($process.ExitCode)"
    if (Test-Path -LiteralPath $process.SpotDIYStdoutLog) {
        $stdout = Get-Content -LiteralPath $process.SpotDIYStdoutLog -Raw
        if (-not [string]::IsNullOrWhiteSpace($stdout)) {
            Write-Output "$label stdout:`n$stdout"
        }
    }
    if (Test-Path -LiteralPath $process.SpotDIYStderrLog) {
        $stderr = Get-Content -LiteralPath $process.SpotDIYStderrLog -Raw
        if (-not [string]::IsNullOrWhiteSpace($stderr)) {
            Write-Output "$label stderr:`n$stderr"
        }
    }
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

try {
    New-Item -ItemType Directory -Path $fixtureFolder -Force | Out-Null
    New-SilentWav (Join-Path $fixtureFolder "01-night-drive.wav") 440
    New-SilentWav (Join-Path $fixtureFolder "02-static-bloom.wav") 660
    if ($Plan09Lyrics) {
        [System.IO.File]::WriteAllText(
            (Join-Path $fixtureFolder "01-night-drive.lrc"),
            "[00:00.50]First synthetic line`r`n[00:02.00]Second synthetic line`r`n",
            [System.Text.UTF8Encoding]::new($false)
        )
    }
    if ($Plan11Shell) {
        Initialize-LegacySchema6Database
    }

    $beforeFirstLaunch = Get-MpvProcesses
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()

    $firstApp = Start-PackagedApp $port
    $cdp = Wait-ForCdp $port ($TimeoutSeconds * 1000)
    $env:SPOTDIY_PACKAGED_CDP_URL = $cdp
    $env:SPOTDIY_PACKAGED_FIXTURE = $fixtureFolder
    & pnpm exec node $nodeHarness $flowMode
    if ($LASTEXITCODE -ne 0) {
        throw "the packaged $smokeLabel flow failed with exit code $LASTEXITCODE"
    }

    Start-Sleep -Milliseconds 500
    $ownedMpvIds.AddRange([int[]](Get-OwnedMpvProcessIds $firstApp.Id $beforeFirstLaunch))
    if ($ownedMpvIds.Count -eq 0) {
        throw "the packaged playback flow did not create a SpotDIY-owned mpv child"
    }

    Close-PackagedApp "first packaged app" $firstApp
    Wait-ForProcessExit $firstApp ($TimeoutSeconds * 1000)
    Start-Sleep -Milliseconds 500
    if ($Plan12Windows) {
        Assert-Schema8Database
    }
    $remaining = @(Get-MpvProcesses | Where-Object { $ownedMpvIds -contains [int]$_.ProcessId })
    if ($remaining.Count -ne 0) {
        throw "SpotDIY-owned mpv process remained after graceful shutdown: $($ownedMpvIds -join ', ')"
    }

    $beforeSecondLaunch = Get-MpvProcesses
    $secondPortListener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $secondPortListener.Start()
    $secondPort = $secondPortListener.LocalEndpoint.Port
    $secondPortListener.Stop()
    $secondApp = Start-PackagedApp $secondPort
    $secondCdp = Wait-ForCdp $secondPort ($TimeoutSeconds * 1000)
    $env:SPOTDIY_PACKAGED_CDP_URL = $secondCdp
    & pnpm exec node $nodeHarness $restartMode
    if ($LASTEXITCODE -ne 0) {
        throw "the packaged $smokeLabel restart boundary failed with exit code $LASTEXITCODE"
    }

    Close-PackagedApp "second packaged app" $secondApp
    Wait-ForProcessExit $secondApp ($TimeoutSeconds * 1000)
    if ($Plan12Windows) {
        Write-Output "PASS: packaged Plan 12 schema 8, tray, SMTC status, shortcuts, overlays, click-through recovery, output profiles, restart persistence, and owned-process cleanup"
    } elseif ($Plan11Shell) {
        Write-Output "PASS: packaged Plan 11 schema migration, appearance persistence, shell modes, inspector, queue, lyrics, restart, and owned-process persistence"
    } elseif ($Plan09Lyrics) {
        Write-Output "PASS: packaged Plan 09 lyrics, bookmarks, A/B loop, presets, queue, restart, and owned-process persistence"
    } elseif ($Plan08Persistence) {
        Write-Output "PASS: packaged Plan 08 playlist, collection, queue, snapshot, restart, and owned-process persistence"
    } else {
        Write-Output "PASS: packaged playback, restart boundary, and owned-process cleanup"
    }
} finally {
    Remove-Item Env:SPOTDIY_PACKAGED_CDP_URL -ErrorAction SilentlyContinue
    Remove-Item Env:SPOTDIY_PACKAGED_FIXTURE -ErrorAction SilentlyContinue
    if ($null -ne $firstApp -and -not $firstApp.HasExited) {
        $firstApp.CloseMainWindow() | Out-Null
        if (-not $firstApp.WaitForExit(3000)) {
            $firstApp.Kill()
            $firstApp.WaitForExit(3000)
        }
    }
    if ($null -ne $secondApp -and -not $secondApp.HasExited) {
        $secondApp.CloseMainWindow() | Out-Null
        if (-not $secondApp.WaitForExit(3000)) {
            $secondApp.Kill()
            $secondApp.WaitForExit(3000)
        }
    }
    Write-AppDiagnostics "first packaged app" $firstApp
    Write-AppDiagnostics "second packaged app" $secondApp
    Stop-ExactOwnedMpv $ownedMpvIds.ToArray()
    if (Test-Path -LiteralPath $profileRoot) {
        for ($attempt = 0; $attempt -lt 10 -and (Test-Path -LiteralPath $profileRoot); $attempt++) {
            try {
                Remove-Item -LiteralPath $profileRoot -Recurse -Force -ErrorAction Stop
            } catch {
                Start-Sleep -Milliseconds 500
            }
        }
        if (Test-Path -LiteralPath $profileRoot) {
            throw "packaged smoke profile could not be cleaned up: $profileRoot"
        }
    }
}
