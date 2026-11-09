# M5 - Radio internet attiva

Data: 2026-11-10 00:43:08
Commit: `ca9c660`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Radio: https://mangoradio.stream.laut.fm/mangoradio
Metodo: 10 avvii, segnale `RUNE_READY_DIR`, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di `on_ready`; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | n/d |
| Startup | tempo processo → ready | n/d |
| Riposo | working set | n/d |
| Riposo | private bytes | n/d |
| Riposo | CPU un core | n/d |
| Riproduzione | working set | 194,81 MB (min 194,59, max 195,14) |
| Riproduzione | private bytes | 174,68 MB (min 172,93, max 175,56) |
| Riproduzione | CPU un core | 8,57 % (min 6,87, max 10,60) |
| Riproduzione | CPU normalizzata | 0,71 % (min 0,57, max 0,88) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2088 | 2791.9 | 194.82 | 175.56 | 8.422 | 0.702 |
| 2 | 1112 | 1152 | 194.98 | 172.96 | 9.372 | 0.781 |
| 3 | 1091 | 1123.2 | 194.91 | 174.48 | 9.349 | 0.779 |
| 4 | 1064 | 1096.2 | 194.59 | 175.47 | 9.351 | 0.779 |
| 5 | 1146 | 1170.5 | 194.94 | 175.09 | 8.726 | 0.727 |
| 6 | 1127 | 1153.2 | 194.6 | 174.47 | 10.602 | 0.884 |
| 7 | 1078 | 1106.7 | 195.14 | 175.24 | 6.872 | 0.573 |
| 8 | 1069 | 1102.4 | 194.79 | 174.88 | 7.805 | 0.65 |
| 9 | 1209 | 1227.8 | 194.79 | 174.14 | 6.867 | 0.572 |
| 10 | 1120 | 1151.3 | 194.6 | 172.93 | 7.799 | 0.65 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
