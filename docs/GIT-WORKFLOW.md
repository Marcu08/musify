# Workflow Git — Musify

Questo documento descrive il flusso di lavoro del repository. Le regole di
sicurezza valgono sempre, anche quando si usa `git add -p` o un client grafico.

## 1. Unità di commit

- Un commit per unità logica: una correzione, una feature o una migrazione.
- Non mescolare refactor non richiesti con una feature.
- Un commit intermedio deve compilare. Prima di committare eseguire almeno
  `cargo check --workspace`; prima del push eseguire la suite completa.
- A fine milestone: push su `main` solo con fmt, clippy, test e release verdi.
- Se due milestone condividono file e non è possibile separarle senza rompere la
  build, usare un unico commit con titolo che le elenca e corpo che spiega le
  due parti.

## 2. Formato dei messaggi

```text
M7.2: fonte Jamendo e streaming OnDemand
docs: report e misure M7.1/M7.2
chore: hook Git e scan segreti
```

- Titolo in italiano, massimo 72 caratteri, senza punto finale.
- Prefisso coerente: `Mxx:`, `docs:`, `chore:`, `fix:`, `test:`.
- Corpo opzionale: cosa cambia e perché, non solo l'elenco dei file.
- Non inserire mai valori di chiavi, token o client id nei messaggi.

## 3. Flusso operativo

```powershell
git status -sb
git diff --stat
git add -p                 # solo se i hunk sono separabili con sicurezza
cargo check --workspace
git commit -m "M7.2: ..."
```

Prima del push:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -j 1 -- -D warnings
cargo test --workspace -j 1
cargo build -p musify --release
powershell -NoProfile -File .\scripts\check-secrets.ps1
git push origin main
```

Dopo il push, se la milestone è completa:

```powershell
git tag -a m7.2 -m "M7.2 - Jamendo"
git push origin m7.2
```

I tag sono annotati e vanno creati solo sul commit verificato della milestone.

## 4. Hook versionati

Attivazione una volta per clone:

```powershell
git config core.hooksPath .githooks
```

- `.githooks/pre-commit`: scan segreti sui file staged e `cargo fmt --check`.
- `.githooks/pre-push`: scan segreti sull'intero albero, `cargo clippy -D warnings`
  e `cargo test`.
- Su Windows gli hook girano con Git Bash e richiamano PowerShell per lo scan.
- Se PowerShell non è disponibile, il pre-commit fallisce invece di saltare il
  controllo.
- Per bypassare un hook solo in un caso eccezionale usare `--no-verify`, mai per
  ignorare un segreto o un test rosso.

## 5. Regole di sicurezza

Non committare mai:

- il valore di `MUSIFY_JAMENDO_CLIENT_ID` o di altre variabili d'ambiente;
- file `.env`, `.env.*`, token, chiavi, certificati (`*.pem`, `*.key`, `*.p12`);
- database (`*.sqlite3`, `*.db`, WAL/SHM), `testdata/`, `target/`;
- cache di copertine (`jamendo-covers/`, `podcast-covers/`, `radio-icons/`);
- binari generati (`*.exe`, `*.dll`, `*.pdb`, `*.rlib`, `*.rmeta`);
- file maggiori di 5 MB non necessari al repository.

Prima di ogni commit:

```powershell
powershell -NoProfile -File .\scripts\check-secrets.ps1
```

Lo script stampa solo nome file e riga, mai il valore trovato. Se segnala
qualcosa, fermarsi, rimuovere il dato dal file e non committare.

## 6. CI e tag

- La CI GitHub Actions deve essere verde dopo il push.
- Se `gh` è disponibile: `gh run list --branch main --limit 5`.
- I tag `m7.1`, `m7.2`, ... vengono creati dopo il push e dopo la verifica.
- Non usare `push --force`, rebase o amend su commit già pushati.
