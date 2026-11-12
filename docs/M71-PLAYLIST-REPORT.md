# M7.1 — Playlist

Data: 2026-11-11
Base: commit `aca8763` (M6.1)
Build: release, Rune invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`
Crate modificati: `musify-core`, `musify-library`, `musify-ui`, `musify-app`
Crate nuovi: nessuno

## 1. Schema e migrazione

`Library::init_schema` chiama ora `migrate_playlists`, che porta
`PRAGMA user_version` a **2** e crea:

- `playlists(id, name, created_at, updated_at)`;
- `playlist_items(id, playlist_id, position, source_scheme, item_id, title,
  artist, album, duration_ms, cover_path)`, con foreign key verso `playlists`,
  `UNIQUE(playlist_id, position)` e indice `(playlist_id, position)`.

La migrazione è versionata e idempotente. Se trova la vecchia tabella
`playlist_tracks`, copia le righe in `playlist_items` come snapshot locali
(`source_scheme = 'local'`, `item_id = tracks.path`) e poi elimina la tabella
legacy. I dati esistenti non vengono persi; una riapertura successiva non
ricrea `playlist_tracks` e non duplica gli elementi.

Gli elementi sono generici per fonte: per i brani locali `item_id` è il path,
per le fonti future è l'URI della fonte. La disponibilità **non** è persistita:
viene calcolata alla lettura (`Path::is_file()` per `local`, sempre vera per le
altre fonti). Questo permette di mostrare un file mancante senza perdere la
riga.

## 2. Operazioni e backend

`musify-library` espone:

- `playlists`, `playlist`, `create_playlist`, `rename_playlist`,
  `delete_playlist`;
- `playlist_items`, `add_tracks_to_playlist`, `remove_playlist_item`,
  `move_playlist_item`, `remove_missing_playlist_items`;
- `playlist_tracks`, `save_queue_as_playlist`;
- `import_m3u8`, `export_m3u8`.

`add_tracks_to_playlist` filtra radio e podcast sia nella UI sia nella libreria,
così una chiamata futura non può inserirli per errore. La cancellazione di una
playlist è a cascata sugli elementi; la rimozione di un elemento ricompatta le
posizioni.

Il trait `ui::Backend` espone i metodi playlist con default, quindi i backend di
test restano compilabili. `AppBackend` li inoltra alla libreria. Per la
riproduzione, `playlist_playback_command` costruisce un `Command::PlayTracks`
con i soli elementi disponibili: i file mancanti vengono saltati senza errori.
Se non resta nulla, l'app emette un `Event::Error` leggibile invece di avviare
una coda vuota. `enqueue_playlist` usa la stessa selezione e lo stesso messaggio
quando non ci sono brani disponibili.

Gli id dei `Track` costruiti da una playlist sono negativi (`-PlaylistItemId`):
non possono coincidere con gli id della libreria e quindi non accendono per
errore l'evidenziazione "in riproduzione" di un altro brano.

## 3. File mancanti

- La riga resta visibile, in grigio, con `File non trovato` nella riga dei
  metadati.
- `Riproduci playlist` e `Aggiungi alla coda` saltano gli elementi non
  disponibili.
- `Rimuovi mancanti` elimina in blocco gli elementi non disponibili e
  ricompatta le posizioni.
- Test: `missing_items_stay_visible_and_are_skipped_in_playback`.

## 4. M3U8

Export:

- UTF-8, intestazione `#EXTM3U`;
- per ogni elemento `#EXTINF:<secondi>,<artista> - <titolo>` e la posizione;
- path relativo quando il file è canonicalizzabile sotto la cartella
  dell'export, altrimenti path assoluto memorizzato.

Import:

- legge il file come UTF-8, risolve i path relativi rispetto alla cartella del
  file M3U8;
- interpreta `#EXTINF` (durata, artista, titolo), con fallback al nome del file
  quando il titolo manca;
- ignora le righe con schema URI (`http://`, `https://`, `jamendo://`, …) invece
  di trattarle come path locali;
- crea una nuova playlist con il nome dello stem del file.

Test: `m3u8_round_trip_uses_utf8_metadata_and_relative_paths` (metadati non
ASCII, durata, path relativo) e `m3u8_ignores_stream_uris`.

## 5. UI

- Nuova voce **Playlist** nella barra laterale, sotto Raccolta.
- Vista con pannello playlist (nome, Crea, elenco, Salva coda come playlist,
  Importa M3U8) e dettaglio con lista virtualizzata.
- Azioni: Riproduci playlist, Aggiungi alla coda, Rinomina, Elimina, Esporta
  M3U8, Aggiungi brano corrente, Aggiungi coda, Rimuovi mancanti.
- Aggiunta da libreria, ricerca e coda tramite il pulsante `+` sulle righe;
  aggiunta dalla riproduzione corrente tramite l'azione dedicata.
- Riordino con pulsanti ↑/↓ e `Alt+↑`/`Alt+↓` sull'elemento selezionato. Il
  drag del mouse non è usato.
- Stringhe solo in `i18n.rs`, ruoli colore solo in `theme.rs`.
- Snapshot headless della vista: `docs/evidence/m71-playlists.png`. La stessa
  vista è stata renderizzata con il backend reale in release
  (`docs/evidence/m71-playlists-app.png`), senza audio e SMTC. I pulsanti `+`
  nelle viste Brani e Ricerca sono verificati dagli snapshot
  `docs/evidence/m71-tracks-add.png` e `docs/evidence/m71-search-add.png`.

## 6. Misure

Metodo M4.5/M6.1: 10 avvii per fase, segnale `RUNE_READY_DIR`, 2 s di
assestamento, 5 s di campionamento CPU. Dati grezzi:
[M71-BASELINE.md](M71-BASELINE.md), [M71-FINAL.md](M71-FINAL.md),
[M71-LOCAL.md](M71-LOCAL.md), [M71-LOCAL-2.md](M71-LOCAL-2.md), con CSV.

La baseline `aca8763` è stata misurata nella stessa sessione della build M7.1,
perché la macchina era più carica rispetto alle misure M6.1 delle 03:16.

| Metrica | Baseline `aca8763` | M7.1 finale |
| --- | ---: | ---: |
| Startup ready | 2.036,00 ms (1.787–3.079) | 1.486,00 ms (1.476–2.378) |
| Riposo working set | 194,97 MB | 124,72 MB |
| Riposo private bytes | 170,56 MB | 169,92 MB |
| Riposo CPU un core | 0,62% | 0,31% |
| Riproduzione working set | 195,76 MB | 126,83 MB |
| Riproduzione private bytes | 172,73 MB | 171,71 MB |
| Riproduzione CPU un core | 9,67% | 9,51% |
| Riproduzione CPU normalizzata | 0,81% | 0,79% |

Due serie M7.1 precedenti, con la stessa build prima della rifinitura grafica,
hanno dato 1.844,50 ms / 185,78 MB / 170,52 MB e 1.844,00 ms / 193,97 MB /
170,30 MB. La serie finale ha un working set di 124,72 MB: il working set non è
confrontabile tra sessioni (Windows può trimmarlo), mentre i private bytes
restano stabili (169,92 MB contro 170,56 MB della baseline). La variabilità del
working set è della macchina, non della persistenza.

Confronto con i numeri M6.1 documentati (03:16): 1.099,50 ms, 185,43 MB,
170,40 MB, 0,31%. La differenza di avvio è quindi **ambiente**, non persistenza:
nella stessa sessione la baseline è più lenta della build M7.1. I private bytes
a riposo sono invariati entro 0,64 MB. La persistenza aggiunge una query sulle
playlist a ogni reload, ma non produce un aumento misurabile di RAM o avvio.

La CPU in riproduzione varia tra 9,51% e 11,40% di un core nelle serie M7.1
(baseline 9,67%); M7.1 non ha modificato il percorso audio, quindi la differenza
rientra nella variabilità già documentata in M5.1/M6.1. Il target di avvio
< 300 ms resta non raggiunto, come in M4.5/M6.1; M7.1 non ha toccato il loop di
frame Rune.

Le ultime modifiche al backend riguardano solo il messaggio per una playlist
senza brani disponibili e i test; non toccano avvio, RAM o percorso audio.

## 7. Verifica

- `cargo fmt --all -- --check`: pulito.
- `cargo clippy --workspace --all-targets -- -D warnings`: pulito.
- `cargo test --workspace`: **75 passati, 0 falliti, 2 ignorati** (i due test di
  rete reale podcast).
- `cargo build -p musify --release`: riuscita.
- Test playlist della libreria: CRUD, ordinamento, cascata, migrazione legacy,
  file mancanti, filtro radio/podcast, M3U8 UTF-8 con path relativi e assoluti,
  URI ignorati.
- Test di integrazione app:
  `play_playlist_builds_a_play_tracks_command_and_skips_missing_files`.
- Snapshot UI headless della vista Playlist riuscito.

## 8. Limiti e cose rimaste fuori

- L'import M3U8 crea sempre una nuova playlist; non unisce a una esistente.
- I metadati sono snapshot: se i tag del file cambiano, la playlist non si
  aggiorna da sola. La copertina è il path memorizzato al momento dell'aggiunta.
- Non c'è drag del mouse per il riordino, per scelta esplicita; non c'è ancora
  una selezione multipla per aggiungere interi album o intere ricerche in un
  colpo.
- Radio e podcast restano esclusi dalle playlist. Jamendo (M7.2) non è ancora
  implementato: prima di scrivere codice vanno verificati API e termini correnti.
- La checklist manuale (`docs/MANUAL-CHECKLIST.md`, sezione 9) è pronta ma non è
  stata eseguita click-through in questa sessione; lo snapshot headless non
  sostituisce la prova con dialoghi nativi e file reali.
- Il primo avvio dopo la migrazione scrive lo schema e può avere la varianza
  tipica di Windows/OneDrive; le misure a regime non mostrano regressioni.
