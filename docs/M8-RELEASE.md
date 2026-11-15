# M8 - Release

Data: 2026-11-15 13:36:34
Commit: `806de9f`
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
| Startup | ready Rune | 1.778,00 ms (min 1.690,00, max 3.241,00) |
| Startup | tempo processo → ready | 1.811,48 ms (min 1.722,96, max 4.594,40) |
| Riposo | working set | 186,00 MB (min 185,69, max 195,54) |
| Riposo | private bytes | 171,25 MB (min 167,20, max 172,52) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,62) |
| Riproduzione | working set | 188,10 MB (min 187,98, max 188,25) |
| Riproduzione | private bytes | 171,22 MB (min 169,60, max 173,30) |
| Riproduzione | CPU un core | 9,04 % (min 7,81, max 13,43) |
| Riproduzione | CPU normalizzata | 0,75 % (min 0,65, max 1,12) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 3241 | 4594.4 | 195.54 | 167.2 | 0.312 |
| 2 | 1988 | 2023.3 | 186 | 169.59 | 0.312 |
| 3 | 1841 | 1872.5 | 186 | 171.89 | 0.624 |
| 4 | 1807 | 1854 | 185.69 | 171.38 | 0.625 |
| 5 | 1734 | 1761.7 | 185.93 | 172.27 | 0.624 |
| 6 | 1818 | 1846.1 | 186.13 | 170.24 | 0 |
| 7 | 1730 | 1762.4 | 185.85 | 171.13 | 0.624 |
| 8 | 1719 | 1745.7 | 186.01 | 172.24 | 0 |
| 9 | 1690 | 1723 | 186.07 | 172.52 | 0 |
| 10 | 1749 | 1776.8 | 185.99 | 168.66 | 0.312 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1837 | 1863.7 | 188.04 | 170.94 | 11.223 | 0.935 |
| 2 | 1750 | 1786.3 | 188.16 | 173.3 | 9.04 | 0.753 |
| 3 | 1716 | 1760.2 | 188.05 | 172.37 | 13.429 | 1.119 |
| 4 | 1771 | 1812.6 | 188.11 | 169.78 | 10.299 | 0.858 |
| 5 | 1719 | 1773.7 | 188.25 | 170.75 | 8.732 | 0.728 |
| 6 | 1723 | 1748.2 | 187.98 | 169.6 | 9.041 | 0.753 |
| 7 | 1736 | 1763.8 | 188.16 | 171.5 | 8.724 | 0.727 |
| 8 | 1728 | 1770.7 | 188.09 | 171.79 | 9.042 | 0.754 |
| 9 | 1714 | 1750.9 | 188.17 | 173.18 | 7.81 | 0.651 |
| 10 | 1711 | 1741.2 | 188.03 | 170.18 | 8.099 | 0.675 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
