# M5.1 — Rifinitura

Data: 2026-11-10
Build: release, Rune invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`

## 1. CPU in riproduzione

### Cosa forzava i ridisegni

Il primo passo è stato contare i commit della UI, non solo guardare la CPU.
Con la riproduzione locale attiva la UI produceva **circa 5,8 commit/s**, tutti
causati da `Event::PositionChanged` (un evento ogni 200 ms). Non c'erano
animazioni continue, polling o ridisegni a riposo: il costo era tutto nei frame
di avanzamento.

Il numero di commit non spiega però la CPU: il contatore dei frame ha mostrato
**circa 60 frame/s** durante la riproduzione, contro 0 a riposo, mentre i commit
logici erano solo 2–8 ogni 5 s. La finestra minimizzata scendeva a 0,94% di un
core. La causa è quindi un loop di presentazione della UI visibile, non il
decoder o la seek bar da sola; gli esperimenti sotto non l’hanno isolato in modo
affidabile.

### Cosa è stato provato e cosa è stato scartato

Sono state provate, una alla volta e misurate con lo stesso harness:

- coalescing dei commit di posizione a 200/250/500/1000 ms;
- `Tx::instant()` e `Tx::settle()` sui commit;
- `El::motion(Spring::NONE)` sulla barra di avanzamento;
- soppressione del wake della UI per i soli `PositionChanged`, con timer di
  polling;
- throttle dei wake a 1 s.

Nessuna variante ha prodotto un guadagno stabile. Le misure della stessa build
oscillavano tra **3,6% e 10,5% di un core** anche a parità di configurazione,
mentre la CPU a finestra minimizzata era **0,94%**: il costo è nel disegno della
UI quando la finestra è visibile, non nel motore audio. Con l'analisi dei frame
è emerso che durante la riproduzione Rune resta a **circa 60 frame/s** anche se
i commit logici sono 2–8 ogni 5 secondi; a riposo i frame sono zero. Il loop di
frame non è stato ricondotto a un singolo elemento con certezza, e tutte le
modifiche provate sono state rimosse per non cambiare il comportamento della UI
senza un beneficio misurato.

### Misura finale

La build finale è quindi tornata al comportamento M5 per la UI. La misura
finale nella stessa sessione è:

| Metrica | Locale | Radio MANGORADIO |
| --- | ---: | ---: |
| CPU un core (mediana) | **8,43%** (6,54–9,37) | **8,28%** (6,56–10,29) |
| CPU 12 thread | **0,70%** | **0,69%** |
| Working set | **187,53 MB** | **194,88 MB** |
| Private bytes | **171,08 MB** | **174,81 MB** |
| Startup ready | **1,146 s** (1,106–2,105) | — |

I file [M51-FINAL-500.md](M51-FINAL-500.md) e
[M51-FINAL-RADIO.md](M51-FINAL-RADIO.md) contengono la misura grezza della
build finale; il valore esatto va letto lì. **Il target di riduzione CPU non è
stato raggiunto**: il report lo dichiara esplicitamente invece di arrotondare
un risultato non riproducibile.

## 2. Radio

### Ricerche annullabili

Ogni ricerca riceve un `request_id` crescente. Se parte una nuova ricerca, il
thread vecchio non pubblica il risultato; l'interfaccia riceve solo l'ultimo.
Il timeout HTTP resta 20 s per richiesta, quindi una richiesta di rete non
blocca la UI e non può sovrascrivere una ricerca più recente.

### TTL della cache

`radio.sqlite3` conserva `cached_at` per ogni stazione. La ricerca offline usa
solo risultati più recenti di **7 giorni** (configurabile con
`MUSIFY_RADIO_CACHE_TTL_HOURS`). I preferiti non scadono. Se la cache è
scaduta e la rete non risponde, la UI mostra l'errore invece di risultati
obsoleti.

### Favicon con cache limitata

Le icone delle stazioni vengono scaricate in background, ridimensionate a 64 px
e salvate in `radio-icons/` accanto a `radio.sqlite3`. La cartella è limitata a
**256 file / 16 MB**; le icone più vecchie vengono eliminate. La UI parte con il
badge "FM" e aggiorna la riga quando arriva `SourceArtwork`. Il download è
limitato alle prime 8 stazioni per ricerca e non blocca la UI.

### Riconnessione Ogg

`HttpStream` espone una `generation` che avanza a ogni nuova connessione. Quando
cambia, il decoder Symphonia viene riaperto da zero: il vecchio demuxer Ogg non
prova a interpretare il nuovo stream come continuazione del precedente. Il
buffer viene svuotato sulla nuova connessione per evitare di miscelare la coda
del vecchio stream con l'header nuovo. Il test di trasporto verifica la
generation; la prova con una stazione Ogg reale resta manuale.

### HLS

**Non implementato.** Il costo non è ragionevole con l'architettura attuale:

- serve un parser m3u8 e la selezione delle variant;
- servono segmenti MPEG-TS o fMP4, mentre Symphonia non ha un demuxer MPEG-TS e
  l'assemblaggio fMP4 è un lavoro a sé;
- servono gestione di playlist live, discontinuità, chiavi AES-128 e sequenze;
- il buffer/backoff attuale è pensato per un flusso continuo, non per segmenti.

È stato aggiunto un rifiuto esplicito: URL `.m3u8` o content-type `mpegurl`
producono `AudioError::Unsupported("gli stream HLS non sono supportati")`, che
la UI mostra come errore di apertura. Nessun rumore o crash.

### Prova lunga

Aggiunto `scripts/soak-radio.ps1`: avvia la radio, porta la finestra in primo
piano ogni 5 s, campiona working set, private bytes, CPU, thread e handle ogni
minuto, scrive il CSV a ogni campione e produce un report con primo/ultimo,
min/max e crescita.

La prova completa da 2 ore **non è stata completata in questa sessione**: il
processo avviato in background è terminato dopo circa 2 minuti e il tentativo
successivo è stato interrotto. È stata eseguita una prova di 5 minuti con la
build finale: private bytes da 176,36 a 175,72 MB (−0,64 MB), working set da
194,55 a 54,10 MB per il trim di Windows, CPU mediana 4,41% di un core. I
private bytes sono stabili; il working set non è una metrica affidabile quando
la finestra perde il foreground. Il risultato è in
[M51-SOAK-5M.md](M51-SOAK-5M.md). Lo script è pronto per la prova da 2 ore.

La prova breve di validazione (3 minuti, 20 s) ha mostrato lo stesso fenomeno:
working set 194,7 → 70,1 MB, private 174,7 → 173,9 MB. Per questo il report
lungo usa il foreground periodico e considera i private bytes la metrica
principale contro i leak.

## 3. Test

`cargo test --workspace --all-features`: **52 test passati**, 0 falliti
(24 audio, 9 core, 14 library, 4 radio, 1 spotify).
`cargo clippy --workspace --all-targets --all-features -- -D warnings`: pulito.
`cargo fmt --all -- --check`: pulito.

Nuovi test M5.1:

- HLS rifiutato con errore `Unsupported`;
- generation della riconnessione HTTP;
- TTL della cache radio (età artificiale dei risultati, cache fresca vs scaduta);
- preferenze/cache radio esistenti restano verdi.

## 4. File toccati

- `crates/ui/src/state.rs`: eventi/artwork radio; nessuna modifica CPU trattenuta;
- `crates/ui/src/backend.rs`: eventi radio con request id e artwork;
- `crates/ui/src/view/content.rs`/`view/mod.rs`: stato pausa e rendering;
- `crates/core/src/event.rs`, `domain.rs`: `request_id`, `SourceArtwork`,
  `image_path`;
- `crates/audio/src/decoder.rs`: rifiuto HLS, riapertura decoder su generation;
- `crates/audio/src/http_stream.rs`: generation, buffer reset, content type;
- `crates/radio/src/client.rs`, `store.rs`, `station.rs`: TTL, icone, cache;
- `crates/app/src/backend.rs`: ricerca cancellabile, prefetch icone;
- `scripts/bench-m45.ps1`, `scripts/soak-radio.ps1`;
- `docs/MANUAL-CHECKLIST.md`, README/ARCHITECTURE.

## 5. Limiti

- Il confronto CPU tra sessioni è influenzato dal riavvio del server/ambiente;
  non è stato possibile ottenere un A/B stabile. La riduzione CPU **non è
  confermata** e le modifiche relative sono state rimosse.
- La seek bar resta aggiornata dal flusso M5 (eventi ogni 200 ms). Non è stato
  introdotto un limite a 4–10 Hz perché le prove non hanno mostrato un guadagno
  riproducibile.
- Ogg reale a metà stream non è stato provato con una stazione pubblica stabile;
  la riapertura del decoder è coperta dal cambio di generation e dal test HTTP,
  non da un test Ogg di lunga durata.
- HLS resta non supportato, con errore chiaro.
- La prova da 2 ore non è conclusa: la sessione ha prodotto solo la prova di 5
  minuti. Lo script `scripts/soak-radio.ps1` è pronto e va eseguito senza
  interrompere il processo.
