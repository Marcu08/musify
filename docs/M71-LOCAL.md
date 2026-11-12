# M7.1 - Locale

Data: 2026-11-11 09:38:58
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
| Startup | ready Rune | 1.844,50 ms (min 1.800,00, max 2.871,00) |
| Startup | tempo processo → ready | 1.875,57 ms (min 1.831,29, max 4.364,58) |
| Riposo | working set | 185,78 MB (min 182,92, max 206,13) |
| Riposo | private bytes | 170,52 MB (min 159,40, max 172,09) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 1,56) |
| Riproduzione | working set | 187,89 MB (min 187,73, max 188,00) |
| Riproduzione | private bytes | 171,30 MB (min 170,05, max 172,49) |
| Riproduzione | CPU un core | 11,25 % (min 9,97, max 14,34) |
| Riproduzione | CPU normalizzata | 0,94 % (min 0,83, max 1,20) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2871 | 4364.6 | 206.13 | 169.49 | 0 |
| 2 | 2123 | 2161.9 | 182.92 | 159.4 | 0 |
| 3 | 1812 | 1856.3 | 185.79 | 170.16 | 0.312 |
| 4 | 1801 | 1834.9 | 185.66 | 172.04 | 1.559 |
| 5 | 1887 | 1917 | 185.81 | 172.09 | 0 |
| 6 | 2214 | 2263.3 | 185.75 | 170.8 | 0.624 |
| 7 | 1827 | 1862.4 | 185.68 | 168.65 | 0.623 |
| 8 | 1836 | 1865.9 | 185.78 | 171.22 | 0.312 |
| 9 | 1853 | 1885.2 | 185.9 | 171.92 | 0 |
| 10 | 1800 | 1831.3 | 185.83 | 170.24 | 0.312 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1789 | 1815.9 | 187.79 | 170.56 | 11.246 | 0.937 |
| 2 | 1786 | 1822 | 187.89 | 170.05 | 11.23 | 0.936 |
| 3 | 1875 | 1911.1 | 187.89 | 170.99 | 9.973 | 0.831 |
| 4 | 1974 | 2001.5 | 187.91 | 172.07 | 9.967 | 0.831 |
| 5 | 1839 | 1881.1 | 187.73 | 172 | 12.159 | 1.013 |
| 6 | 1931 | 1965.1 | 187.96 | 171.6 | 13.423 | 1.119 |
| 7 | 1861 | 1888.7 | 188 | 170.83 | 12.176 | 1.015 |
| 8 | 1877 | 1917.7 | 187.86 | 170.75 | 14.337 | 1.195 |
| 9 | 1824 | 1854.7 | 187.99 | 171.96 | 11.247 | 0.937 |
| 10 | 1820 | 1860.3 | 187.76 | 172.49 | 11.229 | 0.936 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
