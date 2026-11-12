# Report Git — M7.1/M7.2 e automazioni

Data: 2026-11-12
Repository: `C:\Users\marzu\OneDrive\Desktop\musify`
Branch: `main`
Stato iniziale: ultimo commit `aca8763` (M6.1), lavoro M7.1/M7.2 non committato.

## 1. Commit creati

| Hash | Titolo | Contenuto |
| --- | --- | --- |
| `e1d7f03` | `chore: regole Git per fine riga e file ignorati` | `.gitattributes`, `.gitignore` esteso |
| `337d89d` | `chore: hook Git, scan segreti e workflow` | `.githooks/pre-commit`, `.githooks/pre-push`, `scripts/check-secrets.ps1`, `docs/GIT-WORKFLOW.md` |
| `5c92587` | `M7.1 e M7.2: playlist generiche e fonte Jamendo` | codice M7.1 e M7.2 (32 file) |
| `027adfd` | `docs: report e misure M7.1/M7.2` | report, misure, screenshot, README, ARCHITECTURE, checklist |
| `ff35e22` | `chore: scan segreti sull'albero in pre-push` | modalità `-All` e hook pre-push aggiornato |

M7.1 e M7.2 sono in un unico commit di codice perché condividono gli stessi
hunk in `core/domain`, `app/backend`, `ui/state`, `i18n`, `View`, routing,
`Cargo.toml` e `Cargo.lock`. Separarli avrebbe richiesto uno staging parziale
dei hunk con rischio concreto di commit intermedio non compilante. Il corpo del
commit elenca comunque le due parti.

## 2. Controlli eseguiti

| Controllo | Esito |
| --- | --- |
| `cargo check --workspace -j 1` prima del commit codice | riuscito |
| `cargo fmt --all -- --check` | pulito |
| `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | pulito |
| `cargo test --workspace -j 1` | **87 passati, 0 falliti, 2 ignorati** |
| `cargo build -p musify --release` | riuscita |
| `scripts/check-secrets.ps1` sui commit staged | pulito |
| `scripts/check-secrets.ps1 -All` sull'albero | pulito |

La verifica completa in un unico comando ha superato il timeout della shell
durante la build release (clippy e test erano già passati). La build release è
stata ripresa e completata con successo in 11m48s. Non è un problema del
repository.

## 3. Scan segreti

`scripts/check-secrets.ps1`:

- scansiona i file staged per default e l'intero albero con `-All`;
- cerca il valore di `MUSIFY_JAMENDO_CLIENT_ID` se definito nell'ambiente;
- segnala `.env`, database, certificati e file sensibili;
- cerca assegnazioni tipo `api_key=`, `token=`, `secret=`, `password=`;
- cerca stringhe lunghe esadecimali/base64;
- stampa solo `file:riga`, mai il valore trovato;
- esce con codice diverso da 0 se trova qualcosa.

Durante la prima scansione `-All` è stato segnalato il rev Git di Rune in un URL
(`crates/ui/Cargo.toml`). Era un falso positivo: l'euristica delle stringhe
lunghe ora esclude URL, specifiche `rev =` e i file `Cargo.toml`/`Cargo.lock`,
mentre le assegnazioni di segreti restano controllate.

La chiave Jamendo non è mai stata stampata, salvata in un file, inclusa nei
commit message, nei log o negli screenshot. Il controllo di sicurezza ha
confermato che il valore presente nell'ambiente non compare nei file di testo
del repository.

## 4. Automazioni aggiunte

- `.gitattributes`: LF per sorgenti, TOML, Markdown, JSON, YAML, CSV e script
  shell; CRLF per PowerShell/Batch; asset binari e database come `binary`.
- `.gitignore`: aggiunti `.env*`, SQLite/WAL/SHM, database, cache copertine,
  binari generati, file temporanei e log.
- `.githooks/pre-commit`: scan segreti sui file staged e `cargo fmt --check`.
- `.githooks/pre-push`: scan segreti sull'intero albero, clippy `-D warnings` e
  test.
- `scripts/check-secrets.ps1`: script di scansione con modalità staged e `-All`.
- `docs/GIT-WORKFLOW.md`: unità di commit, formato messaggi, flusso, tag,
  sicurezza e cosa non committare.
- `README.md`: sezione per attivare gli hook con
  `git config core.hooksPath .githooks`.

Gli hook sono eseguibili (`100755`) e funzionano su Windows tramite Git Bash,
richiamando PowerShell per lo scan. Se PowerShell non è disponibile, il
pre-commit fallisce invece di saltare il controllo.

## 5. Normalizzazione fine riga

La normalizzazione completa è stata eseguita con `git add --renormalize .` su
albero pulito dopo i commit di codice e documentazione. Non ha prodotto modifiche:
l'indice era già normalizzato secondo `.gitattributes`. Non è stato quindi
necessario un commit separato di normalizzazione; il commit `e1d7f03` contiene
le regole e i file già versionati sono stati aggiunti con LF.

## 6. Push, CI e tag

- Push principale: `git push origin main` senza force.
- Esito: `aca8763..ff35e22  main -> main`.
- CI GitHub Actions: run `43228640737`, conclusione `completed success`, durata
  6m13s.
- Tag annotati:
  - `m7.1` — oggetto `48eda87`, puntato a `ff35e22`;
  - `m7.2` — oggetto `5abef40`, puntato a `ff35e22`.
- I due tag puntano allo stesso commit perché M7.1 e M7.2 sono stati committati
  insieme; `ff35e22` è lo stato verificato che contiene codice, report e hook.
- Il push dei soli tag è stato fatto con `--no-verify`: non cambiava codice e la
  suite era già stata eseguita dal hook pre-push e dalla CI.

## 7. Stato finale e note residue

- `git status -sb` deve risultare pulito dopo il commit di questo report.
- Non sono stati usati `reset --hard`, `clean`, `checkout/restore` distruttivi,
  rebase, amend di commit pushati o push force.
- M7.1 e M7.2 non hanno tag separati per codice e documentazione: i tag puntano
  allo stato combinato verificato. Per le prossime milestone conviene tenere il
  codice separato quando i file non sono condivisi.
- Gli hook richiedono PowerShell su Windows; è documentato.
- La build release è lenta (circa 12 minuti) e la verifica completa può superare
  il timeout di una singola shell; i singoli passi restano riproducibili.
- Il valore del client id Jamendo non è mai stato mostrato; lo scan lo cerca
  esplicitamente se la variabile è definita.
