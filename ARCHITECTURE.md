# Architettura

Musify è un workspace Cargo. Il core non conosce Rune, SQLite, cpal né le API delle fonti:
la UI comunica solo con comandi ed eventi.

## Crate

| Crate | Responsabilità | Dipende da UI? |
| --- | --- | --- |
| `musify-core` | tipi di dominio, `Command`, `Event`, trait `SourceProvider` e `LibraryProvider` | no |
| `musify-audio` | decodifica (symphonia), output (cpal/WASAPI), coda, seek, volume | no |
| `musify-library` | scanner (walkdir + lofty), indice SQLite, ricerca, cache copertine, playlist | no |
| `musify-radio` | `SourceProvider` Radio Browser, cache e preferiti SQLite | no |
| `musify-podcast` | `SourceProvider` iTunes Search, RSS, iscrizioni/episodi SQLite | no |
| `musify-jamendo` | `SourceProvider` Jamendo, cache/preferiti SQLite, copertine | no |
| `musify-ui` | viste e input Rune; traduce gesti in `Command` e applica `Event` | sì |
| `musify-app` | config, logging, registro fonti, SMTC, persistenza, coda | no |

## Flusso comandi/eventi

```text
┌──────────────┐   Command    ┌──────────────────┐   chiamate   ┌──────────────┐
│  UI (Rune)   │ ───────────▶ │  App controller  │ ───────────▶ │ audio engine │
│              │              │                  │              │ library      │
│              │ ◀─────────── │                  │ ◀─────────── │ sources      │
└──────────────┘    Event     └──────────────────┘   Event      └──────────────┘
```

- La UI non chiama mai direttamente il motore audio o il database.
- Un canale `crossbeam-channel` porta i `Command` dalla UI al controller.
- Un secondo canale porta gli `Event` dal controller alla UI.
- Gli eventi di posizione sono emessi a bassa frequenza (circa 4–10 Hz) e solo mentre la
  riproduzione è attiva; a riposo non viene svegliata la UI.

## Thread

| Thread | Lavoro |
| --- | --- |
| UI | Rune/winit; costruisce solo le righe visibili |
| app controller | instrada comandi, possiede coda e stato di riproduzione |
| audio engine | comandi, coda, decodifica e prefill del ring buffer |
| audio callback | consuma il ring buffer; nessuna allocazione, nessun lock bloccante |
| library scanner | walkdir + metadati + scritture SQLite |
| watcher | notify, con debounce, richiede una scansione incrementale |

## Pipeline audio (M1)

```text
file → symphonia (decode, thread engine)
     → f32 stereo + resampling sinc (rubato) alla frequenza del device
     → ring buffer rtrb (2 s)
     → callback cpal: gain, pop, scrittura del buffer del device
```

Il resampler usa rubato `Async` con sinc finestrato Blackman2, 64 tap e 128×
oversampling; il ritardo iniziale viene tagliato e il chunk residuo viene
flushato a fine brano. A parità di frequenza il percorso è una copia esatta. Se
`MUSIFY_LINEAR_RESAMPLER=1`, se rubato non si inizializza o se un chunk fallisce,
si usa il vecchio interpolatore lineare invece di interrompere la riproduzione.

Invarianti:

- Il callback non alloca, non fa I/O, non prende lock e non usa `unwrap`.
- La posizione mostrata deriva dai frame **consumati** dal callback; il buffer
  non viene conteggiato come già riprodotto.
- Pausa: il callback scrive silenzio e non consuma, quindi il buffer resta pieno
  e la ripresa è immediata.
- Seek: un epoch atomico fa svuotare il ring buffer al callback; l'engine attende
  l'ack e riparte dalla nuova posizione. Questo evita che campioni vecchi
  seguano il seek.
- Brano successivo: viene aperto in anticipo; i suoi campioni entrano nello
  stesso stream e il confine di frame decide quando cambiare i metadati. È
  gapless a livello di sample, senza crossfade. Un `Next` manuale svuota il
  buffer e può avere un piccolo gap.
- Volume: `AtomicU16` letto dal callback, con smoothing del gain; nessun lock.
- Modifica della coda mentre il brano successivo è già decodificato: remove,
  move e shuffle salvano la posizione corrente, svuotano ring buffer e
  decodifica, riportano il cursore della coda sul brano udibile (il prefetch
  può averlo spostato avanti) e riaprono il brano. Così non si sente audio del
  brano rimosso o spostato. È coperto da test con un callback finto.

## UI Rune (M3)

```text
UI (Rune, thread principale)
  ├─ Command  ───────────────▶ AppBackend ──▶ AudioEngine / Library / watcher
  └─ Event    ◀────────────── eventi audio, scansione, errori
       ▲
       └─ Waker Rune: la UI viene svegliata solo quando arriva un evento
```

- `musify-ui` dipende solo da `musify-core` e da Rune; il backend è il trait
  `ui::Backend`. L'app implementa il trait in `crates/app/src/backend/mod.rs`,
  con la logica divisa in `persistence.rs`, `queue.rs`, `smtc.rs` e
  `sources.rs`.
- Il thread UI non fa I/O audio o SQLite pesante: le liste vengono lette dal
  backend al reload; le copertine sono caricate on demand da `ImageCache`.
- `virtual_rows` + `virtual_rows_version` costruiscono solo le righe visibili;
  chiavi e versioni cambiano quando cambia la vista.
- Un bridge thread inoltra gli eventi dell'audio engine nel canale UI e chiama
  il `Waker`; la scansione usa lo stesso canale, con progress throttled a 4 Hz.
- Il watcher avvia una scansione incrementale dopo 750 ms di quiete.
- Su Windows il primo `on_ready` ridimensiona la finestra per correggere il
  fattore DPI iniziale di Rune; è un workaround locale documentato.
- Stringhe in `i18n.rs`, ruoli colore in `theme.rs`; nessun testo o colore
  hardcoded nei componenti.

## Integrazione Windows (M4)

- **SMTC**: `souvlaki` riceve l'HWND da Rune in `on_ready`
  (`with_raw_handle`), invia a Windows titolo/artista/album/durata/copertina e
  riceve Play/Pause/Stop/Next/Previous/Seek/SetPosition. Gli eventi diventano
  `Command` per lo stesso audio engine della UI.
- **Tasti multimediali**: Windows li instrada alla sessione SMTC; non c'è un
  hook di tastiera separato.
- **Persistenza**:
  - `config.toml`: cartelle, volume, shuffle, repeat; scrittura atomica.
  - `state.json`: coda come path, indice corrente, posizione; scrittura atomica
    con throttle 2 s e salvataggio alla chiusura.
  - Al riavvio i path vengono risolti con la libreria; la coda viene ripristinata
    con `Command::RestoreQueue`, che apre e prefill-a il brano ma resta in pausa.
  - `podcasts.sqlite3`: iscrizioni, episodi, stato ascoltato e posizione di
    ripresa per episodio. Non viene salvata la coda podcast in `state.json`.
- **DPI**: Rune crea la prima finestra con lo scale iniziale; `on_ready` la
  ridimensiona/riposiziona. È un workaround circoscritto alla UI.

## Prestazioni e qualità locale (M4.5)

- Il profilo release usa `codegen-units=1`, `lto="fat"` e `strip=true`.
- `scripts/bench-m45.ps1` misura 10 avvii a riposo e 10 in riproduzione:
  tempo fino al segnale `RUNE_READY_DIR`, working set, private bytes e CPU su
  una finestra di 5 s. I risultati sono in `docs/M45-BASELINE.md` e
  `docs/M45-FINAL.md`.
- Il caricamento libreria resta sincrono all'avvio: la variante lazy è stata
  misurata e ha peggiorato i private bytes di circa 22 MB, quindi è stata
  rimossa. La cache copertine tiene al massimo 128 immagini a 192 pt per
  limitare il caso di librerie grandi.
- Il resampler sinc è descritto nella pipeline audio; il fallback lineare è
  disponibile per diagnosi e macchine lente.
- La coda invalida l'audio già decodificato a ogni modifica rilevante; i test
  usano un callback finto che riconosce gli epoch di flush.

## Fonti online e radio (M5)

```text
UI Fonti/Radio
  ├─ Backend.source_search("radio") ──▶ SourceRegistry ──▶ musify-radio
  └─ Backend.source_play("radio") ────▶ SourceProvider::resolve
                                             │
                                             ▼
                         Command::PlayTrack (Track.source = radio://url)
                                      │
                              audio engine
                                      │
                        HttpStream (reqwest/rustls, thread + Tokio)
                                      │  buffer adattivo + parser ICY
                                      ▼
                        symphonia → resampler → ring buffer
```

- `SourceId` è l'unico dato di provenienza nel core; `Track.source` vale
  `local://path` per la libreria e `radio://url` per gli stream. Nessun tipo di
  una fonte specifica attraversa il core.
- `musify-radio` non conosce l'audio: implementa `SourceProvider` (schema
  `radio`, ricerca, preferiti, risoluzione `Live`, artwork), converte le stazioni
  in `SourceItem`/`Track` e salva cache e preferiti in `radio.sqlite3`. Ogni
  richiesta crea e distrugge il proprio runtime Tokio.
- Lo stream HTTP vive in `musify-audio`: thread dedicato, runtime Tokio
  `current_thread` solo per la durata dello stream, buffer 128 KiB→1 MiB,
  timeout, backoff e riconnessione. Il parser ICY toglie i metadata dal flusso e
  pubblica `StreamTitle`.
- La UI mostra il titolo ICY e lo stato dello stream; lo stesso titolo va a
  SMTC. La coda radio non viene salvata in `state.json` (solo i brani locali).
- HLS non è implementato. Il seek è ignorato per gli stream.

## Ridisegni e stream (M5.1)

- Gli eventi di posizione arrivano ogni 200 ms. Il contatore dei frame ha
  mostrato che durante la riproduzione Rune presenta a ~60 fps mentre i commit
  logici sono pochi; a riposo i frame sono zero. Coalescing, `Tx::instant`,
  `Tx::settle` e throttle dei wake sono stati provati e rimossi: nessun guadagno
  CPU riproducibile. Il comportamento UI resta quello M5.
- Le ricerche radio hanno un `request_id`; una ricerca nuova invalida quella
  vecchia. La cache delle ricerche ha TTL (7 giorni di default) e le favicon
  stanno in una cartella limitata a 256 file/16 MB.
- La riconnessione HTTP incrementa una `generation`; il decoder Symphonia si
  riapre da zero quando cambia, così Ogg non viene interpretato come
  continuazione del vecchio stream.
- HLS è rifiutato esplicitamente: richiederebbe playlist, segmenti TS/fMP4,
  chiavi e gestione live, fuori dal perimetro dello streaming progressivo.

## Astrazione fonti e backend modulare (M5.2)

- `musify-core::SourceProvider` è l'unica interfaccia di una fonte online:
  `scheme()`, `search(&SourceQuery)`, `favorites()`, `toggle_favorite(id)`,
  `resolve(id)` e `artwork(id)` opzionale. `SourceQuery` ha testo libero e
  filtri chiave/valore interpretati solo dalla fonte.
- `resolve` restituisce un `ResolvedSource` con il `Track` e `SourcePlayback`:
  `Live` per gli stream infiniti, `OnDemand { duration, seekable }` per le
  risorse finite. La radio restituisce sempre `Live`.
- L'app costruisce un `SourceRegistry` per schema. Il registro incrementa un
  `request_id` per fonte, scarta i risultati superati, emette
  `SourceResults`/`SourceSearchFailed`/`SourceArtwork` con il campo `source` e
  precarica al massimo otto artwork per risposta. La logica di rete e cache
  resta dentro il provider.
- Il trait `ui::Backend` espone `source_search`, `source_favorites`,
  `source_toggle_favorite`, `source_play`, `source_entries`, `source_refresh` e
  `source_set_watched`, tutti con lo schema come primo argomento. Le viste radio
  e podcast chiamano gli stessi metodi con `"radio"` o `"podcast"`.
- `backend.rs` è stato diviso in `backend/mod.rs` (cablaggio e tratti),
  `persistence.rs` (config TOML), `queue.rs` (`state.json`), `smtc.rs`
  (metadati Windows) e `sources.rs` (registro fonti).
- Lo scheletro della seconda fonte e la relativa feature sono stati rimossi:
  nessuna fonte è dichiarata se non è implementata.

## HTTP Range e podcast (M6.1)

```text
UI Podcast
  ├─ source_search("podcast") ──▶ iTunes Search ──▶ SourceItem
  ├─ source_entries/refresh ────▶ feed-rs (RSS) ──▶ SQLite (iscrizioni, episodi)
  └─ source_play("podcast") ────▶ SourceProvider::resolve
                                      │
                                      ▼
                     Command::PlayTrackAt (Track.source = podcast://audio-url)
                                      │
                         HttpStream StreamMode::OnDemand
                         ├─ Content-Length + Accept-Ranges
                         ├─ Seek → Range: bytes=<offset>-
                         ├─ 206/416 e redirect gestiti
                         └─ riconnessione dall’offset consegnato
```

- `HttpStream` ha due modalità: `Live` (radio, ICY, riconnessione dall’inizio)
  e `OnDemand` (episodi finiti). In `OnDemand` legge `Content-Length` e
  `Accept-Ranges`; `HttpReader::seek` invia una richiesta `Range` e aspetta la
  nuova connessione. Se il server ignora o rifiuta Range, il lettore resta
  riproducibile ma `is_seekable()` è falso.
- La riconnessione di un `OnDemand` seekable riparte dai byte già consegnati
  al decoder; un server non seekable interrompe invece lo stream invece di
  riprodurre silenziosamente dall’inizio.
- `musify-podcast` usa iTunes Search per la scoperta e `feed-rs` per RSS/Atom.
  I feed malformati, gli enclosure mancanti e i redirect producono errori o
  episodi saltati senza panic. Le richieste di rete avvengono solo su azione
  esplicita: nessun polling.
- SQLite `podcasts.sqlite3`: tabella `podcasts` (feed, metadati, iscrizione) e
  `episodes` (guid, data, durata, enclosure, watched, position_ms). Gli
  aggiornamenti del feed preservano watched e posizione.
- La copertina è in `podcast-covers/<blake3>.jpg`, limitata a 64 file/8 MB;
  viene passata a SMTC tramite `Track.cover_path`. Titolo episodio, autore,
  album e durata sono già nel `Track`.
- `Command::PlayTrackAt` apre l’episodio e applica la posizione di ripresa.
  `AppBackend` salva la posizione ogni 2 s e su pausa/stop/chiusura; quando si
  arriva a meno di 5 s dalla fine marca l’episodio come ascoltato.
- La UI distingue radio e podcast: la radio mostra `LIVE`, il podcast mostra la
  durata e abilita il seek solo dopo `SeekableChanged(true)`.

## Playlist (M7.1)

```text
UI Playlist
  ├─ Backend.playlists/playlist_items ──▶ Library (SQLite)
  ├─ create/rename/delete/add/remove/move
  ├─ play_playlist ──▶ Command::PlayTracks (file mancanti saltati)
  ├─ enqueue_playlist ──▶ Command::Enqueue
  └─ import/export M3U8 ──▶ Library (UTF-8, #EXTINF, path relativi/assoluti)
```

- Lo schema è versionato con `PRAGMA user_version = 2`. La migrazione crea
  `playlists` e `playlist_items`, copia le vecchie righe di `playlist_tracks`
  negli snapshot e poi elimina la tabella legacy. I dati esistenti non vengono
  persi; la migrazione è idempotente e coperta da test.
- `playlist_items` è generica per fonte: `source_scheme`, `item_id`, `position`
  e snapshot di `title`, `artist`, `album`, `duration_ms`, `cover_path`. Per i
  brani locali `item_id` è il path; per le fonti future è l'URI della fonte.
  `available` non è persistito: viene calcolato alla lettura (`Path::is_file()`
  per `local`, sempre vero per le altre fonti).
- Radio e podcast sono esclusi dalle playlist: il filtro è applicato sia nella
  UI sia in `Library::add_tracks_to_playlist`, così una chiamata futura non può
  inserirli per errore.
- `playlist_playback_command` costruisce `Command::PlayTracks` con i soli
  elementi disponibili. Se la playlist è vuota o tutti i file mancano, l'app
  emette un `Event::Error` leggibile invece di avviare una coda vuota.
  `enqueue_playlist` usa la stessa selezione.
- La UI ha una vista dedicata con pannello playlist e lista virtualizzata. Le
  azioni sono pulsanti (crea, rinomina, elimina, riproduci, accoda, salva coda,
  importa/esporta, aggiungi brano corrente/coda, rimuovi mancanti); il riordino
  usa ↑/↓ e `Alt+↑`/`Alt+↓` sull'elemento selezionato, senza drag.
- M3U8: export UTF-8 con `#EXTM3U`/`#EXTINF`, path relativo quando il file è
  sotto la cartella dell'export, altrimenti path assoluto; import che risolve i
  path rispetto al file e ignora le righe con schema URI. Il round-trip è
  coperto da test con metadati non ASCII.
- Il trait `ui::Backend` espone i metodi playlist con default, così i backend di
  test restano compilabili; `AppBackend` li inoltra alla libreria.

## Jamendo (M7.2)

```text
UI Jamendo
  ├─ source_search("jamendo") ──▶ JamendoSource::search
  │     ├─ GET api.jamendo.com/v3.0/tracks (client_id, search/tags, audioformat=mp32)
  │     ├─ rate limit 1/s + backoff su 429/5xx/errori di rete
  │     ├─ cache SQLite 24 h come fallback
  │     └─ SourceItem (audio, license_ccurl, album_image, duration, shareurl)
  └─ source_play("jamendo") ──▶ resolve (solo DB)
        └─ Command::PlayTrack (Track.source = jamendo://audio-url, OnDemand)
              └─ HttpStream::OnDemand (Range/seek)
```

- `musify-jamendo` implementa `SourceProvider` con schema `jamendo`. Il client id
  arriva solo da `MUSIFY_JAMENDO_CLIENT_ID`: se manca, l'app non registra la
  fonte, non apre `jamendo.sqlite3` e la vista Fonti mostra come configurarla.
- Il provider non legge `audiodownload` né `audiodownload_allowed`: usa solo il
  campo `audio` in streaming. Nessun byte audio viene scritto su disco; le
  copertine stanno in `jamendo-covers` (64 file / 8 MB).
- La ricerca è network-first con fallback a una cache SQLite di 24 h; preferiti
  e metadati dei brani stanno in `jamendo.sqlite3`. `resolve` è solo DB, quindi
  la riproduzione non blocca la UI su una richiesta di rete; la rete vive nel
  thread di ricerca del registro fonti.
- Il client usa uno user-agent identificabile, limita le richieste a 1/s,
  ritenta 429/5xx/errori di rete con backoff esponenziale e redige il client id
  dai messaggi di errore.
- `Track::is_on_demand` include `jamendo`, quindi `HttpStream::OnDemand` gestisce
  gli stream finiti e il seek Range. `Track.album` contiene
  `album · licenza · Jamendo` per l'attribuzione in SMTC; la UI mostra licenza e
  link alla licenza CC e alla pagina del brano Jamendo.
- I brani Jamendo entrano nelle playlist M7.1 tramite
  `Backend::add_source_to_playlist`: il backend risolve l'elemento e salva il
  consueto snapshot, con `source_scheme = "jamendo"` e l'URL audio come
  `item_id`.

## Libreria locale (M2)

```text
cartelle musicali
   → walkdir (raccolta file, dedup per path normalizzato)
   → stat per file: mtime + size vs SQLite (invariati: skip)
   → rayon: lofty (tag) + cover (decodifica, resize 512 px, JPEG)
   → transazione SQLite a batch di 500
   → FTS5 (titolo/artista/album) + tabelle albums/artists
   → rimozione dei file spariti sotto le cartelle scansionate
   → pulizia album/artisti orfani
```

- WAL e `busy_timeout=5000`: le letture della UI non bloccano la scansione.
- La connessione della UI e quella dello scanner sono separate.
- Copertine: `covers/<blake3>.jpg`, contenuto indirizzato, riuso automatico.
- La ricerca usa FTS5 con `remove_diacritics 2`; se FTS5 non è disponibile
  (`init_schema` fallisce) usa `LIKE` con escape di `%`, `_` e `\`.
- `notify` osserva le cartelle e, dopo 750 ms di quiete, emette un segnale;
  l'app lancia la scansione incrementale. Il watcher non scrive nel DB.
- Schema: `tracks`, `albums`, `artists`, `playlists`, `playlist_items`.
  `playlist_items` è la forma versionata (M7.1); `playlist_tracks` è la tabella
  legacy, copiata e rimossa dalla migrazione.

## Persistenza

- `config.toml`: cartelle musicali, volume, shuffle/repeat, preferenze UI.
- `state.json`: coda, brano corrente, posizione, timestamp di salvataggio.
- `library.sqlite3`: brani, album, artisti, playlist, elementi delle playlist,
  stato della scansione.
- `jamendo.sqlite3`: cache ricerche, metadati dei brani e preferiti Jamendo.
- `jamendo-covers/`: copertine Jamendo, massimo 64 file / 8 MB.
- `covers/`: copertine ridimensionate, nominate per hash del contenuto.

## Decisioni

- **M0**: Rune è pinnato a un rev `dev` documentato in `Cargo.toml`; nessun path locale.
- **Core indipendente**: nessun tipo Rune, cpal o rusqlite attraversa il confine pubblico.
- **Errori**: `thiserror` nei crate di dominio, `anyhow` solo nel binario.
- **Audio**: niente `unwrap` nel percorso di riproduzione; il callback non alloca e non
  acquisisce lock bloccanti.
- **Gapless**: i sample del brano successivo entrano nello stesso ring buffer; il cambio
  metadati avviene al confine di frame. Nessun crossfade. Un salto manuale può avere un
  piccolo gap perché svuota il buffer.
- **Resampling**: sinc con `rubato` in M4.5, confinato a `resample.rs`; il
  contratto del decoder non cambia e il lineare resta come fallback esplicito.
- **Modifiche di coda**: il cursore della coda non è la verità su ciò che si
  sente, perché il prefetch lo sposta avanti. L'engine usa il proprio indice
  udibile, riallinea il cursore e riapre il brano dopo il flush.
- **M4.5**: nessun target di RAM/avvio forzato; si tengono solo le modifiche con
  un guadagno misurato e si riportano anche quelle fallite (lazy load).
- **M5**: il core trasporta `SourceId`, non flussi o client HTTP. La radio
  preferisce stream progressivi MP3/AAC/Ogg e non HLS; Tokio non esiste finché
  non si apre una fonte online. I flussi non vengono mai scritti su disco.
- **M5.1**: la CPU dei ridisegni è stata strumentata ma non ridotta in modo
  riproducibile; le modifiche non provate sono state rimosse. Le cache radio
  hanno TTL e limiti su disco, le ricerche sono cancellabili e HLS è rifiutato.
- **M7.1**: le playlist sono snapshot generici per fonte, non foreign key verso
  `tracks`: un file spostato o una fonte futura non devono rompere la playlist.
  La disponibilità è calcolata alla lettura e la riproduzione salta gli elementi
  mancanti. Il riordino è esplicito (pulsanti/tastiera), non basato sul drag.
- **M7.2**: Jamendo è una fonte opzionale registrata solo se il client id è
  presente nell'ambiente. La rete avviene solo nella ricerca/artwork, mai in
  `resolve` o a riposo; il client id non viene mai serializzato, loggato o
  mostrato. Solo streaming: i campi di download dell'API non sono modellati.

## Diagrammi di dettaglio

Saranno aggiunti in M1 (pipeline audio) e M2 (schema SQLite e flusso di scansione).
