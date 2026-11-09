# M5 — Fonti online e radio internet

Data: 2026-11-09
Crate nuovi: `musify-radio`
Dipendenze nuove: `reqwest 0.13` (rustls), `tokio 1.54`, `wiremock 0.6` (solo test)
Rune: invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`

## Cosa è stato fatto

### 1. Identificatore di fonte nel core

`musify_core::SourceId` descrive la provenienza di un brano:

- `local://<path>` per i file della libreria;
- `radio://<url>` per uno stream HTTP;
- lo schema è una stringa, quindi `subsonic://` o `spotify:` possono essere
  aggiunti senza toccare la coda.

`Track` ora ha `source: SourceId`. I JSON vecchi senza il campo restano validi e
vengono letti come locali. La coda, i comandi e gli eventi continuano a passare
solo per il core: nessun tipo Rune, reqwest o SQLite attraversa il confine.

Sono stati aggiunti anche:

- `StreamStatus` (`Connecting`, `Buffering`, `Playing`, `Reconnecting`,
  `Offline`, `Error`) e `StreamEvent` (stato e titolo ICY);
- `SourceItem`, il risultato generico di una fonte online mostrato dalla UI;
- gli eventi `StreamStatusChanged`, `StreamTitleChanged`, `SourceResults`,
  `SourceSearchFailed`.

### 2. Radio Browser (`musify-radio`)

Il client segue le indicazioni dell'API:

- server scoperti da `https://all.api.radio-browser.info/json/servers`, lista
  mescolata in base all'orologio e retry su ogni mirror;
- `User-Agent` identificabile: `Musify/0.1.0 (+https://github.com/Marcu08/musify)`;
- ricerca `/json/stations/search` per nome, paese e tag/genere, ordinata per voti
  e con `hidebroken=true`;
- cache SQLite in `radio.sqlite3` (tabella `stations`) e preferiti nella tabella
  `favorites`; se la rete fallisce e la cache contiene risultati, la ricerca
  restituisce i risultati salvati con il flag `offline`;
- runtime Tokio creato per singola richiesta e distrutto subito: a riposo non
  esiste alcun runtime di rete.

Il pannello Radio permette di cercare per nome, paese e genere/tag, salvare o
rimuovere preferiti e avviare una stazione.

### 3. Streaming HTTP

Lo stream vive nel crate audio, non in `musify-radio`:

- `reqwest` asincrono con rustls, `Accept: audio/*`, `Icy-MetaData: 1`;
- un thread dedicato crea un runtime Tokio `current_thread` solo mentre lo
  stream è aperto e lo distrugge alla chiusura;
- buffer adattivo da 128 KiB che cresce fino a 1 MiB quando il decoder soffre la
  fame; il decoder legge da un `MediaSource` non seekable;
- parser ICY incrementale: rimuove i blocchi metadata e pubblica `StreamTitle`;
- timeout di connessione 10 s, timeout di lettura 20 s;
- riconnessione con backoff 2, 4, 8, 16, 32 s (massimo 30 s); dopo cinque
  tentativi lo stato diventa `Offline`, ma i tentativi continuano;
- nessun salvataggio dei flussi su disco: `state.json` salva solo i brani locali.

Il decoder usa symphonia come per i file locali: MP3, AAC/ADTS e Ogg/Vorbis
funzionano finché il demuxer riconosce il flusso. Il seek è disabilitato per gli
stream.

### 4. UI e SMTC

- Nuova sezione **Fonti** con Locale e Radio.
- Vista Radio con campi Nome / Paese / Genere-tag, elenco risultati, preferiti e
  pulsante Ascolta.
- Badge della fonte nella barra "in riproduzione" (`Locale` / `Radio`).
- Per la radio il titolo mostra il `StreamTitle` ICY quando arriva, altrimenti il
  nome della stazione; lo stato mostra connessione, buffering, riproduzione,
  riconnessione o errore; la durata diventa `LIVE`.
- SMTC riceve il titolo ICY e lo stato di riproduzione; i tasti media continuano
  a pilotare lo stesso audio engine.

## Verifica

### Test automatici

`cargo test --workspace --all-features`: **51 test passati**, 0 falliti.

| Gruppo | Test | Cosa copre |
| --- | ---: | --- |
| core | 9 | SourceId locale/radio, JSON vecchio senza source, eventi |
| audio | 23 | codec, seek, resampler, coda, flush coda, HTTP stream, ICY, riconnessione |
| library | 14 | scansione, FTS, copertine, watcher |
| radio | 4 | mappa stazione→track, preferiti/cache, ricerca con server finto e fallback offline |
| spotify | 1 | scheletro compilabile |

Test HTTP/radio rilevanti:

- `http_stream::tests::icy_metadata_is_stripped_and_title_extracted`;
- `http_stream::tests::icy_parser_handles_metadata_split_across_chunks`;
- `http_stream::tests::a_dropped_connection_reconnects_and_keeps_metadata`
  (server TCP finto che chiude la prima connessione e accetta la seconda);
- `decoder::tests::decodes_an_http_stream` (MP3 servito via HTTP e decodificato);
- `client::tests::search_uses_the_fake_server_then_falls_back_to_the_cache`
  (wiremock + preferiti + cache offline).

`cargo clippy --workspace --all-targets --all-features -- -D warnings`: pulito.
`cargo fmt --all -- --check`: pulito.

### Prova reale

Stazione usata: MANGORADIO
(`https://mangoradio.stream.laut.fm/mangoradio`, MP3 128 kbps).

Dal log release:

```text
stato stream status=Buffering
titolo stream title=Some("Mark Ronson feat. Miley Cyrus - Nothing Breaks Like a Heart")
stato stream status=Playing
```

Nessun errore di rete; i due warning `invalid main_data_begin` di symphonia sono
normali all'aggancio di uno stream MP3 e non interrompono la riproduzione.

Soak di 5 minuti sulla stessa stazione: nessuna riconnessione, nessun errore,
tre titoli ICY distinti ricevuti (`Roses`, `All of Me`, `All Night`).
La finestra era in background, quindi Windows ha ridotto il working set a
54,8 MB e la CPU campionata (1,56% di un core) non è confrontabile con le misure
in primo piano della tabella. I private bytes sono rimasti 175,0 MB. Non è una
prova di più ore: vedere i limiti.

### Misura RAM/CPU con e senza radio

Stessa macchina, stessa build release (`ca9c660`), 10 avvii, finestra in primo
piano, 5 s di campionamento CPU dopo 2 s di assestamento. La prova locale usa il
brano FLAC M4.5; la prova radio usa MANGORADIO.

| Metrica | Locale (M5) | Radio MANGORADIO | Differenza |
| --- | ---: | ---: | ---: |
| Working set | 187,46 MB | 194,81 MB | +7,35 MB |
| Private bytes | 171,53 MB | 174,68 MB | +3,15 MB |
| CPU un core | 7,49% | 8,57% | +1,08 pp |
| CPU 12 thread | 0,62% | 0,71% | +0,09 pp |
| Underrun | 0 | 0 | — |

Dettaglio e valori grezzi: [M5-LOCAL-MEASURE.md](M5-LOCAL-MEASURE.md) e
[M5-RADIO-MEASURE.md](M5-RADIO-MEASURE.md) (radio: CPU 6,87–10,60% di un core;
locale 5,92–11,86%). La radio costa circa 7,4 MB di working set e 3,2 MB di
private bytes in più (TLS, buffer 128 KiB–1 MiB, decoding MP3) e circa 1,1 punti
percentuali di un core in più; entrambe le misure restano intorno al 10% di un
core. Il runtime Tokio non è attivo a riposo: l'avvio a riposo della build M5
resta 1,102 s, working set 185,28 MB, private 169,57 MB, CPU 0,31%.

### Evidenze UI

- `docs/evidence/m5-sources.png` — sezione Fonti.
- `docs/evidence/m5-radio.png` — ricerca radio, preferiti, badge e titolo ICY.

## Limiti e cose non fatte

- **Spotify** resta lo scheletro dietro feature: non è stato abilitato né
  implementato. La generalizzazione `SourceId` lo rende possibile, ma non è
  stato aggiunto alcun flusso Spotify.
- **HLS** non è supportato: il player gestisce stream HTTP progressivi
  MP3/AAC/OGG. Le stazioni HLS vanno escluse o falliscono con un errore chiaro.
- La riconnessione riprende i byte dal nuovo stream. Per MP3 il demuxer si
  riallinea; per Ogg/Vorbis un cambio di stream a metà può richiedere un restart
  del decoder, non ancora implementato. Il test di riconnessione copre il
  trasporto HTTP e i metadati, non ore di audio Ogg.
- La cache di ricerca non ha una scadenza: i risultati vecchi restano finché una
  ricerca online riuscita non li aggiorna.
- Una ricerca radio in corso non viene cancellata se l'utente ne lancia un'altra;
  il risultato più recente può essere sostituito da uno precedente.
- La copertina/favicon della stazione non viene scaricata: la UI usa il badge
  "FM" e la copertina locale resta vuota.
- La prova "per ore" non è stata eseguita: la sessione reale è stata breve. Il
  comportamento di lunga durata è coperto solo da buffer, timeout, backoff e
  test di riconnessione; va verificato con una prova manuale lunga.

## Comandi

```powershell
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build -p musify --release
pwsh -NoProfile -File .\scripts\bench-m45.ps1 -SkipBuild -Runs 10 `
  -RadioUrl "https://mangoradio.stream.laut.fm/mangoradio" `
  -Label "Radio internet attiva" -Output docs\M5-RADIO-MEASURE.md
```

Variabili utili:

- `MUSIFY_AUTOPLAY_RADIO=<url>`: avvia una stazione all'avvio (misura e debug);
- `MUSIFY_RADIO_BROWSER_URL=<base>`: usa un server Radio Browser alternativo o un
  fake server nei test.
