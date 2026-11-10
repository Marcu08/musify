# M5.1 — Prova lunga radio

Data: 2026-11-10 12:30:02
Stazione: https://mangoradio.stream.laut.fm/mangoradio
Durata richiesta: 0.08 h · campione ogni 30 s
Campioni: 10 · da 2026-11-10 12:30:32 a 2026-11-10 12:35:03 (5.02 min)
Processo chiuso dallo script al termine.

## Crescita

| Metrica | Primo | Ultimo | Min | Max | Crescita |
| --- | ---: | ---: | ---: | ---: | ---: |
| Working set MB | 194.55 | 54.1 | 54.1 | 194.55 | -140.45 |
| Private bytes MB | 176.36 | 175.72 | 175.51 | 176.36 | -0.640000000000015 |
| CPU un core % | 0 | 2.39 | 0 | 8.04 | 2.39 |

CPU mediana: 4.41% di un core.

## Note

- La finestra viene portata in primo piano dopo il primo frame.
- Il processo è chiuso forzatamente alla fine; non è una prova di chiusura pulita.
- Il CSV grezzo (M51-SOAK-5M.csv) contiene ogni campione.
- Una crescita monotona e piccola può essere cache del decoder o frammentazione;
  una crescita continua e lineare dopo la prima ora è un possibile leak.
