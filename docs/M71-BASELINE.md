# M7.1 - Baseline aca8763

Data: 2026-11-11 10:03:33
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
| Startup | ready Rune | 2.036,00 ms (min 1.787,00, max 3.079,00) |
| Startup | tempo processo → ready | 2.071,78 ms (min 1.815,27, max 4.653,99) |
| Riposo | working set | 194,97 MB (min 187,05, max 195,71) |
| Riposo | private bytes | 170,56 MB (min 166,07, max 172,07) |
| Riposo | CPU un core | 0,62 % (min 0,00, max 4,37) |
| Riproduzione | working set | 195,76 MB (min 195,45, max 198,21) |
| Riproduzione | private bytes | 172,73 MB (min 170,30, max 173,59) |
| Riproduzione | CPU un core | 9,67 % (min 0,94, max 15,31) |
| Riproduzione | CPU normalizzata | 0,81 % (min 0,08, max 1,28) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 3079 | 4654 | 195.71 | 170.29 | 4.37 |
| 2 | 2053 | 2086.2 | 194.89 | 170.23 | 0 |
| 3 | 2350 | 2377.4 | 195.03 | 171.32 | 0.624 |
| 4 | 2008 | 2040.6 | 194.02 | 166.07 | 1.869 |
| 5 | 2158 | 2195.3 | 194.9 | 168.68 | 0.624 |
| 6 | 2019 | 2057.4 | 194.87 | 172.07 | 0.312 |
| 7 | 1892 | 1918.8 | 195.05 | 170.96 | 0.624 |
| 8 | 1787 | 1815.3 | 195.07 | 170.4 | 0.312 |
| 9 | 2159 | 2197.8 | 195.19 | 170.72 | 0.623 |
| 10 | 1902 | 1944.2 | 187.05 | 171.17 | 0.312 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1809 | 1841.2 | 198.21 | 172.92 | 13.74 | 1.145 |
| 2 | 1887 | 1915.4 | 195.74 | 172.84 | 1.873 | 0.156 |
| 3 | 1958 | 1987.3 | 195.63 | 171.29 | 12.487 | 1.041 |
| 4 | 2213 | 2245.7 | 195.98 | 170.3 | 4.989 | 0.416 |
| 5 | 2047 | 2084.5 | 196.21 | 173.59 | 0.936 | 0.078 |
| 6 | 1896 | 1933.1 | 195.58 | 172.74 | 1.874 | 0.156 |
| 7 | 2200 | 2241.9 | 195.45 | 172.47 | 15.307 | 1.276 |
| 8 | 1820 | 1852.3 | 195.79 | 173.11 | 9.667 | 0.806 |
| 9 | 1932 | 1961.9 | 195.72 | 172.29 | 9.979 | 0.832 |
| 10 | 1925 | 1959.4 | 195.86 | 172.71 | 9.677 | 0.806 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
