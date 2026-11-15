# M8 - Fast

Data: 2026-11-15 13:40:07
Commit: `806de9f`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: fast, `musify.exe`
SMTC: attivo
Radio: non attiva
Podcast: non attivo
Metodo: 10 avvii, segnale `RUNE_READY_DIR`, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di `on_ready`; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | 1.732,00 ms (min 1.701,00, max 2.549,00) |
| Startup | tempo processo → ready | 1.762,03 ms (min 1.734,77, max 3.966,35) |
| Riposo | working set | 187,05 MB (min 186,93, max 198,01) |
| Riposo | private bytes | 170,40 MB (min 167,01, max 171,87) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,62) |
| Riproduzione | working set | 193,34 MB (min 188,84, max 198,15) |
| Riproduzione | private bytes | 171,17 MB (min 169,83, max 172,83) |
| Riproduzione | CPU un core | 9,36 % (min 4,36, max 10,61) |
| Riproduzione | CPU normalizzata | 0,78 % (min 0,36, max 0,88) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2549 | 3966.4 | 198.01 | 167.01 | 0 |
| 2 | 1750 | 1782.3 | 187.21 | 169.71 | 0.625 |
| 3 | 1739 | 1771.9 | 187.22 | 168.75 | 0.312 |
| 4 | 1723 | 1749.9 | 187.06 | 171.09 | 0 |
| 5 | 1701 | 1734.8 | 187 | 171.87 | 0.312 |
| 6 | 1724 | 1750.5 | 187.03 | 168.52 | 0.623 |
| 7 | 1724 | 1750.1 | 187 | 171.37 | 0 |
| 8 | 1743 | 1781.4 | 187 | 171.71 | 0.624 |
| 9 | 1743 | 1784.9 | 187.09 | 171.25 | 0.312 |
| 10 | 1725 | 1752.1 | 186.93 | 169.53 | 0.623 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1699 | 1737 | 188.84 | 172.37 | 10.608 | 0.884 |
| 2 | 1705 | 1756.7 | 189.21 | 172.83 | 9.362 | 0.78 |
| 3 | 1792 | 1824.8 | 189.21 | 171.59 | 4.361 | 0.363 |
| 4 | 1728 | 1752.9 | 189.36 | 171.77 | 8.744 | 0.729 |
| 5 | 1724 | 1763.5 | 189.6 | 170.64 | 7.498 | 0.625 |
| 6 | 1718 | 1754.8 | 197.25 | 169.83 | 10.292 | 0.858 |
| 7 | 1714 | 1740.9 | 197.41 | 170.48 | 9.364 | 0.78 |
| 8 | 1747 | 1774.2 | 197.24 | 171.18 | 9.356 | 0.78 |
| 9 | 1737 | 1769 | 198.15 | 170.57 | 8.123 | 0.677 |
| 10 | 1723 | 1749.1 | 197.09 | 171.15 | 9.374 | 0.781 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
