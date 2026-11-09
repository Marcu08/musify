param(
    [string]$Output = "docs/M45-BASELINE.md",
    [string]$Label = "Baseline prestazioni",
    [int]$Runs = 10,
    [switch]$NoSmtc,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$dataDir = Join-Path $root "testdata\m45"
$musicDir = Join-Path $dataDir "music"
$readyDir = Join-Path $dataDir "ready"
$exe = Join-Path $root "target\release\musify.exe"
$libraryCli = Join-Path $root "target\release\library-cli.exe"
$outPath = Join-Path $root $Output
$csvPath = [System.IO.Path]::ChangeExtension($outPath, ".csv")

function Get-Median([double[]]$values) {
    if ($values.Count -eq 0) { return 0 }
    $sorted = $values | Sort-Object
    $middle = [int][math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2 -eq 0) {
        return ($sorted[$middle - 1] + $sorted[$middle]) / 2
    }
    return $sorted[$middle]
}

function Get-Min([double[]]$values) {
    if ($values.Count -eq 0) { return 0 }
    return ($values | Measure-Object -Minimum).Minimum
}

function Get-Max([double[]]$values) {
    if ($values.Count -eq 0) { return 0 }
    return ($values | Measure-Object -Maximum).Maximum
}

function Format-Stat([double[]]$values, [string]$unit = "") {
    if ($values.Count -eq 0) { return "n/d" }
    return ("{0:N2}{1} (min {2:N2}, max {3:N2})" -f (Get-Median $values), $unit, (Get-Min $values), (Get-Max $values))
}

function Reset-Ready {
    New-Item -ItemType Directory -Force -Path $readyDir | Out-Null
    Remove-Item (Join-Path $readyDir "*") -Force -ErrorAction SilentlyContinue
}

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class M45Window {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
}
"@

function Focus-App($process) {
    $process.Refresh()
    $handle = $process.MainWindowHandle
    if ($handle -eq 0) { return }
    [void][M45Window]::ShowWindow($handle, 9)
    # Windows only lets a background process take the foreground after a user input.
    [M45Window]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
    [void][M45Window]::SetForegroundWindow($handle)
    [M45Window]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
}

function Read-Ready([double]$wallMs) {
    $file = Get-ChildItem $readyDir -File -Filter "musify.exe.*" -ErrorAction SilentlyContinue |
        Where-Object { $_.Extension -ne ".tmp" } |
        Select-Object -First 1
    if (-not $file) { return $null }
    $json = Get-Content $file.FullName -Raw | ConvertFrom-Json
    return [pscustomobject]@{
        WallMs = $wallMs
        ReadyMs = [double]$json.ready_ms
        File = $file.FullName
    }
}

function Wait-Ready([int]$TimeoutMs = 15000) {
    for ($i = 0; $i -lt ($TimeoutMs / 10); $i++) {
        Start-Sleep -Milliseconds 10
        $file = Get-ChildItem $readyDir -File -Filter "musify.exe.*" -ErrorAction SilentlyContinue |
            Where-Object { $_.Extension -ne ".tmp" } |
            Select-Object -First 1
        if ($file) { return $true }
    }
    return $false
}

function Stop-App($process) {
    if ($null -eq $process) { return }
    try {
        $process.Refresh()
        if (-not $process.HasExited) {
            [void]$process.CloseMainWindow()
            if (-not $process.WaitForExit(5000)) {
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            }
        }
    } catch {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-Run([bool]$Autoplay) {
    if ($Autoplay) {
        $env:MUSIFY_AUTOPLAY = "1"
    } else {
        Remove-Item Env:MUSIFY_AUTOPLAY -ErrorAction SilentlyContinue
    }
    Reset-Ready
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -WorkingDirectory $root -PassThru `
        -RedirectStandardOutput (Join-Path $dataDir "run.out.log") `
        -RedirectStandardError (Join-Path $dataDir "run.err.log")
    $ready = $null
    for ($i = 0; $i -lt 1500; $i++) {
        Start-Sleep -Milliseconds 10
        $file = Get-ChildItem $readyDir -File -Filter "musify.exe.*" -ErrorAction SilentlyContinue |
            Where-Object { $_.Extension -ne ".tmp" } |
            Select-Object -First 1
        if ($file) {
            $json = Get-Content $file.FullName -Raw | ConvertFrom-Json
            $ready = [pscustomobject]@{
                WallMs = $sw.Elapsed.TotalMilliseconds
                ReadyMs = [double]$json.ready_ms
            }
            break
        }
    }
    $sw.Stop()
    if ($null -eq $ready) {
        Stop-App $p
        throw "Musify non ha scritto il segnale di ready entro 15 s (autoplay=$Autoplay)"
    }

    # Let the first frame settle and the initial incremental scan finish.
    Start-Sleep -Milliseconds 1000
    Focus-App $p
    Start-Sleep -Milliseconds 1000
    $proc = Get-Process -Id $p.Id
    $proc.Refresh()
    $workingSet = [math]::Round($proc.WorkingSet64 / 1MB, 2)
    $privateBytes = [math]::Round($proc.PrivateMemorySize64 / 1MB, 2)

    $cpu1 = $proc.CPU
    $wall1 = Get-Date
    Start-Sleep -Seconds 5
    $proc.Refresh()
    $cpu2 = $proc.CPU
    $wall2 = Get-Date
    $wallSeconds = ($wall2 - $wall1).TotalSeconds
    $cpuSeconds = [double]($cpu2 - $cpu1)
    if ($cpuSeconds -lt 0) { $cpuSeconds = 0.0 }
    $oneCore = if ($wallSeconds -gt 0) { 100.0 * $cpuSeconds / $wallSeconds } else { 0.0 }
    $normalized = $oneCore / $logicalProcessors

    $result = [pscustomobject]@{
        Autoplay = $Autoplay
        ReadyMs = $ready.ReadyMs
        WallMs = $ready.WallMs
        WorkingSetMb = $workingSet
        PrivateMb = $privateBytes
        CpuOneCorePct = [math]::Round($oneCore, 3)
        CpuNormalizedPct = [math]::Round($normalized, 3)
        CpuSeconds = [math]::Round($cpuSeconds, 3)
    }
    Stop-App $p
    Start-Sleep -Milliseconds 300
    return $result
}

if (Get-Process musify -ErrorAction SilentlyContinue) {
    throw "Chiudi tutte le istanze di Musify prima di eseguire il benchmark."
}

New-Item -ItemType Directory -Force -Path $dataDir, $musicDir, $readyDir | Out-Null

if (-not $SkipBuild) {
    Push-Location $root
    try {
        cargo build -p musify --release
        cargo build -p musify-library --release --bin library-cli
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path (Join-Path $musicDir "m45-long.flac"))) {
    $ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
    if ($ffmpeg) {
        & ffmpeg -y -hide_banner -loglevel error -f lavfi `
            -i "sine=frequency=220:duration=30:sample_rate=44100" -ac 2 `
            -metadata title="M45 Long Tone" -metadata artist="Musify Benchmark" `
            -metadata album="M45" -c:a flac (Join-Path $musicDir "m45-long.flac")
    } else {
        Copy-Item (Join-Path $root "crates\audio\tests\assets\tone.flac") `
            (Join-Path $musicDir "m45-long.flac") -Force
    }
}

& $libraryCli scan $musicDir --db (Join-Path $dataDir "library.sqlite3") `
    --covers (Join-Path $dataDir "covers") | Out-Null

Remove-Item (Join-Path $dataDir "state.json") -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $dataDir "state.json.tmp") -Force -ErrorAction SilentlyContinue

$env:MUSIFY_DATA_DIR = (Resolve-Path $dataDir).Path
$env:MUSIFY_MUSIC_DIRS = (Resolve-Path $musicDir).Path
$env:RUNE_READY_DIR = (Resolve-Path $readyDir).Path
Remove-Item Env:MUSIFY_NO_AUDIO -ErrorAction SilentlyContinue
Remove-Item Env:MUSIFY_AUTOPLAY -ErrorAction SilentlyContinue
if ($NoSmtc) {
    $env:MUSIFY_NO_SMTC = "1"
} else {
    Remove-Item Env:MUSIFY_NO_SMTC -ErrorAction SilentlyContinue
}

$logicalProcessors = [int](Get-CimInstance Win32_Processor |
    Measure-Object -Property NumberOfLogicalProcessors -Sum).Sum
$cpuName = (Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name).Trim()
$os = Get-CimInstance Win32_OperatingSystem
$commit = (git -C $root rev-parse --short HEAD 2>$null)
$rustc = (rustc --version)

$idleRuns = @()
$playRuns = @()

Write-Host "M4.5 benchmark: $Runs avvii di startup + $Runs avvii in riproduzione"
for ($i = 1; $i -le $Runs; $i++) {
    Write-Host ("  startup {0}/{1}" -f $i, $Runs)
    $idleRuns += Invoke-Run $false
}
Remove-Item (Join-Path $dataDir "state.json") -Force -ErrorAction SilentlyContinue
for ($i = 1; $i -le $Runs; $i++) {
    Write-Host ("  playback {0}/{1}" -f $i, $Runs)
    $playRuns += Invoke-Run $true
}

$idleReady = @($idleRuns | ForEach-Object { $_.ReadyMs })
$idleWall = @($idleRuns | ForEach-Object { $_.WallMs })
$idleWs = @($idleRuns | ForEach-Object { $_.WorkingSetMb })
$idlePrivate = @($idleRuns | ForEach-Object { $_.PrivateMb })
$idleCpu = @($idleRuns | ForEach-Object { $_.CpuOneCorePct })
$playWs = @($playRuns | ForEach-Object { $_.WorkingSetMb })
$playPrivate = @($playRuns | ForEach-Object { $_.PrivateMb })
$playCpu = @($playRuns | ForEach-Object { $_.CpuOneCorePct })
$playCpuNorm = @($playRuns | ForEach-Object { $_.CpuNormalizedPct })

$markdown = @"
# M4.5 — $Label

Data: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
Commit: ``$commit``
Macchina: $cpuName · $logicalProcessors thread · $([math]::Round($os.TotalVisibleMemorySize / 1MB, 1)) GB RAM
OS: $($os.Caption) $($os.Version)
Rust: ``$rustc``
Build: release, ``$([System.IO.Path]::GetFileName($exe))``
SMTC: $(if ($NoSmtc) { "disattivato (MUSIFY_NO_SMTC=1)" } else { "attivo" })
Metodo: 10 avvii, segnale ``RUNE_READY_DIR``, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di ``on_ready``; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | $(Format-Stat $idleReady " ms") |
| Startup | tempo processo → ready | $(Format-Stat $idleWall " ms") |
| Riposo | working set | $(Format-Stat $idleWs " MB") |
| Riposo | private bytes | $(Format-Stat $idlePrivate " MB") |
| Riposo | CPU un core | $(Format-Stat $idleCpu " %") |
| Riproduzione | working set | $(Format-Stat $playWs " MB") |
| Riproduzione | private bytes | $(Format-Stat $playPrivate " MB") |
| Riproduzione | CPU un core | $(Format-Stat $playCpu " %") |
| Riproduzione | CPU normalizzata | $(Format-Stat $playCpuNorm " %") |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

"@

for ($i = 0; $i -lt $idleRuns.Count; $i++) {
    $r = $idleRuns[$i]
    $markdown += "| $($i + 1) | $($r.ReadyMs) | $([math]::Round($r.WallMs, 1)) | $($r.WorkingSetMb) | $($r.PrivateMb) | $($r.CpuOneCorePct) |`n"
}

$markdown += @"

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |

"@

for ($i = 0; $i -lt $playRuns.Count; $i++) {
    $r = $playRuns[$i]
    $markdown += "| $($i + 1) | $($r.ReadyMs) | $([math]::Round($r.WallMs, 1)) | $($r.WorkingSetMb) | $($r.PrivateMb) | $($r.CpuOneCorePct) | $($r.CpuNormalizedPct) |`n"
}

$markdown += @"

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa ``MUSIFY_AUTOPLAY=1`` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso ``-NoSmtc`` e lo stesso numero di run.
"@

New-Item -ItemType Directory -Force -Path (Split-Path $outPath -Parent) | Out-Null
$markdown | Set-Content -Path $outPath -Encoding utf8
$all = @()
$all += $idleRuns | Add-Member -NotePropertyName Phase -NotePropertyValue "startup" -PassThru
$all += $playRuns | Add-Member -NotePropertyName Phase -NotePropertyValue "playback" -PassThru
$all | Export-Csv -Path $csvPath -NoTypeInformation -Encoding utf8

Write-Host ""
Write-Host "Scritto: $outPath"
Write-Host "Scritto: $csvPath"
Write-Host ("Riposo: ready {0:N0} ms, WS {1:N2} MB, private {2:N2} MB, CPU {3:N2}%" -f (Get-Median $idleReady), (Get-Median $idleWs), (Get-Median $idlePrivate), (Get-Median $idleCpu))
Write-Host ("Riproduzione: WS {0:N2} MB, private {1:N2} MB, CPU {2:N2}% un core, {3:N2}% normalizzata" -f (Get-Median $playWs), (Get-Median $playPrivate), (Get-Median $playCpu), (Get-Median $playCpuNorm))
