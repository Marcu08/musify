# M5.1 - Finale locale

Data: 2026-11-10 22:11:22
Commit: `47c3331`
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
| Startup | ready Rune | 1.145,50 ms (min 1.106,00, max 2.105,00) |
| Startup | tempo processo → ready | 1.176,01 ms (min 1.123,70, max 2.784,00) |
| Riposo | working set | 185,40 MB (min 185,34, max 185,54) |
| Riposo | private bytes | 170,25 MB (min 169,04, max 171,88) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,62) |
| Riproduzione | working set | 187,53 MB (min 187,43, max 187,69) |
| Riproduzione | private bytes | 171,08 MB (min 169,17, max 172,98) |
| Riproduzione | CPU un core | 8,43 % (min 6,54, max 9,37) |
| Riproduzione | CPU normalizzata | 0,70 % (min 0,55, max 0,78) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2105 | 2784 | 185.4 | 169.97 | 0.312 |
| 2 | 1136 | 1155.9 | 185.34 | 171.6 | 0.623 |
| 3 | 1121 | 1144.5 | 185.4 | 170.17 | 0.625 |
| 4 | 1183 | 1208.9 | 185.42 | 170.14 | 0 |
| 5 | 1151 | 1181.5 | 185.41 | 170.04 | 0.312 |
| 6 | 1123 | 1158.2 | 185.54 | 171.07 | 0.312 |
| 7 | 1140 | 1174.6 | 185.42 | 169.04 | 0 |
| 8 | 1227 | 1253.1 | 185.37 | 171.88 | 0 |
| 9 | 1106 | 1123.7 | 185.35 | 170.32 | 0 |
| 10 | 1155 | 1177.4 | 185.36 | 170.63 | 0.623 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1321 | 1351.6 | 187.45 | 172.98 | 7.489 | 0.624 |
| 2 | 1217 | 1241.8 | 187.52 | 170.33 | 8.427 | 0.702 |
| 3 | 1224 | 1254.3 | 187.55 | 170.94 | 8.433 | 0.703 |
| 4 | 1144 | 1161.1 | 187.43 | 169.94 | 9.056 | 0.755 |
| 5 | 1211 | 1239.2 | 187.47 | 169.17 | 8.433 | 0.703 |
| 6 | 1149 | 1168.5 | 187.69 | 172.7 | 8.112 | 0.676 |
| 7 | 1173 | 1204.8 | 187.68 | 172.42 | 8.108 | 0.676 |
| 8 | 1147 | 1166.4 | 187.5 | 172.02 | 9.354 | 0.779 |
| 9 | 1162 | 1185.9 | 187.58 | 171.23 | 6.544 | 0.545 |
| 10 | 1111 | 1149.3 | 187.53 | 170.52 | 9.369 | 0.781 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
