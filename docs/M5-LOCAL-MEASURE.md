# M5 - Riproduzione locale

Data: 2026-11-10 00:26:04
Commit: `9688eae`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Radio: non attiva
Metodo: 10 avvii, segnale `RUNE_READY_DIR`, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di `on_ready`; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | 1.091,00 ms (min 1.074,00, max 1.416,00) |
| Startup | tempo processo → ready | 1.126,35 ms (min 1.108,35, max 1.468,98) |
| Riposo | working set | 185,28 MB (min 181,97, max 185,35) |
| Riposo | private bytes | 170,55 MB (min 168,22, max 171,98) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,94) |
| Riproduzione | working set | 187,41 MB (min 187,24, max 187,81) |
| Riproduzione | private bytes | 171,50 MB (min 169,82, max 172,53) |
| Riproduzione | CPU un core | 7,64 % (min 6,23, max 9,35) |
| Riproduzione | CPU normalizzata | 0,64 % (min 0,52, max 0,78) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1416 | 1469 | 181.97 | 168.66 | 0.623 |
| 2 | 1209 | 1233.2 | 185.3 | 168.8 | 0.312 |
| 3 | 1093 | 1111.9 | 185.34 | 171.77 | 0 |
| 4 | 1089 | 1122.5 | 185.21 | 171.89 | 0.311 |
| 5 | 1112 | 1149.1 | 185.32 | 169.97 | 0.312 |
| 6 | 1107 | 1139.7 | 185.21 | 168.22 | 0 |
| 7 | 1088 | 1119.5 | 185.28 | 168.95 | 0.624 |
| 8 | 1085 | 1130.2 | 185.28 | 171.52 | 0.312 |
| 9 | 1087 | 1109.3 | 185.35 | 171.98 | 0.937 |
| 10 | 1074 | 1108.4 | 185.2 | 171.12 | 0.623 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1099 | 1128.2 | 187.42 | 171.11 | 8.726 | 0.727 |
| 2 | 1080 | 1112.1 | 187.31 | 172.05 | 7.49 | 0.624 |
| 3 | 1116 | 1145.4 | 187.26 | 171.62 | 8.427 | 0.702 |
| 4 | 1103 | 1130.6 | 187.56 | 171.03 | 6.234 | 0.519 |
| 5 | 1077 | 1097.7 | 187.4 | 169.82 | 7.791 | 0.649 |
| 6 | 1106 | 1126.5 | 187.48 | 172.53 | 8.747 | 0.729 |
| 7 | 1145 | 1180.1 | 187.26 | 169.88 | 6.543 | 0.545 |
| 8 | 1098 | 1138.2 | 187.8 | 171.62 | 7.485 | 0.624 |
| 9 | 1079 | 1109.5 | 187.81 | 172.24 | 9.349 | 0.779 |
| 10 | 1074 | 1093.2 | 187.24 | 171.38 | 7.495 | 0.625 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
