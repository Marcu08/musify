[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [ValidateSet("release", "fast")]
    [string]$Profile = "release",
    [string]$Source = "",
    [string]$InstallDir = ""
)

$ErrorActionPreference = "Stop"

$root = Split-Path $PSScriptRoot -Parent
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $localAppData = [Environment]::GetFolderPath("LocalApplicationData")
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        $localAppData = $env:LOCALAPPDATA
    }
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        throw "Cartella LocalAppData non disponibile."
    }
    $InstallDir = Join-Path $localAppData "Musify"
}
if ([string]::IsNullOrWhiteSpace($Source)) {
    $Source = Join-Path $root "target\$Profile\musify.exe"
}

if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
    throw "Eseguibile non trovato: $Source. Compila prima con: cargo build -p musify --profile $Profile"
}
$sourceExe = (Resolve-Path -LiteralPath $Source).Path
$targetExe = Join-Path $InstallDir "musify.exe"

if ($PSCmdlet.ShouldProcess($targetExe, "Copia Musify")) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    try {
        Copy-Item -LiteralPath $sourceExe -Destination $targetExe -Force
    } catch {
        throw "Impossibile aggiornare $targetExe. Chiudi Musify e riprova. Dettaglio: $($_.Exception.Message)"
    }
}

function New-MusifyShortcut([string]$Path) {
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($Path)
    $shortcut.TargetPath = $targetExe
    $shortcut.WorkingDirectory = $InstallDir
    $shortcut.IconLocation = "$targetExe,0"
    $shortcut.Description = "Musify - il tuo player musicale desktop"
    $shortcut.Save()
}

$desktop = [Environment]::GetFolderPath("Desktop")
$programs = [Environment]::GetFolderPath("Programs")
if ([string]::IsNullOrWhiteSpace($desktop) -or [string]::IsNullOrWhiteSpace($programs)) {
    throw "Cartelle Desktop o menu Start non disponibili."
}
$desktopLink = Join-Path $desktop "Musify.lnk"
$startLink = Join-Path $programs "Musify.lnk"

if ($PSCmdlet.ShouldProcess($desktopLink, "Crea collegamento desktop")) {
    New-MusifyShortcut $desktopLink
}
if ($PSCmdlet.ShouldProcess($startLink, "Crea collegamento menu Start")) {
    New-MusifyShortcut $startLink
}

if ($WhatIfPreference) {
    Write-Host "WhatIf: nessuna modifica effettuata."
    return
}

Write-Host "Musify installato in: $targetExe"
Write-Host "Collegamento desktop: $desktopLink"
Write-Host "Collegamento menu Start: $startLink"
$appData = [Environment]::GetFolderPath("ApplicationData")
if ([string]::IsNullOrWhiteSpace($appData)) { $appData = $env:APPDATA }
Write-Host "Dati utente non modificati: $(Join-Path $appData 'Musify')"
Write-Host "Per aggiornare, ricompila e riesegui questo script."
