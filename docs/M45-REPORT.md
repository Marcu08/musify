# M4.5 — Prestazioni e qualità locale

Data: 2026-11-09
Macchina: Windows 11 Home 10.0.26300, AMD Ryzen 5 5625U (6 core / 12 thread),
Radeon integrata, 7,3 GB RAM, DPI 125%.
Toolchain: Rust 1.99.1 stable x86_64-pc-windows-msvc.
Rune: `757bcb1d1da1b3f80484f36ef610b83ddbcc6003` (invariato).

## Cosa è stato fatto

### 1. Baseline riproducibile

`scripts/bench-m45.ps1` esegue 10 avvii a riposo e 10 con riproduzione. Per ogni
avvio misura:

- tempo fino al file scritto in `RUNE_READY_DIR` (Rune lo crea dopo il primo
  frame e il ritorno di `on_ready`, non al primo pixel puro);
- tempo processo → ready;
- working set e private bytes dopo 2 s di assestamento;
- CPU su una finestra di 5 s, espressa come percentuale di un core e
  normalizzata sul numero di thread logici.

La finestra viene portata in primo piano prima del campionamento, altrimenti
Windows sospende i redraw e la CPU in riproduzione risulta artificialmente 0.
Il benchmark va lanciato con PowerShell 7 (`pwsh`): Windows PowerShell 5.1
corrompe l'UTF-8 nei report.

Misure: [M45-BASELINE.md](M45-BASELINE.md) (prima) e
[M45-FINAL.md](M45-FINAL.md) (dopo). Ogni report ha anche il CSV grezzo.

### 2. Profilo release

`[profile.release]` è passato da `codegen-units=16` + `lto="thin"` a
`codegen-units=1` + `lto="fat"`, mantenendo `strip=true`. La build release è più
lenta, ma non c'è regressione misurabile. Il guadagno è piccolo e in parte
dentro la varianza; è stato tenuto perché coerente in due sessioni:

| Metrica | Baseline | LTO |
| --- | ---: | ---: |
| Ready Rune (mediana) | 1.124 ms | 1.094 ms |
| Working set a riposo | 186,26 MB | 184,92 MB |
| Private bytes a riposo | 170,31 MB | 169,87 MB |
| CPU in riproduzione, un core | 8,42% | 7,02% |

Dettaglio: [M45-OPT-LTO.md](M45-OPT-LTO.md).

### 3. Resampling sinc

`crates/audio/src/resample.rs` ora usa rubato `Async` con sinc finestrato
Blackman2, 64 tap, oversampling 128× e interpolazione quadratica. Il ritardo
iniziale viene tagliato, il chunk parziale viene flushato a fine brano e il
resampler viene azzerato al seek. A parità di frequenza il percorso resta una
copia esatta. Il fallback lineare è disponibile con
`MUSIFY_LINEAR_RESAMPLER=1`, e viene usato automaticamente se rubato non si
inizializza o un chunk fallisce.

Misura isolata con `audio-cli` su 15 s di un file 44,1 kHz verso 48 kHz:

| Resampler | CPU un core | CPU su 12 thread | Underrun |
| --- | ---: | ---: | ---: |
| Lineare | 0,31% | 0,03% | 0 |
| Sinc (rubato) | 1,25% | 0,10% | 0 |

Il sinc costa circa 0,9 punti percentuali di un core in più, ma resta sotto il
2% dell'audio engine da solo. Nell'app completa la differenza non è
distinguibile dalla varianza della UI (8,42% baseline → 7,17% finale, con la
sessione LTO intermedia a 7,02%).

Test automatici: lunghezza output coerente con il rapporto di campionamento,
tono a 1 kHz senza discontinuità evidenti, reset pulito tra stream, più i test
esistenti di decodifica e seek.

### 4. Invalidazione della coda

Il problema era reale e più sottile del solo ring buffer: durante il gapless il
cursore della coda avanza sul brano successivo mentre l'engine sta ancora
suonando il precedente. Se si rimuoveva o spostava quel "prossimo", il vecchio
codice poteva riaprire il brano sbagliato o ripartire da zero.

Ora remove, move e shuffle:

1. salvano la posizione del brano udibile;
2. aggiornano la coda e gli indici dei boundary;
3. riallineano il cursore della coda al brano udibile con `PlaybackQueue::select`;
4. svuotano ring buffer, decode buffer, boundary e decoder tramite l'epoch di
   flush già usato da seek;
5. riaprono il brano udibile alla posizione salvata, oppure il successivo
   dall'inizio se è stato rimosso il brano corrente.

Tre test usano un callback finto che riconosce gli epoch, un ring buffer reale e
tre WAV con ampiezze diverse: rimozione del prossimo, spostamento del prossimo,
rimozione del brano udibile. In tutti i casi il ring buffer contiene solo il
brano atteso.

### 5. Cache copertine e caricamento lazy

La cache copertine della UI ora è limitata a 128 immagini e richiede copertine a
192 pt invece di 320 pt. Con il benchmark a un solo brano non produce un
guadagno misurabile; resta come limite al caso peggiore di librerie grandi.

Il caricamento lazy della libreria è stato implementato, misurato e **scartato**:
l'avvio non è migliorato in modo significativo (1,097 s contro 1,124 s, dentro
la varianza) mentre i private bytes a riposo sono passati da 170,31 MB a
192,24 MB (+22 MB). Dettaglio: [M45-OPT-LAZY.md](M45-OPT-LAZY.md). Il codice è
stato riportato al caricamento sincrono.

### 6. Rune e DPI

L'ultimo `dev` disponibile è `e970367` (circa 20 commit dopo il rev pinnato).
Il diff dei file finestra/win32 non contiene modifiche a scale o DPI. Non è
stato cambiato il rev: il workaround `on_ready` in `crates/ui/src/lib.rs` resta
necessario e documentato nella checklist manuale.

### 7. Checklist manuale

[docs/MANUAL-CHECKLIST.md](MANUAL-CHECKLIST.md) copre dialogo cartelle, tasti
media/SMTC, DPI 100–200%, chiusura forzata e riapertura, modifiche di coda,
resampler e comportamento a riposo.

## Baseline vs finale

| Metrica | Baseline | Finale | Differenza |
| --- | ---: | ---: | ---: |
| Ready Rune (mediana) | 1.124 ms | 1.108 ms | −16 ms |
| Processo → ready (mediana) | 1.147 ms | 1.132 ms | −15 ms |
| Working set a riposo | 186,26 MB | 184,97 MB | −1,29 MB |
| Private bytes a riposo | 170,31 MB | 170,43 MB | +0,12 MB |
| CPU a riposo, un core | 0,31% | 0,16% | −0,15 pp |
| Working set in riproduzione | 188,33 MB | 187,02 MB | −1,31 MB |
| Private bytes in riproduzione | 170,84 MB | 170,05 MB | −0,79 MB |
| CPU in riproduzione, un core | 8,42% | 7,17% | −1,25 pp |
| CPU in riproduzione, 12 thread | 0,70% | 0,60% | −0,10 pp |

Intervalli finali: ready 1.068–1.593 ms; working set a riposo 181,70–185,11 MB;
private bytes a riposo 164,94–171,65 MB; CPU in riproduzione 5,92–9,35% di un
core. La varianza di avvio è dominata da Defender, cache shader e processi
concorrenti: la differenza di 16 ms non è un risultato forte. Anche il calo di
CPU in riproduzione va letto con cautela: la baseline aveva una mediana di
8,42% e la sessione LTO intermedia 7,02%; il finale è 7,17%. Non c'è un
peggioramento misurato, ma non si può attribuire con certezza tutto il calo a
LTO o al resampler.

## Target realistici

I vecchi target (< 300 ms di avvio, < 80 MB di RAM) non sono raggiungibili con
Rune/wgpu su questa macchina senza cambiare UI o renderer. Proposta basata sui
dati:

| Metrica | Target realistico |
| --- | --- |
| Avvio mediano | ≤ 1,3 s (attuale 1,11 s) |
| Avvio p95 | ≤ 2,0 s (attuale max 1,59 s) |
| Working set a riposo | ≤ 190 MB (attuale 185,0 MB) |
| Private bytes a riposo | ≤ 175 MB (attuale 170,4 MB) |
| CPU a riposo | ≤ 0,5% di un core (attuale 0,16%) |
| CPU in riproduzione | ≤ 10% di un core / ≤ 0,8% su 12 thread (attuale 7,17% / 0,60%) |
| Underrun | 0 (attuale 0) |

## Test e qualità

- `cargo test --workspace --all-features`: **40 test passati**, 0 falliti
  (18 audio, 7 core, 14 library, 1 spotify).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pulito.
- `cargo fmt --all -- --check`: pulito.
- `cargo build -p musify --release`: riuscita.

## Cosa non ha funzionato

- **Caricamento lazy della libreria**: nessun guadagno di avvio e +22 MB di
  private bytes. Rimosso.
- **Cache copertine ridotta**: non misurabile con una libreria di un brano.
  Tenuta solo come limite di memoria, non come ottimizzazione dimostrata.
- **LTO**: guadagno piccolo e vicino alla varianza; tenuto per assenza di
  regressioni, non come vittoria netta.
- **Sinc**: costo CPU chiaramente misurabile in isolamento (+0,94 pp di un core).
  Tenuto per la qualità, con fallback lineare; nell'app completa è coperto dai
  redraw della UI.

## Limiti

- Il segnale "ready" di Rune non è il primo pixel puro; include il primo frame e
  `on_ready`.
- Il campionamento CPU a 5 s non distingue variazioni sotto il punto percentuale.
- La cache copertine e il caricamento della libreria non sono stressati con una
  libreria da decine di migliaia di file in questo report.
- La checklist manuale non è stata eseguita automaticamente: richiede una
  sessione Windows interattiva.
