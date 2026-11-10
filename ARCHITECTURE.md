# Architettura

Musify è un workspace Cargo. Il core non conosce Rune, SQLite, cpal né le API delle fonti:
la UI comunica solo con comandi ed eventi.

## Crate

| Crate | Responsabilità | Dipende da UI? |
| --- | --- | --- |
| `musify-core` | tipi di dominio, `Command`, `Event`, trait `SourceProvider` e `LibraryProvider` | no |
| `musify-audio` | decodifica (symphonia), output (cpal/WASAPI), coda, seek, volume | no |
| `musify-library` | scanner (walkdir + lofty), indice SQLite, ricerca, cache copertine | no |
| `musify-radio` | `SourceProvider` Radio Browser, cache e preferiti SQLite | no |
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
  `source_toggle_favorite` e `source_play`, tutti con lo schema come primo
  argomento. La UI radio chiama gli stessi metodi con `"radio"`.
- `backend.rs` è stato diviso in `backend/mod.rs` (cablaggio e tratti),
  `persistence.rs` (config TOML), `queue.rs` (`state.json`), `smtc.rs`
  (metadati Windows) e `sources.rs` (registro fonti).
- Lo scheletro della seconda fonte e la relativa feature sono stati rimossi:
  nessuna fonte è dichiarata se non è implementata.

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
- Schema: `tracks`, `albums`, `artists`, `playlists`, `playlist_tracks`.
  Le playlist sono predisposte nello schema, la UI arriverà dopo l'MVP.

## Persistenza

- `config.toml`: cartelle musicali, volume, shuffle/repeat, preferenze UI.
- `state.json`: coda, brano corrente, posizione, timestamp di salvataggio.
- `library.sqlite3`: brani, album, artisti, playlist, stato della scansione.
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

## Diagrammi di dettaglio

Saranno aggiunti in M1 (pipeline audio) e M2 (schema SQLite e flusso di scansione).
