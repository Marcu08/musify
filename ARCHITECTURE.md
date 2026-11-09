# Architettura

Musify è un workspace Cargo. Il core non conosce Rune, SQLite, cpal né Spotify: la UI comunica
solo con comandi ed eventi.

## Crate

| Crate | Responsabilità | Dipende da UI? |
| --- | --- | --- |
| `musify-core` | tipi di dominio, `Command`, `Event`, trait `PlaybackSource` e `LibraryProvider` | no |
| `musify-audio` | decodifica (symphonia), output (cpal/WASAPI), coda, seek, volume | no |
| `musify-library` | scanner (walkdir + lofty), indice SQLite, ricerca, cache copertine | no |
| `musify-ui` | viste e input Rune; traduce gesti in `Command` e applica `Event` | sì |
| `musify-spotify` | seconda `PlaybackSource` via librespot/rspotify (M5) | no |
| `musify-app` | config, logging, cablaggio dei canali, ciclo di vita | no |

## Flusso comandi/eventi

```text
┌──────────────┐   Command    ┌──────────────────┐   chiamate   ┌──────────────┐
│  UI (Rune)   │ ───────────▶ │  App controller  │ ───────────▶ │ audio engine │
│              │              │                  │              │ library      │
│              │ ◀─────────── │                  │ ◀─────────── │ spotify      │
└──────────────┘    Event     └──────────────────┘   Event      └──────────────┘
```

- La UI non chiama mai direttamente il motore audio o il database.
- Un canale `crossbeam-channel` porta i `Command` dalla UI al controller.
- Un secondo canale porta gli `Event` dal controller alla UI.
- Gli eventi di posizione sono emessi a bassa frequenza (circa 4–10 Hz) e solo mentre la
  riproduzione è attiva; a riposo non viene svegliata la UI.

## Thread

| Thread | Lavoro |
| --- | --- |
| UI | Rune/winit; costruisce solo le righe visibili |
| app controller | instrada comandi, possiede coda e stato di riproduzione |
| audio decode | decodifica symphonia e riempie il ring buffer |
| audio callback | consuma il ring buffer; nessuna allocazione, nessun lock bloccante |
| library scanner | walkdir + metadati + scritture SQLite |
| watcher | notify, con debounce, richiede una scansione incrementale |

## Persistenza

- `config.toml`: cartelle musicali, volume, shuffle/repeat, preferenze UI.
- `state.json`: coda, brano corrente, posizione, timestamp di salvataggio.
- `library.sqlite3`: brani, album, artisti, playlist, stato della scansione.
- `covers/`: copertine ridimensionate, nominate per hash del contenuto.

## Decisioni

- **M0**: Rune è pinnato a un rev `dev` documentato in `Cargo.toml`; nessun path locale.
- **Core indipendente**: nessun tipo Rune, cpal o rusqlite attraversa il confine pubblico.
- **Errori**: `thiserror` nei crate di dominio, `anyhow` solo nel binario.
- **Audio**: niente `unwrap` nel percorso di riproduzione; il callback non alloca e non
  acquisisce lock bloccanti.
- **Gapless**: da decidere in M1; se non affidabile, si documenta il comportamento reale
  invece di dichiararlo.

## Diagrammi di dettaglio

Saranno aggiunti in M1 (pipeline audio) e M2 (schema SQLite e flusso di scansione).
