param(
    [string]$RadioUrl = "https://mangoradio.stream.laut.fm/mangoradio",
    [double]$Hours = 2,
    [int]$IntervalSeconds = 60,
    [string]$Output = "docs/M51-SOAK.md",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$dataDir = Join-Path $root "testdata\soak"
$readyDir = Join-Path $dataDir "ready"
$exe = Join-Path $root "target\release\musify.exe"
$outPath = Join-Path $root $Output
$csvPath = [System.IO.Path]::ChangeExtension($outPath, ".csv")

if (Get-Process musify -ErrorAction SilentlyContinue) {
    throw "Chiudi tutte le istanze di Musify prima della prova lunga."
}

if (-not $SkipBuild) {
    Push-Location $root
    try { cargo build -p musify --release } finally { Pop-Location }
}
if (-not (Test-Path $exe)) { throw "Manca $exe" }

New-Item -ItemType Directory -Force -Path $dataDir, $readyDir | Out-Null
Get-ChildItem $readyDir -Force -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
$env:MUSIFY_DATA_DIR = (Resolve-Path $dataDir).Path
$env:RUNE_READY_DIR = (Resolve-Path $readyDir).Path
$env:MUSIFY_AUTOPLAY_RADIO = $RadioUrl
Remove-Item Env:MUSIFY_AUTOPLAY -ErrorAction SilentlyContinue

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class SoakWindow {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
}
"@

function Focus-App($process) {
    $process.Refresh()
    $handle = $process.MainWindowHandle
    if ($handle -eq 0) { return }
    [void][SoakWindow]::ShowWindow($handle, 9)
    [SoakWindow]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
    [void][SoakWindow]::SetForegroundWindow($handle)
    [SoakWindow]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
}

$log = Join-Path $dataDir "soak.out.log"
$err = Join-Path $dataDir "soak.err.log"
$p = Start-Process -FilePath $exe -WorkingDirectory $root -PassThru `
    -RedirectStandardOutput $log -RedirectStandardError $err

# Wait for the first frame, then focus the window so it really paints.
for ($i = 0; $i -lt 1500; $i++) {
    Start-Sleep -Milliseconds 10
    if (Get-ChildItem $readyDir -File -Filter "musify.exe.*" -ErrorAction SilentlyContinue |
        Where-Object { $_.Extension -ne ".tmp" }) { break }
}
Start-Sleep -Seconds 2
Focus-App $p

$samples = @()
$started = Get-Date
$until = $started.AddHours($Hours)
$previousCpu = $null
$previousTime = $null
$index = 0

while ((Get-Date) -lt $until -and -not $p.HasExited) {
    $remaining = $IntervalSeconds
    while ($remaining -gt 0) {
        Focus-App $p
        $step = [math]::Min(5, $remaining)
        Start-Sleep -Seconds $step
        $remaining -= $step
        $p.Refresh()
        if ($p.HasExited) { break }
    }
    if ($p.HasExited) { break }
    $now = Get-Date
    $cpu = $p.CPU
    $cpuPct = if ($null -ne $previousCpu) {
        $wall = ($now - $previousTime).TotalSeconds
        if ($wall -gt 0) { 100.0 * ($cpu - $previousCpu) / $wall } else { 0 }
    } else { 0 }
    $previousCpu = $cpu
    $previousTime = $now
    $index++
    $sample = [pscustomobject]@{
        Index = $index
        Timestamp = $now.ToString("yyyy-MM-dd HH:mm:ss")
        ElapsedMinutes = [math]::Round(($now - $started).TotalMinutes, 2)
        WorkingSetMb = [math]::Round($p.WorkingSet64 / 1MB, 2)
        PrivateMb = [math]::Round($p.PrivateMemorySize64 / 1MB, 2)
        CpuOneCorePct = [math]::Round($cpuPct, 2)
        Threads = $p.Threads.Count
        Handles = $p.HandleCount
    }
    $samples += $sample
    $samples | Export-Csv -Path $csvPath -NoTypeInformation -Encoding utf8
    Write-Host ("[{0}] {1:N0} min · WS {2:N2} MB · private {3:N2} MB · CPU {4:N2}%" -f `
        $sample.Timestamp, $sample.ElapsedMinutes, $sample.WorkingSetMb, $sample.PrivateMb, $sample.CpuOneCorePct)
}

$exited = $p.HasExited
if (-not $exited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }

function Summary([object[]]$values) {
    if ($values.Count -eq 0) { return @{ First = 0; Last = 0; Min = 0; Max = 0; Growth = 0 } }
    $first = $values[0]
    $last = $values[-1]
    return @{
        First = $first
        Last = $last
        Min = ($values | Measure-Object -Minimum).Minimum
        Max = ($values | Measure-Object -Maximum).Maximum
        Growth = $last - $first
    }
}

$ws = Summary @($samples | ForEach-Object { $_.WorkingSetMb })
$private = Summary @($samples | ForEach-Object { $_.PrivateMb })
$cpuValues = @($samples | ForEach-Object { $_.CpuOneCorePct })
$cpuMedian = if ($cpuValues.Count -gt 0) {
    $sorted = $cpuValues | Sort-Object
    $sorted[[int][math]::Floor($sorted.Count / 2)]
} else { 0 }
$cpuSummary = Summary $cpuValues
$firstTimestamp = if ($samples.Count -gt 0) { $samples[0].Timestamp } else { "n/d" }
$lastTimestamp = if ($samples.Count -gt 0) { $samples[-1].Timestamp } else { "n/d" }
$lastElapsed = if ($samples.Count -gt 0) { $samples[-1].ElapsedMinutes } else { 0 }
$exitNote = if ($exited) { "Il processo è terminato prima della fine." } else { "Processo chiuso dallo script al termine." }

$markdown = @"
# M5.1 — Prova lunga radio

Data: $($started.ToString("yyyy-MM-dd HH:mm:ss"))
Stazione: $RadioUrl
Durata richiesta: $Hours h · campione ogni $IntervalSeconds s
Campioni: $($samples.Count) · da $firstTimestamp a $lastTimestamp ($lastElapsed min)
$exitNote

## Crescita

| Metrica | Primo | Ultimo | Min | Max | Crescita |
| --- | ---: | ---: | ---: | ---: | ---: |
| Working set MB | $($ws.First) | $($ws.Last) | $($ws.Min) | $($ws.Max) | $($ws.Growth) |
| Private bytes MB | $($private.First) | $($private.Last) | $($private.Min) | $($private.Max) | $($private.Growth) |
| CPU un core % | $($cpuSummary.First) | $($cpuSummary.Last) | $($cpuSummary.Min) | $($cpuSummary.Max) | $($cpuSummary.Growth) |

CPU mediana: $cpuMedian% di un core.

## Note

- La finestra viene portata in primo piano dopo il primo frame.
- Il processo è chiuso forzatamente alla fine; non è una prova di chiusura pulita.
- Il CSV grezzo ($([System.IO.Path]::GetFileName($csvPath))) contiene ogni campione.
- Una crescita monotona e piccola può essere cache del decoder o frammentazione;
  una crescita continua e lineare dopo la prima ora è un possibile leak.
"@

New-Item -ItemType Directory -Force -Path (Split-Path $outPath -Parent) | Out-Null
$markdown | Set-Content -Path $outPath -Encoding utf8
Write-Host ""
Write-Host "Scritto: $outPath"
Write-Host "Scritto: $csvPath"
Write-Host ("WS {0:N2} -> {1:N2} MB ({2:N2}); private {3:N2} -> {4:N2} MB ({5:N2}); CPU mediana {6:N2}%" -f `
    $ws.First, $ws.Last, $ws.Growth, $private.First, $private.Last, $private.Growth, $cpuMedian)
