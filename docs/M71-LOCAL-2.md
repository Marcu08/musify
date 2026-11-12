# M7.1 - Locale (seconda serie)

Data: 2026-11-11 09:43:14
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
| Startup | ready Rune | 1.843,50 ms (min 1.756,00, max 2.213,00) |
| Startup | tempo processo → ready | 1.882,62 ms (min 1.802,37, max 2.304,66) |
| Riposo | working set | 193,97 MB (min 182,50, max 194,18) |
| Riposo | private bytes | 170,30 MB (min 166,32, max 172,50) |
| Riposo | CPU un core | 0,47 % (min 0,00, max 2,50) |
| Riproduzione | working set | 195,80 MB (min 187,86, max 198,43) |
| Riproduzione | private bytes | 171,10 MB (min 170,08, max 173,81) |
| Riproduzione | CPU un core | 10,29 % (min 2,18, max 15,27) |
| Riproduzione | CPU normalizzata | 0,86 % (min 0,18, max 1,27) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2213 | 2304.7 | 182.5 | 166.32 | 0.623 |
| 2 | 2072 | 2120.4 | 185.53 | 171.2 | 2.498 |
| 3 | 2152 | 2184.9 | 185.87 | 168.69 | 1.248 |
| 4 | 1874 | 1946.8 | 194.1 | 170.09 | 0 |
| 5 | 1791 | 1849.3 | 193.9 | 172.01 | 0.625 |
| 6 | 1874 | 1906.1 | 194.18 | 172.5 | 0.312 |
| 7 | 1807 | 1838.9 | 193.98 | 168.68 | 0 |
| 8 | 1778 | 1821.9 | 194.03 | 170.66 | 0.625 |
| 9 | 1813 | 1859.1 | 193.97 | 168.82 | 0.312 |
| 10 | 1756 | 1802.4 | 194 | 170.51 | 0 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1874 | 1908.5 | 196.01 | 170.93 | 14.662 | 1.222 |
| 2 | 1830 | 1912.1 | 195.65 | 171.11 | 9.352 | 0.779 |
| 3 | 1843 | 1876.8 | 195.91 | 170.38 | 10.285 | 0.857 |
| 4 | 1813 | 1852 | 195.77 | 170.08 | 15.273 | 1.273 |
| 5 | 1894 | 1926.4 | 195.83 | 173.54 | 11.868 | 0.989 |
| 6 | 1837 | 1864.4 | 196.05 | 170.12 | 12.787 | 1.066 |
| 7 | 2022 | 2051 | 187.86 | 171.08 | 9.991 | 0.833 |
| 8 | 1862 | 1889.5 | 188.02 | 171.38 | 10.292 | 0.858 |
| 9 | 2128 | 2172.7 | 190.55 | 172.92 | 2.181 | 0.182 |
| 10 | 2495 | 2679.4 | 198.43 | 173.81 | 4.989 | 0.416 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
