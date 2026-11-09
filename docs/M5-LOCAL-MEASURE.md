# M5 - Riproduzione locale

Data: 2026-11-10 00:12:57
Commit: `fcf1eb8`
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
| Startup | ready Rune | 1.094,00 ms (min 1.068,00, max 1.616,00) |
| Startup | tempo processo → ready | 1.125,85 ms (min 1.084,85, max 1.672,53) |
| Riposo | working set | 185,31 MB (min 181,82, max 185,55) |
| Riposo | private bytes | 169,55 MB (min 167,95, max 172,23) |
| Riposo | CPU un core | 0,00 % (min 0,00, max 1,25) |
| Riproduzione | working set | 187,47 MB (min 187,25, max 187,61) |
| Riproduzione | private bytes | 171,12 MB (min 169,33, max 173,30) |
| Riproduzione | CPU un core | 7,80 % (min 6,23, max 8,73) |
| Riproduzione | CPU normalizzata | 0,65 % (min 0,52, max 0,73) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1616 | 1672.5 | 181.82 | 167.95 | 0 |
| 2 | 1128 | 1161.8 | 185.5 | 172.23 | 0 |
| 3 | 1068 | 1084.9 | 185.27 | 171.06 | 0 |
| 4 | 1119 | 1150 | 185.34 | 170.57 | 0 |
| 5 | 1093 | 1121 | 185.5 | 171.78 | 0 |
| 6 | 1090 | 1120.6 | 185.27 | 169.29 | 1.248 |
| 7 | 1095 | 1130.7 | 185.14 | 169.6 | 0 |
| 8 | 1172 | 1189.7 | 185.25 | 169.5 | 0 |
| 9 | 1077 | 1097.7 | 185.55 | 168.54 | 0 |
| 10 | 1085 | 1107 | 185.37 | 168.56 | 0 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1092 | 1113.5 | 187.25 | 170.11 | 8.125 | 0.677 |
| 2 | 1112 | 1156 | 187.61 | 171.45 | 6.868 | 0.572 |
| 3 | 1095 | 1119.5 | 187.48 | 172.5 | 7.805 | 0.65 |
| 4 | 1055 | 1087.3 | 187.38 | 170.8 | 8.726 | 0.727 |
| 5 | 1075 | 1103.1 | 187.33 | 169.88 | 7.802 | 0.65 |
| 6 | 1201 | 1232.5 | 187.47 | 169.33 | 6.232 | 0.519 |
| 7 | 1103 | 1119.2 | 187.48 | 172.95 | 8.415 | 0.701 |
| 8 | 1072 | 1090.2 | 187.33 | 170.75 | 8.42 | 0.702 |
| 9 | 1096 | 1126.1 | 187.55 | 173.3 | 7.175 | 0.598 |
| 10 | 1078 | 1107.6 | 187.53 | 173.14 | 7.185 | 0.599 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
