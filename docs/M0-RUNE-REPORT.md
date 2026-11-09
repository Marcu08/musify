# M0 — Workspace e smoke test Rune

Data: 2026-11-09
Macchina: Windows 11 Home 10.0.26300, AMD Ryzen 5 5625U (6 core / 12 thread),
Radeon integrata, 7.9 GB RAM.
Toolchain: Rust 1.99.1 stable x86_64-pc-windows-msvc.
Rune: `757bcb1d1da1b3f80484f36ef610b83ddbcc6003` (branch `dev`, pinnato in
`crates/ui/Cargo.toml`).

## Esito

**Rune passa lo smoke test.** La UI definitiva può usare Rune. Il Piano B resta
disponibile dietro lo stesso confine del crate `ui` (nessun tipo Rune nel core).

## Cosa è stato verificato

| Test | Esito | Evidenza |
| --- | --- | --- |
| `examples/hello` headless | passa | `docs/evidence/rune-hello.png` |
| `examples/settings` headless | passa | `docs/evidence/rune-settings.png` |
| `examples/todo` headless | passa | `docs/evidence/rune-todo.png` |
| Prototipo 100.000 righe | passa | `docs/evidence/rune-smoke-typed.png` |
| Scroll fino alla riga 100.000 | passa | `docs/evidence/rune-smoke-bottom.png` |
| Campo di testo + pulsante | passa | click registrato, testo digitato |
| Finestra reale wgpu/DX12 | passa | titolo `Musify smoke test`, input `ciao` ricevuto |
| Crash o rendering rotto | nessuno | 3 esempi + prototipo eseguiti più volte |

Comandi riproducibili:

```powershell
cargo run -p musify-ui --example rune_smoke -- --snap smoke.png
cargo run -p musify-ui --example rune_smoke -- --script script.json --snap smoke.png --at mark:end
```

Il prototipo contiene 100.000 righe virtuali, un campo di testo, due pulsanti e
usa `virtual_rows` + `virtual_rows_version`. Solo le righe visibili vengono
costruite: il tree headless riporta **97 nodi di layout** e **444 istanze**,
non 100.000. Con lo script headless sono stati digitati `città 日本語`, è stato
premuto il pulsante e lo scroll è arrivato alla riga 100.000.

## Input e IME

- La digitazione da tastiera nella finestra reale funziona (`QUERY=c`, `ci`,
  `cia`, `ciao`).
- Il testo Unicode arriva al modello (`città 日本語` nello script headless).
- Rune usa il percorso IME di winit; la composizione IME reale (candidate window,
  lingue CJK) non è stata verificata a fondo in M0. Non è emersa instabilità.
- Con testo CJK Rune stampa `ICU4X data error: No segmentation model for complex
  script: Chinese/Japanese`; il testo viene comunque renderizzato correttamente.
  È un limite di segmentazione, non un crash. Da tenere presente per la ricerca.
- Il test degli appunti con `Ctrl+V` non è stato concluso: Windows non ha
  riportato in primo piano la finestra Rune in modo affidabile durante
  l'automazione. Lo riproverò in M3 con l'app completa.

## Misure di avvio (headless, build release)

| Misura | Valore |
| --- | --- |
| GPU init (cache calda) | 451–500 ms |
| Primo frame CPU | 143–174 ms |
| Tempo totale processo → PNG | 894–963 ms |
| GPU init (prima esecuzione) | ~1.4 s |

L'obiettivo indicativo di **< 300 ms alla prima finestra non è raggiunto** in
questo test headless. Il numero include inizializzazione wgpu e readback PNG;
la misura reale della finestra dell'app verrà fatta in M3. Non è un motivo per
scartare Rune: il framework compila, renderizza e riceve input, e il divario è
misurabile e ottimizzabile (prewarm dove disponibile, prima finestra leggera,
lavoro pesante in `on_ready`). Se in M3 l'avvio reale restasse sopra il secondo,
rivaluterò il Piano B con i dati alla mano.

## Decisione

Si procede con Rune, tenendo il crate `ui` come unico punto di contatto con il
framework. Se Rune dovesse rompersi su Windows, `egui/eframe` o `slint` potranno
sostituirlo senza modificare `core`, `audio`, `library` o il contratto
comandi/eventi.
