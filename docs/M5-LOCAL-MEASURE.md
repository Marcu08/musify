# M5 - Riproduzione locale

Data: 2026-11-10 00:46:09
Commit: `ca9c660`
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
| Startup | ready Rune | 1.102,00 ms (min 1.084,00, max 1.358,00) |
| Startup | tempo processo → ready | 1.136,86 ms (min 1.104,11, max 1.419,99) |
| Riposo | working set | 185,28 MB (min 181,91, max 185,45) |
| Riposo | private bytes | 169,57 MB (min 166,86, max 172,12) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,62) |
| Riproduzione | working set | 187,46 MB (min 187,29, max 187,63) |
| Riproduzione | private bytes | 171,53 MB (min 169,39, max 173,30) |
| Riproduzione | CPU un core | 7,49 % (min 5,92, max 11,86) |
| Riproduzione | CPU normalizzata | 0,62 % (min 0,49, max 0,99) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1358 | 1420 | 181.91 | 166.86 | 0.624 |
| 2 | 1170 | 1195.2 | 185.24 | 169.15 | 0 |
| 3 | 1110 | 1142.9 | 185.29 | 171.22 | 0.312 |
| 4 | 1101 | 1130.8 | 185.45 | 171.68 | 0 |
| 5 | 1084 | 1104.1 | 185.3 | 169.1 | 0.623 |
| 6 | 1125 | 1156.8 | 185.1 | 169.77 | 0.312 |
| 7 | 1089 | 1105.7 | 185.28 | 168.94 | 0.623 |
| 8 | 1100 | 1148.8 | 185.28 | 171.45 | 0.312 |
| 9 | 1103 | 1123.3 | 185.38 | 172.12 | 0 |
| 10 | 1085 | 1120.9 | 185.28 | 169.38 | 0.312 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1069 | 1090.6 | 187.44 | 171.33 | 9.374 | 0.781 |
| 2 | 1155 | 1183.3 | 187.39 | 171.89 | 7.173 | 0.598 |
| 3 | 1094 | 1126 | 187.42 | 171.38 | 8.426 | 0.702 |
| 4 | 1096 | 1117.7 | 187.48 | 171.68 | 6.55 | 0.546 |
| 5 | 1107 | 1140.4 | 187.63 | 172.09 | 7.184 | 0.599 |
| 6 | 1122 | 1140.7 | 187.39 | 171.21 | 6.857 | 0.571 |
| 7 | 1109 | 1140.7 | 187.57 | 169.39 | 5.924 | 0.494 |
| 8 | 1133 | 1164.3 | 187.57 | 173.3 | 8.106 | 0.676 |
| 9 | 1087 | 1105.6 | 187.29 | 172.27 | 7.803 | 0.65 |
| 10 | 1116 | 1138.4 | 187.52 | 171 | 11.86 | 0.988 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
