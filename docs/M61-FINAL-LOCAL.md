# M6.1 - Finale locale

Data: 2026-11-11 03:16:25
Commit: `b124a11`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Radio: non attiva
Podcast: non attivo
Metodo: 10 avvii, segnale `RUNE_READY_DIR`, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di `on_ready`; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | 1.099,50 ms (min 1.054,00, max 1.721,00) |
| Startup | tempo processo → ready | 1.133,32 ms (min 1.074,92, max 1.766,86) |
| Riposo | working set | 185,43 MB (min 185,32, max 185,66) |
| Riposo | private bytes | 170,40 MB (min 168,63, max 171,35) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,31) |
| Riproduzione | working set | 187,60 MB (min 187,45, max 187,91) |
| Riproduzione | private bytes | 170,87 MB (min 169,46, max 172,45) |
| Riproduzione | CPU un core | 6,55 % (min 5,62, max 8,11) |
| Riproduzione | CPU normalizzata | 0,55 % (min 0,47, max 0,68) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1721 | 1766.9 | 185.43 | 170.48 | 0.301 |
| 2 | 1287 | 1316.6 | 185.45 | 170.32 | 0.312 |
| 3 | 1099 | 1145.8 | 185.66 | 170.58 | 0 |
| 4 | 1092 | 1120.8 | 185.43 | 171.35 | 0.312 |
| 5 | 1058 | 1074.9 | 185.49 | 169.26 | 0.312 |
| 6 | 1155 | 1189.7 | 185.42 | 169.8 | 0 |
| 7 | 1100 | 1120.3 | 185.58 | 171.34 | 0 |
| 8 | 1054 | 1087.9 | 185.37 | 168.84 | 0.312 |
| 9 | 1179 | 1196.5 | 185.38 | 171.25 | 0 |
| 10 | 1096 | 1116.6 | 185.32 | 168.63 | 0.312 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1148 | 1165.9 | 187.91 | 170.79 | 5.617 | 0.468 |
| 2 | 1050 | 1067.4 | 187.54 | 169.46 | 7.185 | 0.599 |
| 3 | 1051 | 1070.9 | 187.57 | 169.82 | 7.812 | 0.651 |
| 4 | 1090 | 1113.3 | 187.45 | 169.81 | 7.48 | 0.623 |
| 5 | 1093 | 1111.4 | 187.61 | 170.95 | 6.233 | 0.519 |
| 6 | 1082 | 1115.4 | 187.45 | 170.32 | 6.238 | 0.52 |
| 7 | 1099 | 1127.2 | 187.59 | 172.45 | 5.935 | 0.495 |
| 8 | 1051 | 1087 | 187.84 | 172.38 | 6.856 | 0.571 |
| 9 | 1051 | 1079.8 | 187.61 | 171.69 | 6.238 | 0.52 |
| 10 | 1117 | 1148.6 | 187.86 | 171.92 | 8.114 | 0.676 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
