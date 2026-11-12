# M7.2 - con chiave Jamendo

Data: 2026-11-11 16:16:54
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
| Startup | ready Rune | 1.620,00 ms (min 1.569,00, max 1.922,00) |
| Startup | tempo processo → ready | 1.654,69 ms (min 1.600,11, max 1.962,66) |
| Riposo | working set | 185,98 MB (min 182,52, max 186,14) |
| Riposo | private bytes | 171,57 MB (min 169,12, max 172,31) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,62) |
| Riproduzione | working set | 188,12 MB (min 187,81, max 188,30) |
| Riproduzione | private bytes | 171,39 MB (min 170,10, max 173,35) |
| Riproduzione | CPU un core | 9,98 % (min 7,81, max 13,73) |
| Riproduzione | CPU normalizzata | 0,83 % (min 0,65, max 1,15) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1842 | 1936.6 | 182.52 | 169.12 | 0.312 |
| 2 | 1612 | 1648.1 | 185.99 | 170.53 | 0.624 |
| 3 | 1628 | 1660.2 | 185.81 | 171.67 | 0.312 |
| 4 | 1922 | 1962.7 | 185.79 | 171.19 | 0 |
| 5 | 1609 | 1649.2 | 185.99 | 170.78 | 0 |
| 6 | 1607 | 1631.1 | 186.04 | 171.58 | 0.312 |
| 7 | 1599 | 1627.5 | 185.96 | 171.57 | 0.625 |
| 8 | 1569 | 1600.1 | 186.09 | 172.23 | 0 |
| 9 | 1635 | 1662.9 | 185.93 | 171.96 | 0.623 |
| 10 | 1703 | 1730.4 | 186.14 | 172.31 | 0.623 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1716 | 1749.1 | 188.29 | 173.27 | 9.361 | 0.78 |
| 2 | 1899 | 1943.7 | 188.11 | 173.35 | 10.913 | 0.909 |
| 3 | 1803 | 1837 | 188.2 | 172.71 | 13.734 | 1.145 |
| 4 | 1554 | 1583.1 | 188.14 | 171.8 | 7.807 | 0.651 |
| 5 | 1589 | 1629.1 | 187.81 | 171.39 | 11.218 | 0.935 |
| 6 | 1650 | 1692.7 | 188.3 | 171.18 | 9.978 | 0.831 |
| 7 | 1695 | 1737.6 | 188.13 | 171.39 | 11.236 | 0.936 |
| 8 | 1622 | 1647 | 188.12 | 171.39 | 9.983 | 0.832 |
| 9 | 1650 | 1676.8 | 188.05 | 170.81 | 8.414 | 0.701 |
| 10 | 1580 | 1613.9 | 188.1 | 170.1 | 8.412 | 0.701 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
