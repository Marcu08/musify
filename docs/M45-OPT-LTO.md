# M4.5 — Ottimizzazione: profilo release LTO

Data: 2026-11-09 22:10:32
Commit: `98fe7ed`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Metodo: 10 avvii, segnale `RUNE_READY_DIR`, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di `on_ready`; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | 1.094,50 ms (min 1.062,00, max 2.033,00) |
| Startup | tempo processo → ready | 1.121,47 ms (min 1.087,31, max 2.471,60) |
| Riposo | working set | 184,92 MB (min 180,97, max 185,04) |
| Riposo | private bytes | 169,87 MB (min 164,38, max 171,63) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,62) |
| Riproduzione | working set | 186,82 MB (min 186,67, max 187,06) |
| Riproduzione | private bytes | 171,20 MB (min 169,38, max 172,33) |
| Riproduzione | CPU un core | 7,02 % (min 5,92, max 8,43) |
| Riproduzione | CPU normalizzata | 0,59 % (min 0,49, max 0,70) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2033 | 2471.6 | 180.97 | 164.38 | 0.312 |
| 2 | 1137 | 1154.5 | 184.96 | 169.64 | 0.312 |
| 3 | 1090 | 1112 | 184.9 | 171.52 | 0.312 |
| 4 | 1163 | 1181 | 184.68 | 170.96 | 0 |
| 5 | 1087 | 1120 | 185.04 | 168.43 | 0.312 |
| 6 | 1090 | 1118.9 | 184.93 | 170.09 | 0.312 |
| 7 | 1101 | 1122.9 | 184.91 | 171.4 | 0 |
| 8 | 1099 | 1123.1 | 184.78 | 168.04 | 0.624 |
| 9 | 1077 | 1102.1 | 184.94 | 171.63 | 0.625 |
| 10 | 1062 | 1087.3 | 185.02 | 168.43 | 0 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1148 | 1172.9 | 187.04 | 169.38 | 6.558 | 0.546 |
| 2 | 1077 | 1096.8 | 187.06 | 170.89 | 6.873 | 0.573 |
| 3 | 1074 | 1096.9 | 186.7 | 172.17 | 7.173 | 0.598 |
| 4 | 1143 | 1162.5 | 186.86 | 171.59 | 8.114 | 0.676 |
| 5 | 1110 | 1131.3 | 186.77 | 171.47 | 6.861 | 0.572 |
| 6 | 1088 | 1110.1 | 186.8 | 172.33 | 7.181 | 0.598 |
| 7 | 1085 | 1112.6 | 186.82 | 170.15 | 5.922 | 0.494 |
| 8 | 1085 | 1110.4 | 186.67 | 172.26 | 7.803 | 0.65 |
| 9 | 1185 | 1202.5 | 186.82 | 170.54 | 8.428 | 0.702 |
| 10 | 1076 | 1098.7 | 186.91 | 170.94 | 6.561 | 0.547 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
