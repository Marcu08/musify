# M8 — Qualità della vita

Data: 2026-11-15
Base: `806de9f` (M7.4)
Build: release, Rune invariato a `757bcb1d1da1b3f80484f36ef610b83ddbcc6003`

M8 non aggiunge funzioni musicali: rende più veloce il ciclo di sviluppo, più
semplice l'installazione e più facili da diagnosticare le esecuzioni senza
terminale. Il profilo `release` resta invariato.

## 1. Profilo di build `fast`

In `Cargo.toml`:

```toml
[profile.fast]
inherits = "release"
lto = "thin"
codegen-units = 16
```

- `fast` eredita da `release` (`opt-level = 3`, `strip = true`, panic unwind) e
  cambia solo LTO e codegen unit. Non è il profilo di distribuzione.
- Uso quotidiano: `cargo build -p musify --profile fast` →
  `target/fast/musify.exe`.
- Distribuzione: `cargo build -p musify --release` →
  `target/release/musify.exe`.
- `cargo run -p musify --profile fast` è disponibile per una prova rapida.

### Misure di compilazione

Le misure sono state fatte con `scripts/bench-build-m8.ps1` (build clean in una
target directory isolata, poi `touch` di `crates/app/src/main.rs` e rebuild
incrementale). Dati grezzi in [M8-BUILDS.md](M8-BUILDS.md).

| Profilo | Build clean | Build incrementale | Exe | SHA-256 clean |
| --- | ---: | ---: | ---: | --- |
| `fast` | 622,5 s (10m22,5) | 194,6 s (3m14,6) | 37,24 MB | `82525E…F9F2` |
| `release` | 1320,5 s (22m00,5) | 575,1 s (9m35,1) | 33,34 MB | `31E372…B545` |

La build incrementale è un `touch` di `crates/app/src/main.rs` nella stessa
target directory, non una build da zero. I tempi includono dipendenze, build
script, `winresource` e link. La build release clean è ~2,1× più lenta della
fast clean; l'incrementale release è ~3× più lento del fast. Il profilo
`release` resta `lto = "fat"`, `codegen-units = 1`, `strip = true`.

### Avvio e CPU in riproduzione

Le misure usano `scripts/bench-m45.ps1` con `-Profile release` e
`-Profile fast`, 10 avvii per profilo, segnale `RUNE_READY_DIR`, 2 s di
assestamento e 5 s di campionamento CPU. La libreria di prova è creata una volta
con `library-cli` release: il profilo riguarda solo l'app misurata. La
riproduzione parte con `MUSIFY_AUTOPLAY=1`; il benchmark rimuove
`MUSIFY_JAMENDO_CLIENT_ID` per misurare solo il percorso locale, evitare rete
all'avvio e non lasciare il client id nei log. Dati grezzi in
[M8-RELEASE.md](M8-RELEASE.md) e [M8-FAST.md](M8-FAST.md).

| Metrica | `fast` | `release` |
| --- | ---: | ---: |
| Startup: ready Rune (mediana, min–max) | 1732 ms (1701–2549) | 1778 ms (1690–3241) |
| Startup: processo → ready | 1762 ms (1735–3966) | 1811 ms (1723–4594) |
| Riposo: CPU un core | 0,31 % | 0,31 % |
| Riproduzione: CPU un core | 9,36 % (4,36–10,61) | 9,04 % (7,81–13,43) |
| Riproduzione: CPU normalizzata | 0,78 % | 0,75 % |
| Riproduzione: private bytes | 171,17 MB | 171,22 MB |

Il benchmark `fast` usa l'exe prodotto dal rebuild incrementale nella stessa
directory del profilo; il codice e il profilo sono gli stessi della build clean.
La differenza di avvio (~46 ms a favore di fast) e di CPU in riproduzione
(~0,3 punti percentuali a favore di release) è dentro la varianza delle 10
esecuzioni: i due profili sono equivalenti all'uso. `release` resta la build di
distribuzione per dimensione e LTO. Dati grezzi e CSV in
[M8-FAST.md](M8-FAST.md) e [M8-RELEASE.md](M8-RELEASE.md).

## 2. Hook Git

`.githooks/pre-push` legge i riferimenti da stdin. Se tutti i riferimenti sono
`refs/tags/*`, esegue solo lo scan segreti e salta clippy e test: un tag non
cambia il codice e il commit è già stato verificato. Un push di branch o misto
esegue il controllo completo.

```sh
# tag-only: salta clippy/test
refs/tags/m8 <sha> refs/tags/m8 <sha>

# branch o misto: controlli completi
refs/heads/main <sha> refs/heads/main <sha>
refs/tags/m8 <sha> refs/tags/m8 <sha>
```

`-j 1` è stato rimosso. La concorrenza si limita solo quando serve:

```powershell
$env:MUSIFY_CARGO_JOBS = "1"
```

Senza la variabile Cargo usa tutti i core. Verifica manuale del ramo tag-only:

```text
check-secrets: nessun segreto rilevato nell'albero
pre-push: push di soli tag, salto clippy e test
```

## 3. Icona e informazioni di versione

- Sorgente: `assets/icon.png` (512×512).
- Icona multirisoluzione: `assets/musify.ico` (256, 128, 64, 48, 32, 16),
  generata con ImageMagick:
  `magick assets/icon.png -define icon:auto-resize=256,128,64,48,32,16 assets/musify.ico`.
- `crates/app/build.rs` usa `winresource` come build-dependency solo per
  `cfg(windows)`. Incorpora l'icona e le stringhe di versione.
- Su host non Windows il build script è un no-op.

Proprietà verificate sull'exe release:

```text
ProductName      : Musify
FileDescription  : Musify - il tuo player musicale desktop
FileVersion      : 0.1.0
ProductVersion   : 0.1.0
CompanyName      : Musify
LegalCopyright   : MIT
OriginalFilename : musify.exe
```

L'icona estratta dall'exe è 32×32 (Windows sceglie la risoluzione adatta dal
file multi-size). La build è stata ripetuta in una target directory separata:
non è bit-identica, ma le differenze sono solo metadati (24 byte su 34.954.752:
timestamp PE, timestamp delle risorse e GUID di debug). Codice, icona e stringhe
di versione sono identici. Analisi completa in [M8-BUILDS.md](M8-BUILDS.md).

## 4. Installazione locale

`scripts/install-local.ps1`:

- copia `musify.exe` in `%LOCALAPPDATA%\Musify\musify.exe`;
- crea `Musify.lnk` sul desktop e nel menu Start, con icona e working directory
  dell'installazione;
- non richiede privilegi di amministratore;
- non crea, cancella o modifica `%APPDATA%\Musify` (configurazione, coda, cache,
  log restano dell'app);
- lo stesso script aggiorna: ricompila e riesegue, sovrascrivendo l'exe;
- se Musify è aperto e l'exe è bloccato, lo script si ferma con un messaggio che
  chiede di chiuderlo;
- supporta `-WhatIf` per vedere le operazioni senza eseguirle, `-Profile fast`
  per installare `target\fast\musify.exe` e `-Source` per un percorso esplicito.

Verifica `-WhatIf` eseguita:

```text
WhatIf: Esecuzione dell'operazione "Copia Musify" sulla destinazione "...\AppData\Local\Musify\musify.exe".
WhatIf: Esecuzione dell'operazione "Crea collegamento desktop" sulla destinazione "...\Desktop\Musify.lnk".
WhatIf: Esecuzione dell'operazione "Crea collegamento menu Start" sulla destinazione "...\Start Menu\Programs\Musify.lnk".
WhatIf: nessuna modifica effettuata.
```

La prova reale con copia e collegamenti è nella checklist manuale (sezione 12).

## 5. Log su file

`crates/app/src/logging.rs` implementa un `MakeWriter` che scrive sia su stdout
sia su file. Il percorso è `%APPDATA%\Musify\logs\musify.log` (o
`MUSIFY_DATA_DIR\logs` quando la variabile è impostata).

- Rotazione per dimensione: 2 MB per file, massimo 5 file (`musify.log`,
  `musify.1.log` … `musify.4.log`). Il file corrente viene riaperto in append a
  ogni avvio.
- Le scritture sono protette da un `Mutex`; la rotazione chiude il file, sposta
  gli archivi e riapre il file base.
- `main.rs` configura `tracing` con stdout + file, senza ANSI nel file.
- I target HTTP rumorosi (`reqwest`, `hyper`, `h2`, `rustls`) sono limitati a
  `warn` anche con `RUST_LOG`: gli URL delle richieste Jamendo contengono il
  client id come query parameter e non devono finire nei log.
- Il messaggio di fonte disabilitata non nomina più la variabile d'ambiente:
  dice solo `client id not configured`.
- Test automatici: rotazione, limite file, append tra riaperture.

Riga di esempio da un avvio senza chiave Jamendo:

```text
INFO musify: Musify avviato version="0.1.0" data_dir=... log_dir=...\logs
INFO musify::backend: Jamendo source disabled: client id not configured
INFO musify::backend: backend pronto elapsed_ms=66
INFO musify: Musify pronto version="0.1.0" data_dir=...
```

Nessun valore del client id, token o chiave è stato scritto nei log di prova.

## 6. Controlli

- `cargo fmt --all -- --check`: pulito.
- `cargo clippy --workspace --all-targets -- -D warnings`: pulito.
- `cargo test --workspace`: **116 passati, 0 falliti, 2 ignorati** (i due test
  podcast di rete reale).
- `cargo build -p musify --release`: riuscita, nessun warning.
- `scripts/check-secrets.ps1`: nessun segreto rilevato.
- Build script e test del logger inclusi nella suite; nessun `unwrap`/`expect`
  nel codice di produzione nuovo.

I log grezzi delle build clean sono in `target/m8-build/logs` durante la
sessione di misura; i risultati consolidati sono in
[M8-BUILDS.md](M8-BUILDS.md).

## 7. Checklist manuale

Aggiornata in [docs/MANUAL-CHECKLIST.md](MANUAL-CHECKLIST.md), sezione 12:
build fast/release, versione e icona, installazione con e senza `-WhatIf`,
avvio da collegamento, rotazione log, assenza di segreti, aggiornamento,
pre-push tag-only e `MUSIFY_CARGO_JOBS`.

## 8. Limiti e note

- `fast` è per l'uso quotidiano, non per distribuire: l'exe è più grande del
  `release`, mentre avvio e CPU sono equivalenti entro la varianza. Il `release`
  resta la build di riferimento per distribuzione e LTO.
- I log sono sincroni (niente thread dedicato): il volume è basso e non c'è
  polling. La rotazione è per dimensione, non per data.
- L'installazione è per-utente: non crea servizi, voci di disinstallazione o
  privileged tasks. Non tocca la cartella dati.
- La prova click-through di installazione e avvio da collegamento è manuale; i
  test automatici coprono build script, logging e hook tag-only.
- L'icona è derivata dal logo esistente; non è stato introdotto un nuovo logo.
- Il profilo `release` non è stato modificato.
- La build non è bit-identica per 24 byte di metadati MSVC (timestamp PE,
  timestamp risorse, GUID di debug). Il contenuto è identico. Se in futuro
  serve una bit-reproducibility stretta, va usato un resource compiler
  deterministico o una post-elaborazione del PE; non è stata aggiunta per non
  cambiare il link della release.
