# Musify

Player musicale desktop personale per Windows, scritto in Rust. Interfaccia scura ispirata a
Spotify, ma con identità propria; sorgente principale la musica locale, Spotify solo come
milestone opzionale dietro feature flag.

## Stato

| Milestone | Stato |
| --- | --- |
| M0 — Workspace, CI locale, smoke test Rune | completata |
| M1 — Motore audio | completata |
| M2 — Libreria | da fare |
| M3 — UI Rune | da fare |
| M4 — Integrazione Windows e rifinitura | da fare |
| M5 — Spotify (opzionale) | da fare |

Report: `docs/M0-RUNE-REPORT.md`, `docs/M1-AUDIO-REPORT.md`.

## Requisiti (Windows)

- Windows 10/11 x86_64.
- [rustup](https://rustup.rs/) con toolchain MSVC: il repository fissa Rust 1.99.1 in
  `rust-toolchain.toml` (la stessa versione richiesta da Rune).
- Visual Studio Build Tools con il workload "Desktop development with C++" (linker MSVC).
- Driver GPU con supporto DirectX 12 (Rune usa wgpu).
- Per rigenerare gli asset di test: `ffmpeg` nel `PATH`.

## Compilare ed eseguire

```powershell
cargo build
cargo run -p musify
```

Controlli locali equivalenti alla CI:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\ci.ps1
```

## Motore audio da riga di comando

```powershell
cargo run -p musify-audio --bin audio-cli -- `
  crates/audio/tests/assets/tone.mp3 `
  crates/audio/tests/assets/tone.flac `
  crates/audio/tests/assets/tone.ogg
```

Comandi da stdin: `pause`, `resume`, `toggle`, `next`, `prev`, `seek 12.5`,
`vol 70`, `status`, `quit`. Gli asset di test si rigenerano con
`scripts/make-test-assets.ps1` (richiede ffmpeg).

## Struttura

```text
crates/core      tipi di dominio, Command/Event, trait PlaybackSource e LibraryProvider
crates/audio     decodifica e output audio, coda, seek, volume
crates/library   scanner, SQLite, ricerca, cache copertine
crates/ui        presentazione Rune, nessuna logica di dominio
crates/spotify   sorgente Spotify opzionale (feature `spotify`)
crates/app       binario, configurazione, cablaggio
```

## Dati dell'applicazione

- Configurazione: `%APPDATA%\Musify\config.toml`
- Stato di riproduzione: `%APPDATA%\Musify\state.json`
- Libreria: `%APPDATA%\Musify\library.sqlite3`
- Cache copertine: `%APPDATA%\Musify\covers\`

## Note legali

Musify è un progetto personale. Non aggira DRM, non registra e non distribuisce flussi
Spotify. L'eventuale uso di `librespot` per riprodurre contenuti Spotify può non essere in
linea con i termini di servizio di Spotify: valuta tu se usarlo sul tuo account Premium.
Nessun token o segreto viene salvato nel repository o nei log.
