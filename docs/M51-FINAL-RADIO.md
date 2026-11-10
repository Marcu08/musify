# M5.1 - Finale radio

Data: 2026-11-10 22:12:54
Commit: `47c3331`
Macchina: AMD Ryzen 5 5625U with Radeon Graphics · 12 thread · 7.3 GB RAM
OS: Microsoft Windows 11 Home 10.0.26300
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
Build: release, `musify.exe`
SMTC: attivo
Radio: https://mangoradio.stream.laut.fm/mangoradio
Metodo: 10 avvii, segnale `RUNE_READY_DIR`, 2 s di assestamento, 5 s di campionamento CPU.
Il "ready" di Rune arriva dopo il primo frame e il ritorno di `on_ready`; non è il primo pixel puro.

## Risultati mediani

| Fase | Metrica | Mediana (min–max) |
| --- | --- | --- |
| Startup | ready Rune | n/d |
| Startup | tempo processo → ready | n/d |
| Riposo | working set | n/d |
| Riposo | private bytes | n/d |
| Riposo | CPU un core | n/d |
| Riproduzione | working set | 194,88 MB (min 188,22, max 195,30) |
| Riproduzione | private bytes | 174,81 MB (min 170,81, max 176,58) |
| Riproduzione | CPU un core | 8,28 % (min 6,56, max 10,29) |
| Riproduzione | CPU normalizzata | 0,69 % (min 0,55, max 0,86) |

## Valori grezzi — startup e riposo

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core |
| ---: | ---: | ---: | ---: | ---: | ---: |

## Valori grezzi — riproduzione

| # | ready ms | processo→ready ms | WS MB | Private MB | CPU % un core | CPU % normalizzata |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1419 | 1469.3 | 194.64 | 174.84 | 7.603 | 0.634 |
| 2 | 1167 | 1199.2 | 194.95 | 176.42 | 10.281 | 0.857 |
| 3 | 1201 | 1241.5 | 192.2 | 172.43 | 9.353 | 0.779 |
| 4 | 1238 | 1256.8 | 195.3 | 174.51 | 9.055 | 0.755 |
| 5 | 1284 | 1312.4 | 195.04 | 176.58 | 7.805 | 0.65 |
| 6 | 1181 | 1208.5 | 194.84 | 174.77 | 8.746 | 0.729 |
| 7 | 1140 | 1158.6 | 194.57 | 175.4 | 7.478 | 0.623 |
| 8 | 1267 | 1295.5 | 194.91 | 175.29 | 10.295 | 0.858 |
| 9 | 1266 | 1287.8 | 195.09 | 173.21 | 6.871 | 0.573 |
| 10 | 1215 | 1235.2 | 188.22 | 170.81 | 6.559 | 0.547 |

## Note di lettura

- Le misure di avvio su Windows hanno varianza alta (Defender, cache shader, processi
  concorrenti). Il CSV conserva tutti i valori; qui si riportano mediana e intervallo,
  non solo il caso migliore.
- A riposo non ci sono eventi di posizione né scan in corso: la UI non deve ridisegnare.
- La fase di riproduzione usa `MUSIFY_AUTOPLAY=1` e parte da una libreria già scansionata.
- Per confrontare due build usare lo stesso `-NoSmtc` e lo stesso numero di run.
