# M7.1 - Finale locale

Data: 2026-11-11 14:32:50
Commit: `aca8763`
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
| Startup | ready Rune | 1.486,00 ms (min 1.476,00, max 2.378,00) |
| Startup | tempo processo → ready | 1.524,23 ms (min 1.497,17, max 3.927,08) |
| Riposo | working set | 124,72 MB (min 124,51, max 136,98) |
| Riposo | private bytes | 169,92 MB (min 168,78, max 171,98) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,94) |
| Riproduzione | working set | 126,83 MB (min 126,75, max 127,03) |
| Riproduzione | private bytes | 171,71 MB (min 169,96, max 172,71) |
| Riproduzione | CPU un core | 9,51 % (min 7,17, max 11,23) |
| Riproduzione | CPU normalizzata | 0,79 % (min 0,60, max 0,94) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2378 | 3927.1 | 136.98 | 170.6 | 0.623 |
| 2 | 1621 | 1644.2 | 124.75 | 169.82 | 0 |
| 3 | 1546 | 1581.5 | 124.68 | 168.96 | 0.312 |
| 4 | 1487 | 1509 | 124.67 | 170.02 | 0.312 |
| 5 | 1509 | 1553.7 | 124.51 | 171.76 | 0.935 |
| 6 | 1482 | 1521.7 | 124.79 | 171.75 | 0.623 |
| 7 | 1476 | 1497.2 | 124.63 | 169.05 | 0 |
| 8 | 1484 | 1508.3 | 124.67 | 168.78 | 0 |
| 9 | 1478 | 1502.4 | 124.81 | 171.98 | 0.312 |
| 10 | 1485 | 1526.8 | 124.77 | 169.05 | 0 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1491 | 1513.2 | 126.97 | 172.07 | 9.369 | 0.781 |
| 2 | 1490 | 1524.5 | 126.79 | 172.33 | 9.667 | 0.806 |
| 3 | 1488 | 1512 | 126.75 | 169.96 | 10.608 | 0.884 |
| 4 | 1638 | 1661.5 | 126.79 | 171.02 | 9.66 | 0.805 |
| 5 | 1766 | 1790.1 | 126.88 | 171.99 | 11.235 | 0.936 |
| 6 | 1643 | 1685.3 | 127.03 | 170.07 | 8.435 | 0.703 |
| 7 | 1600 | 1636 | 126.86 | 171.43 | 8.121 | 0.677 |
| 8 | 1606 | 1633.9 | 126.78 | 172.39 | 10.905 | 0.909 |
| 9 | 1544 | 1586.2 | 126.88 | 171.23 | 7.17 | 0.598 |
| 10 | 1576 | 1600.2 | 126.77 | 172.71 | 9.345 | 0.779 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
