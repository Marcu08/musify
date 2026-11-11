# M6.1 - Finale podcast

Data: 2026-11-11 03:38:28
Commit: `b124a11`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Radio: non attiva
Podcast: https://nyt.simplecastaudio.com/03d8b493-87fc-4bd1-931f-8a8e9b945d8a/episodes/60e9e1cf-51c5-41b1-af3d-736ef4484e18/audio/128/default.mp3?aid=rss_feed&awCollectionId=03d8b493-87fc-4bd1-931f-8a8e9b945d8a&awEpisodeId=60e9e1cf-51c5-41b1-af3d-736ef4484e18&feed=54nAGcIl
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
| Riproduzione | working set | 194,14 MB (min 193,00, max 194,89) |
| Riproduzione | private bytes | 173,68 MB (min 172,35, max 174,97) |
| Riproduzione | CPU un core | 8,58 % (min 7,50, max 9,99) |
| Riproduzione | CPU normalizzata | 0,71 % (min 0,62, max 0,83) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2077 | 2717.2 | 194.19 | 174.47 | 8.425 | 0.702 |
| 2 | 1090 | 1112.3 | 194.89 | 174.19 | 9.369 | 0.781 |
| 3 | 1129 | 1148.1 | 194.5 | 174.59 | 8.104 | 0.675 |
| 4 | 1112 | 1140.5 | 194.09 | 173.7 | 8.725 | 0.727 |
| 5 | 1064 | 1083.9 | 193.54 | 173.5 | 8.109 | 0.676 |
| 6 | 1081 | 1105.6 | 194.06 | 173.03 | 8.431 | 0.703 |
| 7 | 1124 | 1150.3 | 194.84 | 173.65 | 9.99 | 0.832 |
| 8 | 1102 | 1135.4 | 193.94 | 172.5 | 9.36 | 0.78 |
| 9 | 1068 | 1086.9 | 193 | 174.97 | 7.498 | 0.625 |
| 10 | 1100 | 1132.1 | 194.38 | 172.35 | 8.724 | 0.727 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
