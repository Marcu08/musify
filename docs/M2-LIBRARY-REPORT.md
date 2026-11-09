# M2 — Libreria locale

Data: 2026-11-09
Crate: `musify-library`
Database: SQLite 3 (rusqlite 0.41, bundled, WAL, FTS5).

## Cosa è stato fatto

- Scanner incrementale con `walkdir`: confronto `mtime` + dimensione per saltare i
  file invariati; rimozione dei brani spariti solo sotto le cartelle configurate.
- Metadati con `lofty` 0.25: titolo, artista, album, album artist, numero
  traccia/disco, durata, anno, genere.
- Scrittura in batch da 500 file, transazioni SQLite e WAL. Il parsing dei
  metadati e delle copertine è parallelo (`rayon`); le scritture restano
  sequenziali per non contendere il database.
- Schema: `tracks`, `albums`, `artists`, `playlists`, `playlist_tracks`.
- Ricerca FTS5 (`unicode61 remove_diacritics 2`) su titolo/artista/album, con
  fallback `LIKE` indicizzato se FTS5 non è disponibile. Query dell'utente
  tokenizzata e sanitizzata; niente sintassi FTS5 iniettata.
- Copertine: front cover incorporata, altrimenti `cover.*`, `folder.*`,
  `front.*` nella cartella del brano. Ridimensionamento a 512 px, JPEG qualità
  85, cache su disco con nome = hash Blake3 del contenuto. Limite di 8 MB per
  le immagini esterne.
- Watcher `notify` con debounce di 750 ms: emette un segnale quando un file
  audio viene creato/modificato/rimosso; l'app può avviare la riscansione.
- CLI `library-cli`: `scan`, `search`, `stats`, `bench`.
- Implementazione di `musify_core::LibraryProvider`.

## Comandi

```powershell
cargo test -p musify-library

cargo run -p musify-library --bin library-cli -- scan C:\Musica
cargo run -p musify-library --bin library-cli -- search "citta"
cargo run -p musify-library --bin library-cli -- stats
cargo run -p musify-library --bin library-cli -- bench C:\Musica
```

## Risultati dei test

`cargo test -p musify-library`: **14 test passati**.

- Schema idempotente e statistiche iniziali.
- Inserimento/lettura di un brano e ricerca.
- Ricerca FTS5 senza accenti: `citta` trova `Città di notte`.
- Sanitizzazione query FTS5 e pattern LIKE.
- Estrazione copertina incorporata, file JPEG ≤ 512 px e riuso della cache.
- Scansione di mp3/flac/ogg, seconda scansione incrementale, rimozione file.
- Watcher: creazione di un file `.mp3` produce un evento entro 5 s.
- Estensioni audio case-insensitive e normalizzazione path Windows.

## Benchmark 10.000 file

Ambiente: release, Windows 11, Ryzen 5 5625U (12 thread), file in cartella
locale temporanea, Windows Defender attivo. 10.000 copie di un mp3 da 1,2 s
(192 MB), stessi tag (1 album/1 artista), senza copertine incorporate.

| Operazione | Tempo | Per file |
| --- | --- | --- |
| Prima scansione (indice freddo) | **8,51 s** | ~0,85 ms |
| Seconda scansione (incrementale) | **0,98 s** | ~0,10 ms |

La prima versione sequenziale impiegava 137 s; il collo di bottiglia era il
parsing dei metadati, non SQLite. Il parsing parallelo ha portato il tempo a
8,5 s. Con librerie reali, la presenza di copertine incorporate aggiunge il
costo di decodifica/ridimensionamento dell'immagine (non incluso in questo
benchmark).

## Problemi noti

- La normalizzazione dei path per il confronto è Windows-specifica
  (separatore `\`, case-insensitive). È coerente con il target del progetto.
- L'album è identificato da `(titolo, album artist)`. Se un album compilation
  non ha il tag album artist, brani con artisti diversi finiscono in album
  separati.
- Il watcher emette solo un segnale di "qualcosa è cambiato"; la scansione
  incrementale vera e propria viene lanciata dall'app. Non c'è ancora un
  debounce configurabile da TOML.
- Il benchmark usa file con gli stessi tag: album/artisti reali aumentano il
  numero di righe ma non cambiano il costo per file.
