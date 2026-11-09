# M4.5 — Baseline prestazioni

Data: 2026-11-09 21:56:09
Commit: `98fe7ed`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Metodo: 10 avvii, segnale `RUNE_READY_DIR`, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di `on_ready`; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | 1.124,00 ms (min 1.081,00, max 1.526,00) |
| Startup | tempo processo → ready | 1.147,06 ms (min 1.107,30, max 1.583,32) |
| Riposo | working set | 186,26 MB (min 182,88, max 186,47) |
| Riposo | private bytes | 170,31 MB (min 167,40, max 171,93) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,31) |
| Riproduzione | working set | 188,33 MB (min 187,99, max 189,32) |
| Riproduzione | private bytes | 170,84 MB (min 169,28, max 172,86) |
| Riproduzione | CPU un core | 8,42 % (min 6,86, max 9,68) |
| Riproduzione | CPU normalizzata | 0,70 % (min 0,57, max 0,81) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1526 | 1583.3 | 182.88 | 167.4 | 0 |
| 2 | 1097 | 1116.4 | 186.41 | 169.96 | 0.312 |
| 3 | 1104 | 1133.3 | 186.19 | 171.46 | 0.311 |
| 4 | 1115 | 1137.5 | 186.26 | 170.34 | 0 |
| 5 | 1208 | 1230.4 | 186.27 | 169.12 | 0.312 |
| 6 | 1133 | 1163.4 | 186.11 | 170.37 | 0 |
| 7 | 1091 | 1108.8 | 186.11 | 170.84 | 0.312 |
| 8 | 1138 | 1156.6 | 186.43 | 171.93 | 0.312 |
| 9 | 1141 | 1164.3 | 186.32 | 169.85 | 0.312 |
| 10 | 1081 | 1107.3 | 186.47 | 170.29 | 0 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1088 | 1120.1 | 187.99 | 171.95 | 9.675 | 0.806 |
| 2 | 1475 | 1495.4 | 188.24 | 170.38 | 8.725 | 0.727 |
| 3 | 1115 | 1137.7 | 188.12 | 169.28 | 7.791 | 0.649 |
| 4 | 1110 | 1131.9 | 188.36 | 170.52 | 7.493 | 0.624 |
| 5 | 1089 | 1120 | 188.44 | 170.29 | 8.424 | 0.702 |
| 6 | 1076 | 1098.8 | 188.11 | 171.3 | 8.749 | 0.729 |
| 7 | 1093 | 1110.5 | 188.52 | 171.06 | 8.412 | 0.701 |
| 8 | 1103 | 1132.6 | 189.32 | 170.62 | 8.434 | 0.703 |
| 9 | 1081 | 1098.7 | 188.32 | 172.86 | 7.172 | 0.598 |
| 10 | 1097 | 1114.1 | 188.34 | 172.4 | 6.859 | 0.572 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
