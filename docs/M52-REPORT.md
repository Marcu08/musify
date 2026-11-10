# M5.2 — Pulizia e astrazione delle fonti

Data: 2026-11-11
Base: commit `6c16263`
Build: release, Rune invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`
Crate nuovi: nessuno
Crate rimossi: `musify-spotify`

## 1. Cosa è stato rimosso

- Eliminato il crate `crates/spotify` (scheletro mai usato).
- Rimossi il member workspace, la dipendenza e la feature `spotify` da
  `crates/app/Cargo.toml`.
- Rimossi i riferimenti a Spotify da `README.md` e `ARCHITECTURE.md`.
- Eliminato il trait `core::PlaybackSource` e il relativo `PlaybackError`, che
  non avevano implementazioni né chiamanti. Nel core restano `SourceProvider`
  e `LibraryProvider`.

I report storici M3/M4/M4.5/M5/M5.1 conservano i riferimenti alle misure e allo
stato di allora; non sono stati riscritti.

## 2. Astrazione delle fonti

`musify-core::SourceProvider` è ora l'unica interfaccia di una fonte online:

```rust
fn scheme(&self) -> &'static str;
fn search(&self, query: &SourceQuery) -> Result<SourceSearch, SourceError>;
fn favorites(&self) -> Result<Vec<SourceItem>, SourceError>;
fn toggle_favorite(&self, id: &str) -> Result<bool, SourceError>;
fn resolve(&self, id: &str) -> Result<ResolvedSource, SourceError>;
fn artwork(&self, id: &str) -> Result<Option<PathBuf>, SourceError>; // opzionale
```

- `SourceQuery` contiene testo libero e filtri `(chiave, valore)`; il core non
  interpreta le chiavi. La radio usa `country` e `tag`.
- `ResolvedSource` restituisce il `Track` e `SourcePlayback`: `Live` per gli
  stream infiniti, `OnDemand { duration, seekable }` per le risorse finite.
- `RadioBrowser` implementa il trait: ricerca, preferiti, toggle, risoluzione
  `Live` e artwork/favicon. La logica di rete, cache SQLite e download icone
  resta in `musify-radio`.

L'app costruisce un `SourceRegistry` per schema. Il registro:

- incrementa un `request_id` per fonte e scarta i risultati superati;
- emette `SourceResults`, `SourceSearchFailed` e `SourceArtwork` con il campo
  `source`;
- precarica al massimo otto artwork per risposta, con deduplica dei download in
  corso;
- non conosce radio, podcast o altri tipi concreti.

`ui::Backend` espone `source_search`, `source_favorites`,
`source_toggle_favorite` e `source_play`, tutti con lo schema come primo
argomento. La vista Radio chiama gli stessi metodi con `"radio"`; testi,
layout, preferiti e comportamento restano quelli M5.1.

## 3. Backend modulare

`crates/app/src/backend.rs` è stato sostituito da:

| File | Responsabilità |
| --- | --- |
| `backend/mod.rs` | `AppBackend`, cablaggio, ciclo di vita, query libreria, scan |
| `backend/persistence.rs` | `Config`, lettura/scrittura atomica di `config.toml`, path dati |
| `backend/queue.rs` | `SavedState`, ripristino e salvataggio di `state.json` |
| `backend/smtc.rs` | snapshot SMTC, metadati, comandi Windows |
| `backend/sources.rs` | `SourceRegistry`, ricerca/preferiti/risoluzione, prefetch artwork |

Il comportamento è stato mantenuto:

- la coda locale viene ripristinata in pausa e le fonti live non vengono salvate;
- il throttle di salvataggio resta 2 s, con salvataggio forzato alla chiusura;
- SMTC riceve gli stessi eventi e mantiene il throttle di 1 s;
- la scansione, il watcher, la config e i comandi audio non cambiano;
- l'ordine di aggiornamento SMTC e salvataggio stato è lo stesso.

## 4. Test

`cargo test --workspace --all-features`: **54 test passati, 0 falliti**.

| Crate | Test |
| --- | ---: |
| `musify-audio` | 24 |
| `musify-core` | 9 |
| `musify-library` | 14 |
| `musify-radio` | 5 |
| `musify` (app) | 2 |
| altri binari/doc-test | 0 |

Nuovi test:

- `backend::sources::tests::registry_routes_searches_and_rejects_unknown_sources`:
  una ricerca passa dal provider registrato, emette `SourceResults` con lo
  schema corretto e le fonti sconosciute restituiscono errore.
- `backend::sources::tests::registry_routes_favorites_toggle_and_resolve`:
  preferiti, toggle e risoluzione `OnDemand` passano dal registro.
- `client::tests::provider_resolves_cached_stations_as_live_streams`:
  `RadioBrowser` espone lo schema `radio` e risolve una stazione in `Live`.

Rispetto a M5.1 (52 test) sono spariti il test dello scheletro Spotify e sono
stati aggiunti tre test sul registro/provider.

## 5. Misure prima/dopo

Stessa macchina, stessa build release, 10 avvii per fase. I valori M5.1 sono in
`docs/M51-FINAL-500.md` e `docs/M51-FINAL-RADIO.md`; i valori M5.2 in
`docs/M52-FINAL-LOCAL.md` e `docs/M52-FINAL-RADIO.md`, con CSV grezzo.

| Metrica | M5.1 | M5.2 | Esito |
| --- | ---: | ---: | --- |
| Startup ready (mediana) | 1.146 ms (1.106–2.105) | **1.110 ms** (1.059–2.370) | migliore |
| Working set a riposo | 185,40 MB | **185,37 MB** | invariato |
| Private bytes a riposo | 170,25 MB | **169,62 MB** | migliore |
| CPU a riposo | 0,31% | **0,16%** | migliore |
| Working set locale | 187,53 MB | **187,35 MB** | invariato |
| Private bytes locale | 171,08 MB | **170,59 MB** | invariato |
| CPU locale | 8,43% (6,54–9,37) | **7,80%** (5,93–9,68) | migliore |
| Working set radio | 194,88 MB | **194,68 MB** | invariato |
| Private bytes radio | 174,81 MB | **175,24 MB** | +0,43 MB |
| CPU radio | 8,28% (6,56–10,29) | **8,43%** (6,23–10,93) | entro il rumore |
| CPU radio normalizzata | 0,69% | **0,70%** | entro il rumore |

La CPU radio cambia di +0,15 punti percentuali su un core, con intervalli che si
sovrappongono quasi completamente; non è una regressione riproducibile ma non
viene presentata come un miglioramento. Anche il calo locale (8,43% → 7,80%) è
dentro la variabilità osservata e non viene attribuito al refactor. Startup e
memoria sono invariati o leggermente migliori. Le misure restano rumorose su
Windows, come già annotato in M5.1.

## 6. Verifica

- `cargo fmt --all -- --check`: pulito.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pulito.
- `cargo test --workspace --all-features`: 54 passati, 0 falliti.
- `cargo build -p musify --release --bin musify`: riuscita.
- Benchmark locale e radio: vedi sezione 5.
- `README.md`, `ARCHITECTURE.md` e `docs/MANUAL-CHECKLIST.md` aggiornati.

## 7. Problemi residui

- M6.1 Podcast non è iniziata: HTTP Range, ricerca iTunes, RSS, SQLite delle
  iscrizioni e UI podcast restano da fare.
- Lo stato della vista Radio conserva campi `radio_query` e testi radio; il
  livello comandi/eventi è generico, ma la UI non è ancora una vista fonti
  parametrica completa.
- I messaggi d'errore delle fonti sono ora generici
  (`source network error: ...`) invece dei precedenti `radio ...`. Il
  comportamento normale e la UI non cambiano.
- La CPU in riproduzione resta sopra il target M5.1; M5.2 non tocca il loop di
  frame Rune.
- La prova di riproduzione podcast e la validazione manuale con feed reale
  appartengono a M6.1.
