# M7.2 — Jamendo

Data: 2026-11-11
Base: commit `aca8763` (M7.1 non ancora committata)
Build: release, Rune invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`
Crate nuovo: `musify-jamendo`

## 1. Documentazione e termini verificati

Documentazione ufficiale letta:

- <https://developer.jamendo.com/v3.0/docs>
- <https://developer.jamendo.com/v3.0/tracks>
- <https://developer.jamendo.com/v3.0/authentication>
- <https://devportal.jamendo.com/api_terms_of_use>

Punti rilevanti:

- L'API è REST-like su `https://api.jamendo.com/v3.0/`; ogni chiamata di lettura
  richiede `client_id`. Il client id è personale e non va condiviso.
- `GET /tracks/` restituisce, tra gli altri, `id`, `name`, `duration`,
  `artist_name`, `album_name`, `album_image`, `audio`, `license_ccurl` e
  `shareurl`. `audio` è l'URL di streaming; `audiodownload` e
  `audiodownload_allowed` riguardano il download e **non** sono usati.
- I termini (Clause 3.3) consentono l'uso gratuito non commerciale; per usi
  commerciali serve un accordo con Jamendo. La pagina principale indica fino a
  35.000 richieste/mese per le app non commerciali.
- Clause 2: le credenziali sono personali e non vanno condivise.
- Clause 3.3: Jamendo può imporre limiti di frequenza; la cache deve essere
  limitata a quanto necessario e non deve creare un accesso offline al catalogo.
- Clause 4.1: vanno attribuiti gli artisti come autori, Jamendo come provider e
  va fornito un backlink diretto dalla rappresentazione del contenuto alla
  pagina del brano su Jamendo.
- La licenza di ogni brano è Creative Commons e va rispettata.

Regole prudenti applicate in Musify:

- massimo 1 richiesta/secondo, con backoff esponenziale su `429`, `5xx` ed
  errori di rete (fino a 3 tentativi);
- cache dei risultati con TTL di 24 h (configurabile solo per test/sviluppo);
- nessuna copia dei file audio, nessun download e nessun uso dei campi di
  download;
- copertine in cache limitata (64 file / 8 MB);
- attribuzione visibile: artista, titolo, etichetta CC con link alla licenza e
  link alla pagina Jamendo del brano; album SMTC con licenza e “Jamendo”;
- user-agent identificabile `Musify/0.1.0 (+https://github.com/Marcu08/musify)`.

## 2. Implementazione

### Fonte e registro

`musify-jamendo` implementa `SourceProvider` con schema `jamendo`. L'app
registra la fonte nel `SourceRegistry` solo se `MUSIFY_JAMENDO_CLIENT_ID` è
presente e non vuota. Se manca:

- la fonte non viene registrata;
- `jamendo.sqlite3` non viene aperto;
- la scheda Jamendo non compare in Fonti;
- la vista Fonti mostra `Jamendo non configurato` e la variabile da impostare.

La chiave non viene mai salvata in configurazione, SQLite, log, screenshot o
messaggi di errore. Gli errori di rete non includono l'URL e i messaggi
dell'API vengono redatti sostituendo l'eventuale client id con `[client id]`.

### Ricerca

La ricerca usa `GET /tracks/` con:

- `search` per il testo libero (brano/album/artista);
- `tags` per il filtro genere/tag;
- `audioformat=mp32`, `imagesize=300`, `include=licenses`, `limit=100`.

La UI espone “Brano o artista” (testo libero) e “Tag / genere”. Nella prova
reale `tags=jazz` ha restituito 100 risultati; `search=love` ha restituito 100
risultati; `search=jazz` ha restituito 0 risultati, mentre `tags=jazz` ne ha
restituiti 100. È un comportamento dell'API: per il genere il campo Tag è
quello affidabile, mentre il testo libero serve per titolo/artista/album.

### Cache, preferiti e copertine

- `jamendo.sqlite3` contiene `tracks`, `favorites` e `searches`.
- La ricerca è network-first; se la rete fallisce, usa la cache se non ha più di
  24 h e restituisce `offline = true`.
- I preferiti sono locali e persistono tra i riavvii.
- `resolve` è **solo DB**: la riproduzione non fa una richiesta di rete sul
  thread UI. Se il brano non è in cache, restituisce `NotFound`.
- Le copertine sono scaricate dal campo `album_image` solo su richiesta del
  registro fonti, ridimensionate a 256 px e salvate in `jamendo-covers`
  (64 file / 8 MB). Nessun audio viene scritto su disco.

### Streaming OnDemand

`Track::is_on_demand` ora include `jamendo`, quindi il motore audio usa
`HttpStream::OnDemand` con Range/seek esattamente come per i podcast. La prova
reale ha verificato `206 Partial Content`, `Accept-Ranges: bytes` e
`Content-Range` sul server audio; la UI riceve `SeekableChanged` dal trasporto.

### Attribuzione e SMTC

- `SourceItem` ha ora `duration` e `license_url` (campi opzionali; radio e
  podcast li lasciano vuoti).
- La riga Jamendo mostra artista, album, durata, copertina, etichetta CC
  cliccabile e link “Jamendo” alla pagina del brano.
- `Track.album` vale `album · licenza · Jamendo` (o `licenza · Jamendo` per i
  singoli), così l'overlay SMTC mostra l'attribuzione anche se non ha un campo
  dedicato. La barra del player mostra la licenza accanto alla fonte.
- `Track.cover_path` punta alla copertina in cache; SMTC la usa come artwork.

### Playlist M7.1

`Backend::add_source_to_playlist` risolve l'elemento Jamendo e lo passa alla
libreria. Lo snapshot usa `source_scheme = "jamendo"` e l'URL audio come
`item_id`; la riproduzione ricostruisce un `Track` con schema `jamendo`,
`is_on_demand = true` e i metadati salvati. La prova reale ha aggiunto un brano
a una playlist temporanea e verificato `item_scheme=jamendo`, `available=true`,
`playable_scheme=jamendo`, `on_demand=true`.

## 3. Test

Test automatici Jamendo (11, tutti con server finto `wiremock`, nessuna rete
reale):

- `search_parses_metadata_and_ignores_download_fields`
- `search_falls_back_to_a_fresh_cache_when_the_server_is_down`
- `rate_limit_retries_then_succeeds`
- `rate_limit_exhaustion_does_not_leak_the_client_id`
- `api_error_message_is_redacted`
- `favorites_persist_and_resolve_is_db_only`
- `artwork_is_downloaded_and_cached`
- `favorites_persist_and_search_cache_expires`
- `license_labels_cover_common_creative_commons_urls`
- `display_album_keeps_license_and_jamendo_attribution`
- `download_fields_are_not_part_of_the_model`

Il test core `jamendo_tracks_are_on_demand_streams` verifica che un `Track`
Jamendo usi la modalità OnDemand. I test HTTP Range esistenti coprono seek,
`206`, `416`, server senza Range e riconnessione dall’offset.

Suite completa: **87 passati, 0 falliti, 2 ignorati** (i due test di rete reale
podcast). `cargo fmt` e `cargo clippy --workspace --all-targets -D warnings`
sono puliti. La build release è riuscita.

## 4. Prova reale manuale

La prova è stata eseguita con un esempio temporaneo, poi rimosso, usando la
chiave già presente nell'ambiente. La chiave non è stata stampata, salvata o
inclusa negli output; nessun file audio è stato scritto. Risultati:

```text
search results=100 offline=false
  Sexy Smooth Jazz - Full Length | pinegroove · Sexy Smooth Jazz | CC BY-NC-ND | 166 s
  ...
resolve scheme=jamendo on_demand=true duration=166 album=... · CC BY-NC-ND · Jamendo
audio status=206 Partial Content accept_ranges=Some("bytes") content_range=Some("bytes 0-0/4002989")
playlist added=1 items=1 item_scheme=jamendo available=true playable_scheme=jamendo on_demand=true
```

Non sono stati verificati in modo udibile playback, seek e overlay SMTC: la
checklist manuale (`docs/MANUAL-CHECKLIST.md`, sezione 10) descrive questi passi
per una sessione reale. La UI con e senza chiave è stata verificata con snapshot
headless (`m72-sources-jamendo.png`, `m72-sources-nokey.png`,
`m72-sources-nokey-release.png`) e la vista Jamendo con `m72-jamendo.png`.

## 5. Misure

Metodo M4.5/M6.1, 10 avvii per fase, release, stessa macchina. Dati grezzi:
[M72-FINAL-NOKEY.md](M72-FINAL-NOKEY.md), [M72-FINAL-KEY.md](M72-FINAL-KEY.md),
con CSV. La chiave era presente solo nella seconda serie; nessuna richiesta di
rete è stata fatta durante gli avvii.

| Metrica | M7.1 finale | M7.2 senza chiave | M7.2 con chiave |
| --- | ---: | ---: | ---: |
| Startup ready | 1.486 ms | 1.905 ms | 1.620 ms |
| Riposo working set | 124,72 MB | 193,74 MB | 185,98 MB |
| Riposo private bytes | 169,92 MB | 170,19 MB | 171,57 MB |
| Riposo CPU un core | 0,31% | 0,31% | 0,31% |
| Riproduzione CPU un core | 9,51% | 10,76% | 9,98% |
| Riproduzione CPU normalizzata | 0,79% | 0,90% | 0,83% |

Senza chiave la fonte non viene registrata e i valori restano nella variabilità
di M7.1 (private bytes 170,19 MB). Con la chiave si aprono la connessione SQLite
Jamendo e il provider: +1,38 MB di private bytes a riposo rispetto alla serie
senza chiave, coerente con una connessione SQLite; nessun runtime di rete viene
creato all'avvio. Lo startup con chiave è più veloce della serie senza chiave per
la varianza già documentata; non c'è una regressione misurabile. Il working set
varia e non è confrontabile tra sessioni.

## 6. Limiti e cose rimaste fuori

- La prova manuale completa (ascolto, seek udibile, overlay SMTC, aggiunta da
  UI) richiede una sessione audio reale; la checklist è pronta ma non è stata
  eseguita click-through.
- La ricerca libera dell'API non è affidabile per i generi: `search=jazz` ha
  restituito 0 risultati, `tags=jazz` 100. La UI ha un campo Tag dedicato.
- `resolve` è solo DB. Se un metadato viene eliminato dalla cache (30 giorni,
  non preferito), un vecchio `SourceItem` non è più riproducibile senza una
  nuova ricerca. Le playlist salvano l'URL audio nello snapshot e non dipendono
  da `resolve`.
- Gli URL audio Jamendo possono cambiare o scadere; una playlist con un URL
  vecchio può fallire. Non c'è ancora un refresh automatico degli snapshot.
- La cache TTL è 24 h per default; l'override
  `MUSIFY_JAMENDO_CACHE_TTL_HOURS` è solo per test/sviluppo.
- Jamendo è usato solo per uso personale/non commerciale. Non ci sono download,
  registrazioni, ripping, scraping o accesso offline al catalogo.
- La chiave va impostata prima dell'avvio; aggiungerla a runtime richiede un
  riavvio.
- HLS non è supportato, come per radio e podcast. Jamendo viene richiesto in
  MP3 (`mp32`) e riprodotto in streaming progressivo.
- Il client id non compare nel repository, nei log, negli screenshot o nei
  test; i test usano un valore sintetico generato a runtime. La prova reale ha
  usato la chiave dell'ambiente senza stamparla.
