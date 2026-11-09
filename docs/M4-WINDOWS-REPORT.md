# M4 — Integrazione Windows, persistenza e rifinitura

Data: 2026-11-09
Piattaforma: Windows 11 10.0.26300, Ryzen 5 5625U, 12 thread, DPI 125%.

## Cosa è stato fatto

### SMTC e tasti multimediali

- Integrazione **souvlaki 0.9.3** con `SystemMediaTransportControls`.
- L'HWND viene ottenuto da Rune in `on_ready` tramite
  `WindowHandle::with_raw_handle` e passato al backend; nessun HWND inventato o
  cercato per titolo.
- Metadati inviati a Windows: titolo, artista, album, durata e copertina
  (`file://` verso la cache JPEG).
- Playback status e posizione aggiornati a circa 1 Hz (non a ogni frame).
- Eventi SMTC collegati ai comandi audio: Play, Pause, Toggle, Stop, Next,
  Previous, Seek ±10 s e SetPosition.
- Verifica reale: il tasto media Play/Pause ha generato l'overlay di Windows
  (titolo `Long Tone`) e ha messo in pausa/ripreso Musify; il tasto Next ha
  generato l'evento `Next` nei log. Evidenza:
  `docs/evidence/musify-smtc.png`.

### Persistenza

- `config.toml` in `%APPDATA%\Musify\`: cartelle musicali, volume, shuffle,
  repeat. Scrittura atomica (`config.toml.tmp` → rename).
- `state.json` in `%APPDATA%\Musify\`: coda come elenco di path, indice del
  brano corrente, posizione in millisecondi. Scrittura atomica e throttle di 2 s;
  salvataggio forzato alla chiusura.
- All'avvio la coda viene risolta di nuovo tramite la libreria (i file spariti
  vengono saltati) e ripristinata **in pausa** alla posizione salvata, senza far
  partire l'audio.
- Verifica reale: riproduzione, pausa a 0:01, chiusura con Ctrl+Q, riapertura;
  la UI mostra la stessa coda e `Long Tone` in pausa a 0:01. Evidenza:
  `docs/evidence/musify-restored.png`.

### Rifinitura

- Scorciatoie finali: Spazio, ←/→ seek, Ctrl+←/→ precedente/successivo,
  Ctrl+F ricerca, Ctrl+R riscansione, +/− volume, Esc, Ctrl+Q.
- Dialogo nativo per aggiungere cartelle; rimozione cartelle; riscansione
  manuale; stato scansione in sidebar.
- Errori audio/libreria mostrati nella UI, non solo nei log.
- README e ARCHITECTURE aggiornati.

## Comandi di verifica

```powershell
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build -p musify --release
.\target\release\musify.exe
```

Variabili utili per test isolati: `MUSIFY_DATA_DIR`, `MUSIFY_MUSIC_DIRS`
(separate da `;`), `MUSIFY_NO_AUDIO=1`, `MUSIFY_NO_SMTC=1`, `RUNE_READY_DIR`.

## Stato dei test

- `cargo test --workspace --all-features`: **36 test passati** (7 core,
  14 audio, 14 library, 1 spotify, più i binari senza test).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pulito.
- `cargo fmt --all -- --check`: pulito.

## Misure finali (release)

Le misure di avvio hanno alta varianza su questa macchina (Windows Defender,
cache shader, processi concorrenti). La misura M3 più favorevole era 0,66 s; con
l'app completa M4 (libreria, audio, SMTC, ripristino coda) le misure sono state
1,0–2,7 s, mediana ~1,5 s. Un avvio vuoto senza audio/libreria/SMTC è stato
~1,0 s. Un outlier sotto riavvii ravvicinati ha toccato 9,2 s; non è
rappresentativo ma è riportato.

| Metrica | Valore | Obiettivo indicativo |
| --- | --- | --- |
| Avvio fino a `on_ready` (app completa) | **~1,0–2,7 s**, mediana ~1,5 s | < 0,30 s |
| Avvio vuoto (no audio/libreria/SMTC) | **~1,0 s** | < 0,30 s |
| RAM a riposo | **~176 MB** | < 80 MB |
| CPU a riposo, 5 s | **0,00 %** | zero redraw |
| RAM in riproduzione | **~182 MB** | — |
| CPU in riproduzione, un core | **~7,2 %** | < 2 % |
| CPU in riproduzione, normalizzata 12 thread | **~0,60 %** | < 2 % |

I target di avvio e RAM non sono centrati; sono riportati senza arrotondamenti.
La UI a riposo non ridisegna. La CPU in riproduzione è accettabile sul sistema
(0,60%) ma non sul singolo core (7,2%) a causa dei redraw della barra di
posizione e dell'audio.

## Problemi noti

- SMTC è stato provato su Windows 11 con DPI 125%. Su altri DPI il workaround di
  resize al primo `on_ready` è lo stesso già documentato in M3.
- I tasti multimediali dipendono dalla sessione SMTC attiva: se un altro player
  diventa la sessione preferita da Windows, i tasti possono andare a quello.
- La copertina SMTC usa il JPEG della cache; se non esiste una copertina,
  Windows mostra il placeholder dell'app.
- M5 (Spotify) non è implementata: il crate `musify-spotify` è solo lo scheletro
  compilabile dietro feature. L'uso di librespot va valutato rispetto ai termini
  di servizio di Spotify, come scritto nel README.
- Il target di avvio < 300 ms e quello RAM < 80 MB restano aperti; un eventuale
  Piano B UI andrebbe valutato con gli stessi numeri.
