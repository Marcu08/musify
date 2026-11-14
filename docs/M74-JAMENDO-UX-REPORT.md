# M7.4 — Jamendo veloce da usare

Data: 2026-11-14
Base: `d2372b2` (M7.2 committata)
Build: release, Rune invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`
Crate modificati: `musify-core`, `musify-jamendo`, `musify-ui`, `musify-app`

## 1. Verifica della documentazione ufficiale

Documentazione letta il 2026-11-14:

- <https://developer.jamendo.com/v3.0/tracks>
- <https://developer.jamendo.com/v3.0/read-methods>
- <https://developer.jamendo.com/v3.0/playlists>
- <https://developer.jamendo.com/v3.0/playlists/tracks>
- <https://developer.jamendo.com/v3.0/playlists/hots>
- <https://developer.jamendo.com/v3.0/radios>
- <https://developer.jamendo.com/v3.0/radios/stream>
- <https://devportal.jamendo.com/api_terms_of_use>

### Cosa esiste davvero

| Elemento | Documentazione | Usato in M7.4 |
| --- | --- | --- |
| `order=popularity_week` | Sì, ordine per popolarità della settimana; i rating forzano `desc` | Sì, "Popolari settimana" |
| `order=popularity_month` | Sì | Sì, "Popolari mese" |
| `order=popularity_total` | Sì | Sì, "Sempre popolari" |
| `order=releasedate_desc` | `releasedate` è nell'elenco degli `order`, suffisso `_desc` ammesso | Sì, "Novità" |
| `limit` / `offset` | `limit` max 200 (default 10), `offset` intero | Sì, pagine da 60, max 200 |
| `fullcount=true` | Sì, aggiunge `results_fullcount` ma la documentazione avverte che costa | No, omesso di proposito |
| `search` | Testo libero su brano, album, artista e tag | Sì, ricerca testuale |
| `tags` | Uno o più tag, semantica AND | Sì, generi dai pulsanti |
| `boost` | Rating da usare con la ricerca per non perdere la rilevanza | Sì, quando c'è testo e ordinamento per popolarità |
| `featured=1` | Sì, selezione dei music manager Jamendo | Verificato, non esposto: le Selezioni usano le playlist hot |
| `/playlists/hots` | Sì, "Jamendo hot selection of playlists", restituisce solo gli id | Sì |
| `/playlists/tracks` | Sì, restituisce le playlist con i brani annidati; più id separati da spazio o `+` | Sì |
| `/radios` | Sì, elenco delle radio Jamendo | No |
| `/radios/stream` | Esiste, ma la pagina ufficiale avverte: "The stream link returned is not more working, and it could be never fixed" | No, omesso |
| `/charts/track` | Sì, restituisce solo id/posizione e va incrociato con `/tracks` | No, avrebbe richiesto una seconda richiesta per ogni pagina |

La documentazione sui parametri condivisi conferma che i valori multipli
(`[]integer`, `[]string`) si separano con uno spazio o un `+`. Per le playlist
hot abbiamo usato `id=<id1> <id2> ...` con una sola richiesta; il limite di
entità aggregate è 400, quindi la risposta può essere troncata.

### Verifiche fatte sull'API reale

Fatte con la chiave già presente nell'ambiente, senza mai stamparla, salvarla o
includerla in un file. Le richieste sono state distanziate di almeno 1,3 s.

- `tags=jazz&order=popularity_month&limit=5&offset=0` e `offset=5` hanno
  restituito id diversi: la paginazione funziona.
- `tags=jazz&search=love&order=releasedate_desc` ha restituito 5 risultati.
- `tags=jazz&search=love&boost=popularity_month` ha restituito 5 risultati.
- `playlists/hots?limit=10` ha restituito 10 id; `playlists/tracks` con quegli
  id ha restituito 5 playlist utilizzabili e 11 brani totali (le altre erano
  vuote o non accessibili).
- **Comportamento anomalo**: la stessa query valida ha restituito in momenti
  diversi `status=success, code=0, results_count=5` e
  `status=success, code=0, results_count=0`. È successo con
  `popularity_week`, `popularity_month` e `releasedate_desc`. Il client ritenta
  una volta e, se esiste, usa la cache precedente. È documentato nei limiti.
- `tags=jazz&order=listens_total` ha restituito 0 in una prova, mentre
  `downloads_total` ha restituito 5. Non usiamo questi due ordini.

### Termini

- Uso gratuito non commerciale; per usi commerciali serve un accordo con
  Jamendo.
- Le credenziali sono personali e non vanno condivise.
- Jamendo può imporre limiti di frequenza; il client resta a 1 richiesta/s.
- La cache deve essere limitata a quanto necessario e non deve creare un accesso
  offline al catalogo. Musify usa TTL 24 h e non salva audio.
- Attribuzione: artisti come autori, Jamendo come provider, backlink diretto
  alla pagina del brano. Resta visibile in ogni riga (licenza CC + link
  Jamendo) e in SMTC tramite album/artista.
- Nessun uso di `audiodownload`/`audiodownload_allowed`: solo l'URL `audio` in
  streaming.

## 2. Cosa è stato fatto

### Home a generi

- Griglia di 12 generi: Jazz, Rock, Elettronica, Ambient, Pop, Hip hop,
  Classica, Lounge, Folk, World, Metal, Colonne sonore. I tag API sono in
  `i18n.rs` insieme alle etichette italiane; i componenti non contengono testi.
- All'apertura della vista il genere predefinito è Jazz con "Popolari
  settimana": la lista parte senza compilare nulla. Un clic su un altro genere
  sostituisce la lista.
- Quattro ordinamenti: Popolari settimana / mese / sempre, Novità. Il selettore
  è una riga di pill; il genere attivo e il conteggio sono visibili.
- La finestra predefinita passa da 1120×620 a 1120×680 per lasciare spazio alla
  home; il minimo resta 980×600.

### Ricerca e combinazione

- Il campo di testo resta per titolo/artista/album. Il vecchio campo "Tag /
  genere" è stato rimosso: i generi si scelgono dai pulsanti.
- Genere + ordinamento + testo sono combinati nella stessa query. Con testo
  libero e ordinamento per popolarità il client manda `boost` invece di
  `order`, così la rilevanza del testo non viene persa. Con "Novità" manda
  `order=releasedate_desc`.
- "Cerca" (e Invio nel campo) aggiunge il filtro locale `refresh=1` e salta la
  cache fresca; i clic su genere/ordinamento e "Carica altri" usano la cache
  entro il TTL. La cache è SQLite, 24 h.

### Paginazione

- Pagina da 60, `offset` incrementale, "Carica altri" in fondo. La lista resta
  `.virtual_rows` con chiavi stabili; le pagine successive vengono accodate
  senza duplicati.
- `limit` è validato e limitato a 1..200 lato provider. `fullcount` non viene
  richiesto.

### Azioni rapide

Sopra la lista:

- **Riproduci tutto**: risolve gli id visibili (solo DB per Jamendo) e manda
  `Command::PlayTracks`.
- **Mescola**: stessa lista in ordine mescolato; non attiva il toggle globale
  di shuffle.
- **Aggiungi tutto alla coda**: `Command::Enqueue`.
- **Salva come playlist**: crea una playlist M7.1 con nome automatico
  `Jamendo · <genere> · <ordinamento>` (o `Jamendo · <selezione>`), con gli
  snapshot e la licenza. Se l'aggiunta fallisce, la playlist vuota viene
  eliminata.

Doppio clic su una riga o `Enter` sulla riga selezionata riproducono il singolo
brano. Il pulsante "Ascolta" resta.

### Precaricamento e prestazioni percepite

- All'avvio, solo se `MUSIFY_JAMENDO_CLIENT_ID` è presente, un thread una tantum
  legge i generi più usati da `genre_usage` (fallback:
  jazz, electronic, rock, pop) e scarica la prima pagina "Popolari settimana"
  nella stessa cache delle ricerche. `prefetch_started` impedisce un secondo
  passaggio.
- Il precaricamento non fa polling: finisce e il thread termina. Una ricerca
  utente incrementa una `generation` atomica e ferma il precaricamento.
- La ricerca è cache-first entro 24 h: il primo clic su un genere precaricato è
  una lettura SQLite. Se la rete fallisce, si usa una cache più vecchia con
  `offline=true`. Se l'API risponde `success` con 0 risultati a una query di
  genere, il client ritenta una volta; se anche il retry è vuoto, usa la cache
  stale se c'è.
- La latenza è mostrata nella UI: `Da cache · N ms` oppure
  `Risultati in N ms`. Il tempo parte dal clic e finisce quando
  `SourceResults` è applicato allo stato (lista ricostruita). Non include il
  frame GPU successivo.

### Annullamento

- `SourceRegistry` ha già un `request_id` per fonte: i risultati superati
  vengono scartati.
- `Backend::source_cancel` incrementa il request id e chiama
  `SourceProvider::cancel`. Jamendo incrementa la stessa `generation` usata da
  ricerca, retry, rate limiter e selezioni.
- Il rate limiter e i backoff dormono in passi da 25 ms e controllano la
  generazione: una nuova ricerca interrompe l'attesa e non parte la richiesta
  successiva.
- Uscendo dalla vista Jamendo viene annullata la richiesta in corso. Cambiare
  genere o ordinamento annulla la precedente.

### Selezioni

- `source_selections("jamendo")` chiama `/playlists/hots` (8 id) e poi
  `/playlists/tracks` con gli id separati da spazio; tiene fino a 4 playlist con
  almeno un brano e le salva in `selections`.
- La sezione "Selezioni Jamendo" mostra chip compatti con titolo e copertina se
  già in cache. Un clic legge i brani dal DB (nessuna rete) e riusa la lista
  virtualizzata e le azioni rapide. "Torna ai generi" ripristina la griglia.
- Se l'endpoint non restituisce selezioni utilizzabili, la sezione è assente.
  Il pulsante "Aggiorna selezioni" forza un nuovo tentativo.

### Scorciatoie e componenti

- `Alt+←/→`: genere precedente/successivo.
- `Enter`: riproduce il brano selezionato.
- `Ctrl+Enter`: riproduce tutto.
- `Alt+Enter`: mescola e riproduce.
- Le stringhe nuove sono solo in `i18n.rs`; i colori solo in `theme.rs`
  (`theme::TRANSPARENT` incluso). I componenti `choice_chip` e le azioni di
  lista sono riusabili.
- Radio e Podcast avevano già il preferito/iscrizione a un clic. Non è stato
  aggiunto "Riproduci tutti gli episodi" ai podcast: la ripresa per episodio
  richiederebbe una mappa coda → episodio che oggi non esiste, e non volevamo
  introdurre una funzione con la ripresa rotta. La nota è nei limiti.

## 3. Misure prima/dopo

### Metodo

- Build release, stessa macchina, server finto `wiremock` con 40 ms di ritardo
  simulato per richiesta e 100 brani in risposta.
- Baseline: commit `d2372b2`, prima delle modifiche, flusso M7.2 (tag search,
  `limit=100`, network-first). Il vecchio flusso non aveva un clic sul genere;
  la misura equivalente è la ricerca per tag.
- Dopo: stesso harness, `cargo test -p musify-jamendo --release --test latency
  -- --nocapture`.
- I tempi sono letti dal provider fino alla lista di `SourceItem` pronta. La UI
  applica 60 elementi in **22,1 µs (debug) / 41,5 µs (release)**, misurati con
  `apply_event`; il frame GPU successivo non è incluso. Il valore mostrato
  nella UI è lo stesso intervallo clic → evento applicato.

Uscita grezza dell'harness dopo le modifiche (due esecuzioni):

```text
latency cold100=65.2098ms warm100=4.1569ms cold60=55.8704ms warm60=4.5128ms prefetch=59.0225ms prefetched_click=3.5448ms
latency cold100=64.4746ms warm100=5.7993ms cold60=53.1985ms warm60=3.9270ms prefetch=52.5419ms prefetched_click=3.7806ms
```

### Risultati

| Scenario | Prima (M7.2) | Dopo (M7.4) | Variazione |
| --- | ---: | ---: | ---: |
| Cache fredda, 100 risultati, tag search | 67,55 ms | 64,5–65,2 ms | −2,3…−3,1 ms (la rete domina) |
| Cache calda, 100 risultati, stessa query | 62,43 ms | **4,2–5,8 ms** | **−56,6…−58,3 ms (~11–15×)** |
| Cache fredda, 60 risultati, genere + Popolari settimana | non esisteva | 53,2–55,9 ms | — |
| Cache calda, 60 risultati, stesso genere/ordine | non esisteva | **3,9–4,5 ms** | — |
| Precaricamento una tantum di un genere | non esisteva | 52,5–59,0 ms | una richiesta, in background |
| Primo clic su un genere precaricato | non esisteva | **3,5–3,8 ms** | cache hit |

La cache fredda è dominata dai 40 ms simulati di rete: il guadagno del client
è piccolo. Il guadagno vero è la cache calda/precaricata, dove il tempo scende
da ~62 ms a ~4–6 ms. Con la rete reale i valori assoluti sono più alti, ma il
rapporto tra cold e warm resta.

### Nota onesta

- Non pubblichiamo tempi reali dell'API Jamendo: variano con la rete, il carico
  del servizio e la posizione, e la chiave è personale. Le verifiche reali
  hanno controllato endpoint, parametri e forma delle risposte, non i tempi.
- Il precaricamento parte all'avvio solo se la chiave è presente e la fonte è
  registrata. Se l'utente cerca subito, il precaricamento si ferma; il primo
  clic può quindi essere freddo.
- La cache-first significa che un risultato può avere fino a 24 h. "Cerca"
  forza un aggiornamento; non c'è un refresh automatico in background.

## 4. Test

Tutti i test usano `wiremock` e nessuna rete reale. Il client id nei test è
sintetico e generato a runtime.

Jamendo (25 test, `cargo test -p musify-jamendo`):

- parsing metadati e ignoramento dei campi download;
- cache fresca senza rete e fallback a cache stale con `offline=true`;
- rate limit 429, retry e nessuna fuga del client id;
- errore API redatto;
- preferiti persistiti e `resolve` solo DB;
- artwork scaricato e in cache;
- ordinamento e paginazione (tag, `order`, `limit`, `offset`);
- `releasedate_desc` per Novità;
- `boost` quando c'è testo e popolarità;
- refresh esplicito che bypassa la cache fresca;
- annullamento di una richiesta ritardata;
- retry una volta quando una query di genere restituisce 0 risultati;
- precaricamento della prima pagina e idempotenza;
- selezioni hot: fetch, parsing di `duration` come stringa, cache e lettura DB;
- uso dei generi registrato;
- clamp di `limit` e chiavi di cache che includono ordine/offset;
- rate limiter con attesa e annullamento.

UI (10 test, `cargo test -p musify-ui`):

- apertura Jamendo con Jazz + Popolari settimana;
- genere, ordinamento e offset nelle query;
- "Cerca" con `refresh=1`;
- apertura selezione e ritorno ai generi;
- azioni di lista con tutti gli id e nome playlist automatico;
- richiesta selezioni una sola volta e retry dopo risposta vuota;
- annullamento uscendo dalla vista;
- applicazione di 60 risultati in 22,1 µs (debug) / 41,5 µs (release).

App (4 test, `cargo test -p musify`): registry search, favorites/resolve,
risoluzione in blocco e evento selezioni.

Stato finale: `cargo fmt --all` pulito,
`cargo clippy --workspace --all-targets -- -D warnings` pulito,
`cargo test --workspace` con **113 passati, 0 falliti, 2 ignorati** (i due test
podcast di rete reale).

## 5. Limiti dell'API scoperti

1. **Risposte vuote intermittenti**: la stessa query di genere può restituire
   5 risultati e poi 0 con `status=success, code=0`. Non è un errore HTTP e non
   c'è un messaggio. Il client ritenta una volta e usa la cache stale se
   disponibile. Senza cache la lista può risultare vuota.
2. **`search` non è affidabile per i generi**: già in M7.2 `search=jazz` dava 0
   mentre `tags=jazz` dava 100. La home usa i tag dei pulsanti, non il testo
   libero, per i generi.
3. **Playlist hot sporadiche**: `/playlists/hots` restituisce 10 id, ma
   `/playlists/tracks` può restituirne solo 5 con 2-3 brani ciascuna; alcune
   playlist sono vuote o non accessibili. Mostriamo solo quelle con almeno un
   brano.
4. **Radio Jamendo non funzionante**: `/radios/stream` è documentato come
   "not more working, and it could be never fixed". Non è stato implementato.
5. **`limit` massimo 200**: non si può chiedere l'intero catalogo in una volta.
   La paginazione è obbligatoria.
6. **`fullcount` costoso**: disponibile, ma la documentazione avverte; non lo
   usiamo. `has_more` è dedotto dalla dimensione della pagina.
7. **Entità aggregate max 400**: `/playlists/tracks` può troncare i brani delle
   playlist. Non è un problema per 4 selezioni, ma va ricordato.
8. **Cache e offline**: i termini vietano un accesso offline al catalogo. La
   cache è un fallback limitato a 24 h, non una libreria scaricata.
9. **`order` con testo**: un `order` diverso da `relevance` perde la rilevanza
   del testo; per questo con testo libero usiamo `boost`. "Novità" è
   l'eccezione esplicita: l'utente chiede l'ordine per data.

## 6. Checklist manuale

Aggiornata in [docs/MANUAL-CHECKLIST.md](MANUAL-CHECKLIST.md), sezione 10:
home, generi, ordinamenti, cache fredda/calda, refresh, azioni rapide, doppio
clic/Enter, Carica altri, selezioni, scorciatoie, annullamento, attribuzione,
seek, preferiti, contenuto di `%APPDATA%`, precaricamento una tantum, riposo,
assenza della chiave e controllo log.

Non è stata eseguita una prova click-through con audio reale in questa
sessione: la checklist è pronta per una prova manuale su build release. I test
automatici coprono contratto API, cache, annullamento, rate limit e stato UI,
non l'uscita audio o l'overlay SMTC.

## 7. Cose rimaste fuori

- La prova manuale completa (ascolto, seek udibile, SMTC, salvataggio da UI)
  richiede una sessione audio reale.
- Il precaricamento copre solo la prima pagina dei primi 4 generi e una volta
  per avvio; se viene annullato non riprende. È una scelta per non avere polling.
- La cache-first può mostrare risultati fino a 24 h. "Cerca" forza il refresh,
  ma non c'è un timer di aggiornamento in background.
- Le selezioni sono le playlist hot di Jamendo, non un browser completo di
  playlist; l'endpoint può essere sporadico.
- La radio Jamendo è omessa per il warning ufficiale.
- I podcast non hanno "Riproduci tutti gli episodi": la ripresa per episodio
  richiederebbe una mappa coda → episodio. Il preferito/iscrizione a un clic
  esisteva già e resta invariato.
- `resolve` è solo DB: un vecchio `SourceItem` non più in cache non è
  riproducibile senza una nuova ricerca; le playlist salvano l'URL audio e
  possono fallire se l'URL Jamendo cambia.
- Il client id va impostato prima dell'avvio; aggiungerlo a runtime richiede un
  riavvio.

## 8. File principali toccati

- `crates/core/src/traits.rs`, `crates/core/src/event.rs`: `SourceSearch` con
  `cached`/`has_more`, `SourceSelections`, metodi opzionali `cancel`,
  `prefetch`, `selections`.
- `crates/jamendo/src/client.rs`: ordinamenti, paginazione, cache-first,
  refresh, retry su risposte vuote, precaricamento, annullamento, selezioni.
- `crates/jamendo/src/store.rs`: `genre_usage`, `selections`, TTL e test.
- `crates/jamendo/src/model.rs`: `JamendoSelection`.
- `crates/app/src/backend/sources.rs`, `crates/app/src/backend/mod.rs`:
  annullamento, selezioni, risoluzione in blocco, play/enqueue/save playlist.
- `crates/ui/src/state.rs`, `crates/ui/src/view/jamendo.rs`,
  `crates/ui/src/view/common.rs`, `crates/ui/src/i18n.rs`,
  `crates/ui/src/theme.rs`, `crates/ui/src/backend.rs`: home, azioni,
  scorciatoie, latenza, stati.
- `crates/jamendo/tests/latency.rs`: harness di misura.
- `docs/MANUAL-CHECKLIST.md`, `README.md`, `ARCHITECTURE.md` e questo report.
