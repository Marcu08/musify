# Measure clean and incremental builds for the fast and release profiles.
# Clean builds use isolated target directories, so the normal target/ cache and
# the source tree are not touched. Run a release build beforehand if you want
# the reproducibility hash comparison:
#   cargo build -p musify --release
[CmdletBinding()]
param(
    [string]$Output = "docs/M8-BUILDS.md",
    [string]$WorkRoot = "target/m8-build"
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$work = Join-Path $root $WorkRoot
if (Test-Path -LiteralPath $work) {
    Remove-Item -LiteralPath $work -Recurse -Force
}
$logDir = Join-Path $work "logs"
New-Item -ItemType Directory -Force -Path $logDir | Out-Null

function Invoke-Build([string]$Label, [string]$TargetDir, [string[]]$CargoArgs) {
    $log = Join-Path $logDir "$Label.log"
    $arguments = $CargoArgs + @("--target-dir", $TargetDir)
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    & cargo @arguments *> $log
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previous
    $stopwatch.Stop()
    if ($exitCode -ne 0) {
        Get-Content $log -Tail 80
        throw "Build fallita: $Label"
    }
    return [pscustomobject]@{
        Label = $Label
        Seconds = [math]::Round($stopwatch.Elapsed.TotalSeconds, 1)
    }
}

function Touch-Main {
    (Get-Item (Join-Path $root "crates/app/src/main.rs")).LastWriteTime = Get-Date
}

$fastDir = Join-Path $work "fast-clean"
$releaseDir = Join-Path $work "release-clean"

$fastClean = Invoke-Build "fast-clean" $fastDir @("build", "-p", "musify", "--profile", "fast")
$fastExe = Join-Path $fastDir "fast/musify.exe"
$fastSize = (Get-Item $fastExe).Length
$fastHash = (Get-FileHash $fastExe -Algorithm SHA256).Hash
$artifactDir = Join-Path $work "artifacts"
New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
Copy-Item -LiteralPath $fastExe -Destination (Join-Path $artifactDir "musify-fast.exe") -Force
Touch-Main
$fastIncremental = Invoke-Build "fast-incremental" $fastDir @("build", "-p", "musify", "--profile", "fast")

$releaseClean = Invoke-Build "release-clean" $releaseDir @("build", "-p", "musify", "--release")
$releaseExe = Join-Path $releaseDir "release/musify.exe"
$releaseSize = (Get-Item $releaseExe).Length
$releaseHash = (Get-FileHash $releaseExe -Algorithm SHA256).Hash
Copy-Item -LiteralPath $releaseExe -Destination (Join-Path $artifactDir "musify-release.exe") -Force
Touch-Main
$releaseIncremental = Invoke-Build "release-incremental" $releaseDir @("build", "-p", "musify", "--release")

$mainReleaseExe = Join-Path $root "target/release/musify.exe"
$mainReleaseHash = if (Test-Path -LiteralPath $mainReleaseExe) {
    (Get-FileHash $mainReleaseExe -Algorithm SHA256).Hash
} else {
    "n/d (nessuna build in target/release)"
}
$reproducible = if ($mainReleaseHash -eq $releaseHash) { "si" } else { "no" }

$os = Get-CimInstance Win32_OperatingSystem
$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name).Trim()
$commit = (git rev-parse --short HEAD 2>$null)
$rustc = (rustc --version)

$markdown = @"
# M8 - Misure di compilazione

Data: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
Commit: $commit
Macchina: $cpu - $($os.Caption) $($os.Version)
Rust: $rustc
Metodo: build clean in target directory isolate, poi touch di
crates/app/src/main.rs e rebuild incrementale. Le directory sono
$WorkRoot. Il target normale non viene pulito.

| Profilo | Build clean | Build incrementale | Exe | SHA-256 clean |
| --- | ---: | ---: | ---: | --- |
| fast | $($fastClean.Seconds) s | $($fastIncremental.Seconds) s | $([math]::Round($fastSize / 1MB, 2)) MB | $fastHash |
| release | $($releaseClean.Seconds) s | $($releaseIncremental.Seconds) s | $([math]::Round($releaseSize / 1MB, 2)) MB | $releaseHash |

## Riproducibilita

- target/release/musify.exe SHA-256: $mainReleaseHash
- Build release clean SHA-256: $releaseHash
- Uguali: **$reproducible**

La build clean e quella nel target normale usano directory diverse ma lo stesso
sorgente e le stesse opzioni. Se gli hash differiscono ma la dimensione e il
contenuto sono uguali, confrontare i metadati PE (TimeDateStamp e GUID di debug):
il resource compiler MSVC puo aggiungere timestamp non deterministici.
"@

$outPath = Join-Path $root $Output
New-Item -ItemType Directory -Force -Path (Split-Path $outPath -Parent) | Out-Null
[System.IO.File]::WriteAllText($outPath, $markdown, [System.Text.UTF8Encoding]::new($false))
Write-Host "Scritto: $outPath"
Write-Host ("fast: clean {0} s, incrementale {1} s, {2:N2} MB" -f $fastClean.Seconds, $fastIncremental.Seconds, ($fastSize / 1MB))
Write-Host ("release: clean {0} s, incrementale {1} s, {2:N2} MB" -f $releaseClean.Seconds, $releaseIncremental.Seconds, ($releaseSize / 1MB))
Write-Host ("riproducibile: $reproducible")
