# M5 - Radio internet attiva

Data: 2026-11-10 00:23:03
Commit: `9688eae`
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
| Riproduzione | working set | 194,85 MB (min 194,52, max 195,38) |
| Riproduzione | private bytes | 174,30 MB (min 172,95, max 175,68) |
| Riproduzione | CPU un core | 8,42 % (min 6,24, max 9,99) |
| Riproduzione | CPU normalizzata | 0,70 % (min 0,52, max 0,83) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1959 | 2626.5 | 194.7 | 175.49 | 7.792 | 0.649 |
| 2 | 1085 | 1105.2 | 194.84 | 175.27 | 8.418 | 0.701 |
| 3 | 1097 | 1116.8 | 195.38 | 174.18 | 7.479 | 0.623 |
| 4 | 1082 | 1118.8 | 194.81 | 172.95 | 8.435 | 0.703 |
| 5 | 1097 | 1134.6 | 195.01 | 173.66 | 6.239 | 0.52 |
| 6 | 1059 | 1081.1 | 194.61 | 174.41 | 6.544 | 0.545 |
| 7 | 1233 | 1253.1 | 194.86 | 174.51 | 9.042 | 0.753 |
| 8 | 1122 | 1153.5 | 194.88 | 175.68 | 8.427 | 0.702 |
| 9 | 1114 | 1145.5 | 194.52 | 173.08 | 8.734 | 0.728 |
| 10 | 1103 | 1133.6 | 194.98 | 173.68 | 9.988 | 0.832 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
