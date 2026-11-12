# M7.2 - senza chiave Jamendo

Data: 2026-11-11 16:13:32
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
| Startup | ready Rune | 1.905,00 ms (min 1.677,00, max 3.094,00) |
| Startup | tempo processo → ready | 1.944,92 ms (min 1.707,68, max 4.626,06) |
| Riposo | working set | 193,74 MB (min 193,50, max 202,03) |
| Riposo | private bytes | 170,19 MB (min 168,86, max 171,71) |
| Riposo | CPU un core | 0,31 % (min 0,00, max 0,62) |
| Riproduzione | working set | 187,98 MB (min 187,77, max 195,94) |
| Riproduzione | private bytes | 171,16 MB (min 170,04, max 173,54) |
| Riproduzione | CPU un core | 10,76 % (min 9,98, max 12,49) |
| Riproduzione | CPU normalizzata | 0,90 % (min 0,83, max 1,04) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 3094 | 4626.1 | 202.03 | 168.86 | 0.312 |
| 2 | 2278 | 2325.5 | 193.76 | 170.23 | 0.312 |
| 3 | 1978 | 2008.6 | 193.5 | 170.34 | 0 |
| 4 | 1768 | 1803 | 193.71 | 171.68 | 0.311 |
| 5 | 1911 | 1947.8 | 193.79 | 169.6 | 0 |
| 6 | 1899 | 1942 | 193.64 | 169.98 | 0.623 |
| 7 | 2060 | 2103.1 | 193.95 | 170.16 | 0 |
| 8 | 1677 | 1707.7 | 193.5 | 169.75 | 0.623 |
| 9 | 1777 | 1808.4 | 193.8 | 171.38 | 0 |
| 10 | 1779 | 1821.3 | 193.67 | 171.71 | 0.312 |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1679 | 1712.4 | 195.87 | 173.54 | 10.912 | 0.909 |
| 2 | 1738 | 1767.1 | 195.94 | 170.33 | 12.492 | 1.041 |
| 3 | 1866 | 1912.2 | 187.98 | 173.12 | 10.305 | 0.859 |
| 4 | 1734 | 1773.6 | 188.05 | 170.04 | 10.917 | 0.91 |
| 5 | 1770 | 1803.8 | 187.98 | 172.66 | 10.602 | 0.883 |
| 6 | 1799 | 1838.1 | 188.16 | 171.29 | 10.309 | 0.859 |
| 7 | 1745 | 1782.1 | 187.94 | 172.6 | 11.871 | 0.989 |
| 8 | 1862 | 1898.9 | 187.81 | 171.04 | 9.979 | 0.832 |
| 9 | 1719 | 1761.5 | 187.77 | 171.04 | 11.239 | 0.937 |
| 10 | 1742 | 1770.7 | 187.85 | 170.56 | 9.976 | 0.831 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
