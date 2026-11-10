# M5.2 - Finale radio

Data: 2026-11-11 00:04:13
Commit: `6c16263`
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
| Riproduzione | working set | 194,68 MB (min 187,79, max 194,91) |
| Riproduzione | private bytes | 175,24 MB (min 171,20, max 176,25) |
| Riproduzione | CPU un core | 8,43 % (min 6,23, max 10,93) |
| Riproduzione | CPU normalizzata | 0,70 % (min 0,52, max 0,91) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1626 | 1667 | 194.69 | 173.68 | 10.613 | 0.884 |
| 2 | 1092 | 1144 | 194.68 | 175.02 | 9.046 | 0.754 |
| 3 | 1085 | 1119.5 | 194.65 | 176.25 | 8.123 | 0.677 |
| 4 | 1234 | 1254.8 | 194.82 | 175.8 | 9.355 | 0.78 |
| 5 | 1123 | 1138.9 | 194.9 | 175.45 | 6.855 | 0.571 |
| 6 | 1077 | 1093.5 | 194.48 | 173.24 | 8.727 | 0.727 |
| 7 | 1109 | 1143.6 | 187.79 | 171.2 | 7.793 | 0.649 |
| 8 | 1111 | 1142.6 | 194.55 | 175.73 | 6.559 | 0.547 |
| 9 | 1128 | 1144.8 | 194.91 | 175.96 | 10.932 | 0.911 |
| 10 | 1063 | 1090.7 | 194.67 | 173.62 | 6.234 | 0.519 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
