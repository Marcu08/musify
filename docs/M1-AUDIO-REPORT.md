# M1 — Motore audio

Data: 2026-11-09
Crate: `musify-audio`
Output di prova: WASAPI condiviso, 48.000 Hz, 2 canali.

## Cosa è stato fatto

- Decodifica **symphonia 0.6** per mp3, flac, ogg/vorbis, wav, aac/m4a.
- Output **cpal 0.18** su WASAPI, thread di output dedicato e ring buffer
  lock-free `rtrb` (2 secondi di audio stereo). Il callback non alloca, non
  acquisisce lock e non fa I/O: popping di campioni, gain e scrittura del buffer.
- Thread engine dedicato: comandi, coda, shuffle/repeat, seek, volume, apertura
  del brano successivo in anticipo.
- Posizione calcolata sui frame effettivamente consumati dal callback, non sui
  frame decodificati: la UI non anticipa l'audio nel buffer.
- Gapless **sample-continuous** quando il brano successivo è decodificabile in
  tempo: i sample del brano successivo entrano nello stesso ring buffer e la
  transizione dei metadati avviene al confine di frame. Non c'è crossfade.
  Un `Next` manuale svuota il buffer, quindi può avere un piccolo gap.
- Resampling lineare verso la frequenza del dispositivo. Scelta esplicita:
  economica e deterministica, non un resampler sinc; da rivalutare se la qualità
  non basta.
- CLI interattiva `audio-cli`.
- 5 asset di test minuscoli generati con ffmpeg (`scripts/make-test-assets.ps1`).

## Comandi

```powershell
cargo test -p musify-audio

cargo run -p musify-audio --bin audio-cli -- `
  crates/audio/tests/assets/tone.mp3 `
  crates/audio/tests/assets/tone.flac `
  crates/audio/tests/assets/tone.ogg

# comandi da stdin:
# pause, resume, toggle, next, prev, seek 12.5, vol 70, status, quit

cargo run -p musify-audio --bin audio-cli -- --seconds 10 testdata\long.flac
```

## Risultati dei test

`cargo test -p musify-audio`: **14 test passati**.

- Decodifica di mp3, flac, ogg, wav, m4a.
- Seek accurato su flac.
- Resampler: identità sample-exact, rapporto 44.1→48 kHz entro 2 frame.
- Coda: avanzamento sequenziale, repeat-all, repeat-one, shuffle come
  permutazione, rimozione del brano corrente, rimozione prima del corrente,
  spostamento di un elemento.

## Misure

Misura su build **release**, `long.flac` (20 s, 44.1 kHz), 15 s di riproduzione,
volume 80, output 48 kHz stereo. CPU di processo campionata su una finestra di
5 secondi dopo il warm-up:

| Metrica | Valore |
| --- | --- |
| CPU (un core) | **0,93 %** |
| CPU normalizzata su 12 thread | **0,08 %** |
| RAM working set | **15,9 MB** |
| Underrun del callback | **0** |

La riproduzione di 4 brani consecutivi (mp3, flac, ogg, wav, m4a) ha prodotto
transizioni di brano senza underrun dopo l'aggiunta del prefill.

## Problemi noti

- Il resampler è lineare: con sorgenti a 44.1 kHz su uscite a 48 kHz introduce
  una colorazione minima. Se serve qualità superiore, sostituire solo
  `resample.rs` con `rubato` (l'interfaccia del decoder resta invariata).
- Modificare la coda mentre un brano è già stato decodificato in anticipo è
  best-effort: la rimozione vale per il futuro, non per l'audio già nel ring
  buffer. Da rifinire in M3/M4 se l'uso reale lo richiede.
- Il seek è accurato a livello di packet e poi scarta i campioni in eccesso;
  la posizione mostrata è quella effettiva del decoder, non quella richiesta.
- Nessun controllo DRM: solo file locali non protetti.
