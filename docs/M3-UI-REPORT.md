# M3 — UI Rune collegata al core

Data: 2026-11-09
Crate UI: `musify-ui`
Binario: `musify` (`crates/app`)

## Cosa è stato fatto

- UI scura Rune con identità propria (accento ambra, non il verde Spotify):
  sidebar, ricerca, viste Brani/Album/Artisti, coda, impostazioni, barra
  "in riproduzione".
- Liste virtualizzate (`virtual_rows`) per brani, album, artisti e coda: solo
  le righe visibili vengono costruite.
- Ricerca FTS5 collegata alla UI; risultati divisi in brani/album/artisti.
- Copertine caricate on demand con `ImageCache` di Rune e cache locale già
  prodotta dalla libreria (JPEG 512 px).
- Coda modificabile: rimozione, spostamento su/giù, svuotamento, click per
  riprodurre.
- Barra di riproduzione: play/pausa, precedente/successivo, seek, volume,
  shuffle, repeat, copertina, titolo/artista.
- Scelta cartelle musicali con dialogo nativo Rune (`native-file-dialog`);
  watcher e scansione incrementale collegati al backend.
- Scorciatoie: Spazio, ←/→, Ctrl+←/→, Ctrl+F, Ctrl+R, Ctrl+Q, Esc, +/−.
- Tutte le stringhe italiane sono in `crates/ui/src/i18n.rs`; i colori passano
  da `crates/ui/src/theme.rs` e dai token Rune, nessun colore hardcoded nei
  componenti.
- Backend disaccoppiato: la UI conosce solo il trait `ui::Backend`; audio,
  SQLite, cpal e Rune non si attraversano.
- Waker Rune usato per svegliare la UI solo quando arrivano eventi (posizione,
  coda, scansione, errori).

## Comandi

```powershell
cargo run -p musify
# oppure
cargo build -p musify --release
.\target\release\musify.exe
```

La cartella dati predefinita è `%APPDATA%\Musify`. Per test isolati si possono
usare `MUSIFY_DATA_DIR`, `MUSIFY_MUSIC_DIRS` (separate da `;`) e
`MUSIFY_NO_AUDIO=1`.

## Verifiche funzionali

- Avvio finestra reale su Windows 11/wgpu-DX12.
- Libreria di prova caricata, lista brani virtualizzata, ricerca.
- Riproduzione avviata dalla UI: brano corrente e posizione nella barra,
  `Pausa` visibile, seek con ←/→ (posizione passata da 0:02 a 0:12).
- Coda, impostazioni, album, artisti e ricerca verificati con snapshot headless
  e con il backend reale.
- Nessun crash durante le prove; scansione e watcher attivi in background.

Screenshot: `docs/evidence/musify-tracks.png`, `musify-queue.png`,
`musify-settings.png`, `musify-search.png`, `musify-albums.png`,
`musify-artists.png`, `musify-playing.png`.

Gli snapshot UI vengono da `cargo run -p musify-ui --example ui_snapshot --
--snap ... --scene ...`; il backend reale è stato provato con una libreria di
file FLAC generati e configurata via `MUSIFY_DATA_DIR`/`MUSIFY_MUSIC_DIRS`.

## Misure (release, Windows 11, Ryzen 5 5625U, 12 thread)

| Metrica | Valore | Obiettivo indicativo |
| --- | --- | --- |
| Avvio fino al segnale `on_ready` (`RUNE_READY_DIR`) | **663 ms** (cache calda) | < 300 ms |
| RAM a riposo (working set) | **176,2 MB** | < 80 MB |
| CPU a riposo (5 s, finestra immobile) | **0,00 %** | zero redraw / wake |
| RAM in riproduzione | **182,5 MB** | — |
| CPU in riproduzione (un core) | **7,17 %** | < 2 % |
| CPU in riproduzione (normalizzata su 12 thread) | **0,60 %** | < 2 % |

Lettura: la UI a riposo non ridisegna (CPU 0,00%), quindi l'obiettivo "zero
wake-up quando nulla cambia" è rispettato. L'avvio e la RAM **non** centrano gli
obiettivi indicativi: Rune/wgpu e la UI costano più di 300 ms e più di 80 MB su
questa macchina. La CPU in riproduzione è 7,17% di un core ma 0,60% del sistema;
il target < 2% è rispettato solo nella lettura normalizzata. Questi numeri sono
reali e non vengono arrotondati a favore del progetto.

## Problemi noti

- Su Windows con DPI 125% la prima finestra viene creata con il fattore di scala
  iniziale sbagliato; al primo `on_ready` viene ridimensionata/riposizionata per
  far coincidere viewport e finestra nativa. È un workaround documentato in
  `crates/ui/src/lib.rs`.
- Il target di avvio < 300 ms non è raggiunto; il grosso è inizializzazione
  wgpu/shader e prima build della UI.
- Il target RAM < 80 MB non è raggiunto. Rune (wgpu, font, atlas, accessibilità)
  è la parte dominante; l'audio e la libreria da soli stavano in ~16 MB.
- La UI è stata provata solo su Windows; il crate `ui` è comunque separato e
  sostituibile.
- Il dialogo nativo delle cartelle usa `rfd` tramite Rune; non è stato possibile
  automatizzarlo in modo affidabile con l'automazione desktop usata per gli
  altri test. La configurazione via TOML/`MUSIFY_MUSIC_DIRS` è verificata.
