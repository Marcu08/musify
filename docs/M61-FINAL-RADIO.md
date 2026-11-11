# M6.1 - Finale radio

Data: 2026-11-11 03:17:56
Commit: `b124a11`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Radio: https://mangoradio.stream.laut.fm/mangoradio
Podcast: non attivo
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
| Riproduzione | working set | 195,04 MB (min 194,80, max 195,55) |
| Riproduzione | private bytes | 173,92 MB (min 172,90, max 176,69) |
| Riproduzione | CPU un core | 7,65 % (min 5,62, max 9,05) |
| Riproduzione | CPU normalizzata | 0,64 % (min 0,47, max 0,75) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1255 | 1296.3 | 195.55 | 173.75 | 7.496 | 0.625 |
| 2 | 1094 | 1118.1 | 195.3 | 173.27 | 7.802 | 0.65 |
| 3 | 1083 | 1117.8 | 195.22 | 174.62 | 8.109 | 0.676 |
| 4 | 1061 | 1089.4 | 194.88 | 176.08 | 7.186 | 0.599 |
| 5 | 1081 | 1102.4 | 195.15 | 176.69 | 7.809 | 0.651 |
| 6 | 1075 | 1105.1 | 194.89 | 172.9 | 7.173 | 0.598 |
| 7 | 1082 | 1098.8 | 194.85 | 173.17 | 9.053 | 0.754 |
| 8 | 1131 | 1174.9 | 194.8 | 174.04 | 5.924 | 0.494 |
| 9 | 1102 | 1118.4 | 195.21 | 175.12 | 5.616 | 0.468 |
| 10 | 1075 | 1104.3 | 194.93 | 173.8 | 7.806 | 0.651 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
