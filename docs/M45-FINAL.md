# M4.5 — Finale prestazioni

Data: 2026-11-09 23:04:30
Commit: `bd07b28`
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
| Startup | ready Rune | 1.107,50 ms (min 1.068,00, max 1.593,00) |
| Startup | tempo processo → ready | 1.132,24 ms (min 1.089,90, max 1.652,99) |
| Riposo | working set | 184,97 MB (min 181,70, max 185,11) |
| Riposo | private bytes | 170,43 MB (min 164,94, max 171,65) |
| Riposo | CPU un core | 0,16 % (min 0,00, max 1,25) |
| Riproduzione | working set | 187,02 MB (min 186,87, max 187,31) |
| Riproduzione | private bytes | 170,05 MB (min 168,77, max 172,15) |
| Riproduzione | CPU un core | 7,17 % (min 5,92, max 9,35) |
| Riproduzione | CPU normalizzata | 0,60 % (min 0,49, max 0,78) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1593 | 1653 | 181.7 | 164.94 | 0 |
| 2 | 1127 | 1149.7 | 184.97 | 170 | 0.624 |
| 3 | 1110 | 1129.1 | 184.94 | 167.74 | 1.249 |
| 4 | 1068 | 1089.9 | 185.01 | 170.07 | 0 |
| 5 | 1117 | 1135.4 | 185.11 | 168.46 | 0 |
| 6 | 1170 | 1186.3 | 184.93 | 170.79 | 0 |
| 7 | 1105 | 1124.9 | 184.95 | 171.2 | 0.312 |
| 8 | 1101 | 1146.1 | 184.96 | 171.47 | 0.312 |
| 9 | 1088 | 1110 | 185.07 | 171.65 | 0 |
| 10 | 1097 | 1114.6 | 184.99 | 170.9 | 0.625 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1073 | 1092.8 | 186.87 | 169.29 | 6.866 | 0.572 |
| 2 | 1080 | 1111.4 | 187.29 | 169.46 | 6.562 | 0.547 |
| 3 | 1081 | 1100 | 187.2 | 172.15 | 5.923 | 0.494 |
| 4 | 1123 | 1153.7 | 187.02 | 170.63 | 9.354 | 0.78 |
| 5 | 1112 | 1138.1 | 186.88 | 169.23 | 5.937 | 0.495 |
| 6 | 1100 | 1140.9 | 187.31 | 170.85 | 7.808 | 0.651 |
| 7 | 1070 | 1103.8 | 187.02 | 168.84 | 6.862 | 0.572 |
| 8 | 1078 | 1093.8 | 186.94 | 171.1 | 8.725 | 0.727 |
| 9 | 1164 | 1197.1 | 186.96 | 168.77 | 7.48 | 0.623 |
| 10 | 1083 | 1099.1 | 187.14 | 170.93 | 7.482 | 0.624 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
