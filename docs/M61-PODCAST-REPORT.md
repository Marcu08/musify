# M6.1 — Podcast e HTTP Range

Data: 2026-11-11
Base: commit `b124a11` (M5.2)
Build: release, Rune invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`
Crate nuovo: `musify-podcast`

## 1. HTTP Range

`crates/audio/src/http_stream.rs` ora ha due modalità:

- `StreamMode::Live`: comportamento radio M5/M5.2, ICY, riconnessione
  dall'inizio.
- `StreamMode::OnDemand`: risorsa finita, usata dagli episodi podcast.

In `OnDemand` il trasporto:

- legge `Content-Length` e `Accept-Ranges: bytes` dalla risposta finale
  (dopo i redirect gestiti da reqwest);
- abilita `HttpReader::seek` solo se il server dichiara Range;
- traduce `SeekFrom::Start/Current/End` in una richiesta
  `Range: bytes=<offset>-`, attende la nuova connessione e svuota il buffer;
- gestisce `206 Partial Content`, `Content-Range`, `416 Range Not
  Satisfiable` e server che ignorano Range con `200`;
- riconnette una risorsa seekable dai byte già consegnati al decoder, non
  dall'inizio;
- per una risorsa non seekable interrompe lo stream invece di riprodurre
  silenziosamente dall'inizio;
- mantiene il thread vivo dopo EOF per permettere un seek successivo.

`TrackDecoder::is_seekable()` espone la capacità reale al motore. Il motore
emette `Event::SeekableChanged` e rifiuta il seek su stream live/non Range. La
UI mostra la durata per i podcast e `Stream non seekable` quando il server non
supporta Range.

Test automatici:

- `range_requests_resume_from_the_byte_offset` (200 iniziale, seek 206);
- `a_server_without_range_support_stays_playable_but_not_seekable`;
- `a_range_request_rejected_with_416_fails_the_seek`;
- `a_dropped_range_stream_reconnects_from_the_delivered_offset`;
- `seeks_an_http_on_demand_stream_with_range_requests` (MP3 reale servito dal
  server finto, seek Symphonia attraverso il trasporto Range);
- il test live ICY esistente continua a passare.

Lo stream reale con redirect multipli è stato verificato manualmente: il
trasporto ha seguito i redirect, rilevato `Accept-Ranges`, aperto e cercato
l'MP3.

## 2. Fonte podcast

`musify-podcast` implementa `SourceProvider` con schema `podcast`:

- **Ricerca**: iTunes Search API (`media=podcast`, `entity=podcast`), con
  `MUSIFY_ITUNES_URL` sovrascrivibile per i test.
- **Feed**: `feed-rs 3.0` per RSS/Atom/JSON Feed. I redirect vengono seguiti e
  l'URL finale è usato come base per gli enclosure relativi.
- **Enclosure**: gestiti `content.src`, `media[].content[].url` e
  `link rel="enclosure"`. Gli episodi senza enclosure vengono saltati.
- **Durata**: `media.duration` di feed-rs più un fallback sul tag
  `itunes:duration` nel blocco `<item>` (feed-rs non emette un media object
  quando contiene solo la durata).
- **Feed malformati**: errore `invalid podcast feed`, nessun panic.
- **Arte**: copertina scaricata in `podcast-covers/<blake3>.jpg`, massimo
  64 file / 8 MB.

SQLite `podcasts.sqlite3`:

```sql
podcasts(id, feed_url, title, author, description, website, artwork_url,
         genre, subscribed, last_refresh)
episodes(id, podcast_id, title, description, published_at, duration_ms,
         audio_url, audio_type, watched, position_ms)
```

Gli aggiornamenti del feed preservano `watched` e `position_ms`. Non c'è
polling: ricerca, refresh, iscrizione e copertine partono solo da un'azione
dell'utente. Nessun runtime di rete resta attivo a riposo.

## 3. Riproduzione, seek e ripresa

- `SourceProvider::resolve` produce un `Track` con `SourceId::podcast(url)`,
  titolo episodio, autore/podcast, durata e copertina.
- `Command::PlayTrackAt` apre l'episodio e applica la posizione salvata.
- `AppBackend` salva la posizione ogni 2 s e su pausa/stop/chiusura. A meno di
  5 s dalla fine marca l'episodio come ascoltato.
- La posizione è per episodio, non per coda: non viene salvata una coda podcast
  in `state.json`.
- SMTC riceve titolo episodio, autore/podcast, album, durata e copertina dallo
  stesso `Track`.

## 4. UI

- Vista `Podcast` in Fonti: ricerca, iscrizioni, elenco podcast con copertina,
  pulsante Iscriviti/Annulla e Apri.
- Pannello episodi: titolo, data, durata, descrizione breve, stato ascoltato,
  posizione di ripresa, pulsanti Ascolta, Segna ascoltato/Non ascoltato e
  Aggiorna feed.
- La vista usa gli stessi metodi generici `source_*` del registro fonti; gli
  eventi `SourceEntries` arrivano in background solo dopo un refresh richiesto.
- Screenshot di controllo con `musify.exe --scene podcast`: layout e riga
  episodio renderizzati correttamente.

## 5. Test

`cargo test --workspace --all-features`: **67 passati, 0 falliti, 2 ignorati**
(i due test di rete reale).

| Crate | Test |
| --- | ---: |
| `musify-audio` | 29 + 1 ignorato |
| `musify-core` | 9 |
| `musify-library` | 14 |
| `musify-podcast` | 8 + 1 ignorato |
| `musify-radio` | 5 |
| `musify` (app) | 2 |

I due test ignorati sono stati eseguiti manualmente:

- `real_itunes_search_and_feed_smoke`: ricerca reale di “The Daily”, 71
  episodi, primo episodio con durata 1904 s.
- `plays_and_seeks_a_real_podcast_episode`: feed reale, redirect multipli,
  episodio di 2052,9 s, `seekable=true`, seek a 30 s e decodifica successiva.

Flusso UI completo eseguito sulla build release con eventi mouse/tastiera e
screenshot della finestra:

1. Fonti → Podcast → ricerca “The Daily” (risultati iTunes reali con copertine).
2. Apri “The Daily” → feed RSS reale, episodi con data, durata e descrizione.
3. Ascolta il primo episodio: titolo e autore nel player, durata 31:44.
4. Seek avanti con i tasti freccia: la posizione passa da 0:46 a 2:06 e la
   riproduzione continua.
5. Chiusura e riapertura: ricerca e riapertura dello stesso episodio, riparte
   da 4:45 invece che da 0:00; il DB contiene `position_ms=447555` dopo la
   chiusura finale.

Durante questo flusso è stato trovato e corretto un bug reale: feed-rs assegna
`content.src` all’URL del feed per `content:encoded`; il parser lo usava come
enclosure e tentava di riprodurre l’XML. Ora l’enclosure audio ha priorità e un
test di regressione copre il caso.

## 6. Misure

Stessa macchina e metodo M5.x, 10 avvii per fase. Dati grezzi:
[M61-FINAL-LOCAL.md](M61-FINAL-LOCAL.md),
[M61-FINAL-RADIO.md](M61-FINAL-RADIO.md),
[M61-FINAL-PODCAST.md](M61-FINAL-PODCAST.md), con CSV.

| Metrica | Locale | Radio | Podcast |
| --- | ---: | ---: | ---: |
| Working set | 187,60 MB | 195,04 MB | 194,14 MB |
| Private bytes | 170,87 MB | 173,92 MB | 173,68 MB |
| CPU un core | 6,55% (5,62–8,11) | 7,65% (5,62–9,05) | 8,58% (7,50–9,99) |
| CPU normalizzata | 0,55% | 0,64% | 0,71% |
| Startup ready | 1.099,50 ms (1.054–1.721) | — | — |
| Riposo working set | 185,43 MB | — | — |
| Riposo private bytes | 170,40 MB | — | — |
| Riposo CPU | 0,31% | — | — |

Confronto M5.2: locale 187,35 MB / 170,59 MB / 7,80%; radio 194,68 MB /
175,24 MB / 8,43%. Le differenze sono entro la variabilità già documentata.
Il podcast attivo costa come la radio: ~8,6% di un core e ~194 MB di working
set. Il target CPU < 2% non è raggiunto, come in M5.1; M6.1 non ha modificato il
loop di frame Rune.

## 7. Verifica

- `cargo fmt --all -- --check`: pulito.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  pulito.
- `cargo test --workspace --all-features`: 67 passati, 2 ignorati.
- `cargo build -p musify --release --bin musify`: riuscita.
- Test manuali di rete reale: ricerca/feed e riproduzione/seek, vedi sezione 5.
- Screenshot della vista Podcast con `--scene podcast`.

## 8. Problemi residui

- Il click-through UI è stato eseguito con eventi sintetici (mouse/tastiera) e
  verificato con `PrintWindow`; il trascinamento sintetico della seek bar non è
  stato affidabile, quindi il seek UI è stato verificato con i tasti freccia e
  il seek di trasporto con i test MP3 Range.
- L’iscrizione è stata verificata dal test reale e dal toggle UI, ma il flusso
  “iscrizione → riavvio → elenco iscrizioni” non è stato ripetuto nel
  click-through; gli episodi sono rimasti nel DB e si riaprono con la ricerca.
- Il fallback “server senza Range” è coperto dai test mock, non da un episodio
  pubblico non seekable.
- I feed vengono aggiornati solo su richiesta e non c'è paginazione: un feed
  molto grande viene letto interamente, con limite di 8 MB.
- Podcast molto grandi possono superare il buffer di 1 MiB; il decoder resta
  seekable ma l'avvio dipende dalla rete.
- HLS non è supportato, come per la radio.
- La CPU in riproduzione resta sopra il target M5.1; non è stato toccato il
  loop di frame Rune.
