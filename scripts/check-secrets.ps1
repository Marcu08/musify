# Scans files staged for commit and fails if it finds likely secrets.
# It prints only file names and line numbers, never the matched value.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $staged = @(git diff --cached --name-only --diff-filter=ACMR)
    if ($staged.Count -eq 0) {
        Write-Host "check-secrets: nessun file staged"
        exit 0
    }

    $key = $env:MUSIFY_JAMENDO_CLIENT_ID
    $findings = [System.Collections.Generic.List[string]]::new()
    $binaryExtensions = @(
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".bmp",
        ".mp3", ".flac", ".wav", ".ogg", ".m4a", ".mp4",
        ".sqlite3", ".db", ".exe", ".dll", ".pdb", ".rlib", ".rmeta"
    )

    foreach ($file in $staged) {
        $normalized = $file.Replace("\", "/")
        $extension = [System.IO.Path]::GetExtension($normalized).ToLowerInvariant()
        if ($binaryExtensions -contains $extension) {
            continue
        }
        if ($normalized -match "(^|/)\.env($|\.)") {
            $findings.Add("$file (file .env)")
            continue
        }
        if ($normalized -match "\.(sqlite3|db|pem|key|p12|pfx)$") {
            $findings.Add("$file (file sensibile)")
            continue
        }

        $lines = @(git show ":$file" 2>$null)
        if ($LASTEXITCODE -ne 0) {
            continue
        }
        for ($index = 0; $index -lt $lines.Count; $index++) {
            $line = [string]$lines[$index]
            if (-not [string]::IsNullOrEmpty($key) -and $line.Contains($key)) {
                $findings.Add("${file}:$($index + 1) (client id Jamendo)")
                continue
            }
            if ($line -match '(?i)\b(api[_-]?key|secret|token|password|passwd|client[_-]?secret)\b\s*[:=]\s*["'']?[A-Za-z0-9_\-\.]{8,}') {
                $findings.Add("${file}:$($index + 1) (possibile segreto)")
                continue
            }
            if ($normalized -notmatch "(^|/)Cargo\.lock$" -and (
                $line -match "\b[0-9a-fA-F]{64,}\b" -or
                $line -match "\b[A-Za-z0-9+/]{48,}={0,2}\b"
            )) {
                $findings.Add("${file}:$($index + 1) (stringa lunga tipo chiave)")
            }
        }
    }

    if ($findings.Count -gt 0) {
        Write-Host "check-secrets: potenziali segreti trovati (valori non mostrati):"
        $findings | Sort-Object -Unique | ForEach-Object { Write-Host "  $_" }
        exit 1
    }

    Write-Host "check-secrets: nessun segreto rilevato nei file staged"
    exit 0
} finally {
    Pop-Location
}
