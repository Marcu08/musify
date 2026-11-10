# M5.2 - Finale locale

Data: 2026-11-11 00:02:30
Commit: `6c16263`
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
| Startup | ready Rune | 1.110,00 ms (min 1.059,00, max 2.370,00) |
| Startup | tempo processo → ready | 1.133,24 ms (min 1.076,90, max 3.101,65) |
| Riposo | working set | 185,37 MB (min 185,25, max 197,38) |
| Riposo | private bytes | 169,62 MB (min 168,26, max 172,07) |
| Riposo | CPU un core | 0,16 % (min 0,00, max 0,62) |
| Riproduzione | working set | 187,35 MB (min 187,29, max 187,59) |
| Riproduzione | private bytes | 170,59 MB (min 169,41, max 171,86) |
| Riproduzione | CPU un core | 7,80 % (min 5,93, max 9,68) |
| Riproduzione | CPU normalizzata | 0,65 % (min 0,49, max 0,81) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2370 | 3101.7 | 197.38 | 170.25 | 0 |
| 2 | 1131 | 1151.6 | 185.39 | 168.96 | 0.312 |
| 3 | 1136 | 1154.2 | 185.36 | 169.8 | 0.312 |
| 4 | 1100 | 1118.3 | 185.25 | 172.07 | 0 |
| 5 | 1110 | 1134.6 | 185.33 | 168.26 | 0.624 |
| 6 | 1089 | 1125.2 | 185.29 | 168.48 | 0 |
| 7 | 1109 | 1128.7 | 185.34 | 170.14 | 0.312 |
| 8 | 1110 | 1131.9 | 185.38 | 169.25 | 0 |
| 9 | 1129 | 1165.6 | 185.38 | 169.44 | 0.623 |
| 10 | 1059 | 1076.9 | 185.42 | 170.54 | 0 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1082 | 1101.8 | 187.55 | 170.36 | 6.561 | 0.547 |
| 2 | 1093 | 1114.9 | 187.52 | 170.81 | 9.682 | 0.807 |
| 3 | 1162 | 1181.9 | 187.41 | 169.71 | 5.934 | 0.495 |
| 4 | 1077 | 1098.7 | 187.36 | 171.37 | 7.496 | 0.625 |
| 5 | 1079 | 1098.4 | 187.59 | 170.45 | 9.681 | 0.807 |
| 6 | 1076 | 1095.6 | 187.31 | 169.41 | 9.061 | 0.755 |
| 7 | 1133 | 1155.4 | 187.29 | 170.68 | 7.478 | 0.623 |
| 8 | 1081 | 1100.1 | 187.3 | 170.49 | 9.664 | 0.805 |
| 9 | 1084 | 1112.4 | 187.33 | 171.86 | 8.107 | 0.676 |
| 10 | 1090 | 1118.1 | 187.32 | 171.61 | 7.495 | 0.625 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
