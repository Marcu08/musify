# M4.5 — Ottimizzazione: caricamento lazy (scartata)

Data: 2026-11-09 22:34:30
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
| Startup | ready Rune | 1.097,00 ms (min 1.082,00, max 2.065,00) |
| Startup | tempo processo → ready | 1.118,66 ms (min 1.104,14, max 2.526,26) |
| Riposo | working set | 189,98 MB (min 189,87, max 196,11) |
| Riposo | private bytes | 192,24 MB (min 190,17, max 198,00) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,94) |
| Riproduzione | working set | 191,79 MB (min 191,55, max 193,85) |
| Riproduzione | private bytes | 192,75 MB (min 190,86, max 193,23) |
| Riproduzione | CPU un core | 7,49 % (min 2,49, max 8,12) |
| Riproduzione | CPU normalizzata | 0,62 % (min 0,21, max 0,68) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2065 | 2526.3 | 196.11 | 198 | 0.625 |
| 2 | 1099 | 1122 | 189.87 | 191.48 | 0.312 |
| 3 | 1082 | 1115.3 | 189.96 | 190.17 | 0.312 |
| 4 | 1111 | 1136.3 | 190.09 | 192.25 | 0 |
| 5 | 1142 | 1160 | 189.95 | 191.3 | 0.312 |
| 6 | 1086 | 1109.1 | 190.2 | 193.15 | 0 |
| 7 | 1084 | 1104.1 | 189.95 | 192.22 | 0.936 |
| 8 | 1212 | 1233.4 | 189.97 | 192.41 | 0 |
| 9 | 1095 | 1113.9 | 190 | 193.09 | 0.312 |
| 10 | 1089 | 1106 | 189.99 | 191.61 | 0 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1080 | 1104.7 | 191.55 | 192.7 | 7.493 | 0.624 |
| 2 | 1105 | 1127.1 | 191.7 | 193.21 | 5.936 | 0.495 |
| 3 | 1086 | 1106.2 | 191.88 | 193.14 | 7.491 | 0.624 |
| 4 | 1070 | 1104.5 | 191.74 | 192.8 | 8.124 | 0.677 |
| 5 | 1098 | 1119.3 | 191.79 | 191.63 | 7.495 | 0.625 |
| 6 | 1095 | 1111.7 | 191.8 | 193.15 | 7.803 | 0.65 |
| 7 | 1061 | 1078.7 | 191.79 | 193.23 | 7.175 | 0.598 |
| 8 | 1096 | 1129 | 191.75 | 191.66 | 6.244 | 0.52 |
| 9 | 1240 | 1264.3 | 191.94 | 190.86 | 2.494 | 0.208 |
| 10 | 1296 | 1333.8 | 193.85 | 192.7 | 7.484 | 0.624 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
