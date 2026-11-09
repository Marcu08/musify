# M5 - Radio internet attiva

Data: 2026-11-10 00:09:18
Commit: `fcf1eb8`
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
| Riproduzione | working set | 194,74 MB (min 194,51, max 194,96) |
| Riproduzione | private bytes | 174,05 MB (min 173,19, max 175,92) |
| Riproduzione | CPU un core | 6,87 % (min 5,93, max 9,69) |
| Riproduzione | CPU normalizzata | 0,57 % (min 0,49, max 0,81) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2331 | 2983.8 | 194.58 | 175.92 | 7.487 | 0.624 |
| 2 | 1113 | 1131.7 | 194.66 | 175.75 | 7.166 | 0.597 |
| 3 | 1087 | 1116.3 | 194.8 | 173.89 | 5.931 | 0.494 |
| 4 | 1118 | 1137.9 | 194.68 | 175.34 | 6.544 | 0.545 |
| 5 | 1181 | 1201.1 | 194.84 | 174.39 | 6.873 | 0.573 |
| 6 | 1109 | 1125.3 | 194.51 | 174.1 | 9.685 | 0.807 |
| 7 | 1092 | 1124.3 | 194.79 | 173.46 | 5.935 | 0.495 |
| 8 | 1080 | 1114.7 | 194.96 | 174 | 6.548 | 0.546 |
| 9 | 1152 | 1172.5 | 194.96 | 173.19 | 8.119 | 0.677 |
| 10 | 1085 | 1122.1 | 194.54 | 173.95 | 6.858 | 0.572 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
