# Checklist manuale — Musify

Data: 2026-11-09
Ambito: M4.5 (prestazioni e qualità locale) e M4 (Windows).
Queste verifiche richiedono una finestra reale, il mixer di Windows o la sessione
utente: non sono automatizzabili in modo affidabile. Eseguirle su una build
release (`cargo build -p musify --release`) e annotare data, DPI e dispositivo
audio usati.

## Preparazione

- Chiudere ogni istanza di Musify (`Get-Process musify`).
- Partire da dati isolati, così le prove non toccano la libreria personale:

```powershell
$env:MUSIFY_DATA_DIR = "$PWD\testdata\manual"
$env:MUSIFY_MUSIC_DIRS = "$PWD\testdata\manual\music"
New-Item -ItemType Directory -Force $env:MUSIFY_MUSIC_DIRS | Out-Null
.\target\release\musify.exe
```

- Aggiungere alla cartella almeno: un brano con copertina, uno senza copertina,
  un file corrotto o non audio (per verificare gli errori) e due album.
- Tenere aperto `%APPDATA%\Musify\` (o `$env:MUSIFY_DATA_DIR`) per controllare
  `config.toml` e `state.json`.

## 1. Dialogo cartelle e scansione

| Passo | Risultato atteso |
| --- | --- |
| Impostazioni → Aggiungi cartella | Si apre il dialogo nativo di Windows, non una finestra disegnata da Rune. |
| Annullare il dialogo | Nessuna cartella aggiunta, nessun errore in UI o nei log. |
| Scegliere una cartella con musica | La cartella compare in Impostazioni; parte la scansione con stato visibile. |
| Attendere la fine della scansione | Brani, album e artisti compaiono; le copertine si caricano senza bloccare la UI. |
| Aggiungere un file alla cartella mentre Musify è aperto | Il watcher lo rileva e lo aggiunge senza riavvio. |
| Rimuovere la cartella da Impostazioni | I brani di quella cartella spariscono; i file su disco restano. |
| Chiudere e riaprire | Le cartelle configurate restano; la scansione incrementale non rielabora tutto. |

Verificare nei log assenza di panic o errori SQLite. Un file corrotto deve
produrre un errore leggibile, non fermare la scansione degli altri file.

## 2. Tasti multimediali e SMTC

| Passo | Risultato atteso |
| --- | --- |
| Avviare un brano, premere il tasto Play/Pause della tastiera | Musify mette in pausa e riprende; Windows mostra l'overlay con il titolo corretto. |
| Premere Next e Previous | Cambia brano; l'overlay si aggiorna. |
| Premere Stop | La riproduzione si ferma e lo stato SMTC passa a stopped. |
| Usare i controlli dell'overlay di Windows | Play, pausa, next, previous e seek funzionano come dalla UI. |
| Controllare titolo, artista, album, durata e copertina nell'overlay | Corrispondono al brano in riproduzione; senza copertina compare il placeholder. |
| Aprire un altro player che prende la sessione SMTC | I tasti possono andare all'altro player: è il comportamento di Windows, non un bug di Musify. |

Ripetere con `MUSIFY_NO_SMTC=1`: l'app deve funzionare, senza overlay né tasti
multimediali.

## 3. DPI e multi-monitor

Provare almeno a 100%, 125%, 150% e 200% (Impostazioni → Schermo → Scala).

| Passo | Risultato atteso |
| --- | --- |
| Avvio a 125% | La finestra è 1120×620 in punti, non tagliata, con testo e controlli nitidi. |
| Avvio a 150% e 200% | Stessa dimensione logica, nessun contenuto tagliato o fuori schermo. |
| Ridimensionare e massimizzare | Layout stabile, sidebar e barra di riproduzione non si sovrappongono. |
| Spostare la finestra su un monitor con DPI diverso | Il contenuto resta leggibile; i click cadono sui controlli giusti. |
| Chiudere su un monitor secondario e riaprire | La finestra riappare interamente visibile e alla scala corretta. |
| Cambiare scala a Musify aperto | Nessun crash; al massimo serve riavviare per una nitidezza perfetta. |

Il workaround in `crates/ui/src/lib.rs` esegue un resize al primo `on_ready`:
se il bug si ripresenta, annotare DPI, monitor e risoluzione.

## 4. Chiusura forzata e riapertura

| Passo | Risultato atteso |
| --- | --- |
| Riprodurre un brano, attendere qualche secondo, chiudere con Ctrl+Q | Alla riapertura coda, brano e posizione sono ripristinati in pausa. |
| Uccidere il processo con `Stop-Process -Name musify -Force` durante la riproduzione | Nessun file `state.json.tmp` o `config.toml.tmp` residuo bloccante; alla riapertura l'ultimo stato valido è leggibile. |
| Forzare la chiusura durante una scansione | Alla riapertura la scansione riparte e la libreria non resta corrotta. |
| Forzare la chiusura durante un aggiornamento della coda | La coda riparte dall'ultimo salvataggio valido; nessun panic. |
| Riavviare due volte di seguito con la stessa coda | La coda non si duplica e l'indice corrente resta coerente. |
| Avviare con un file della coda cancellato | Il brano sparito viene saltato; gli altri restano. |

## 5. Coda e audio (M4.5)

| Passo | Risultato atteso |
| --- | --- |
| Con due brani in coda, mettere in pausa il primo e rimuovere il secondo dalla coda | Il primo resta in pausa alla stessa posizione; nessun frammento del secondo viene riprodotto. |
| Con due brani in coda, rimuovere il brano corrente | Parte il successivo dall'inizio, senza audio residuo del brano rimosso. |
| Spostare in coda il brano che sta per partire | Il brano corrente continua dalla posizione attuale; il nuovo ordine è rispettato. |
| Attivare/disattivare shuffle durante la riproduzione | Il brano corrente non riparte da zero e non si sentono click o salti. |
| Attendere la transizione gapless tra due brani | Nessun silenzio aggiunto, nessun click; titolo e posizione cambiano al confine. |
| Cercare di riprodurre un brano a 44,1 kHz su un dispositivo a 48 kHz | L'audio è continuo; con `MUSIFY_LINEAR_RESAMPLER=1` il comportamento resta valido. |
| Mettere in pausa e riprendere più volte | La posizione non salta e non ci sono underrun nei log. |

## 6. Radio internet (M5)

| Passo | Risultato atteso |
| --- | --- |
| Aprire Fonti → Radio | La vista mostra i preferiti salvati e i campi Nome/Paese/Genere-tag. |
| Cercare `jazz` | Arrivano stazioni; durante la ricerca compare lo stato di caricamento. |
| Salvare una stazione nei preferiti e riavviare | La stazione resta nei preferiti; la cache è in `radio.sqlite3`. |
| Avviare una stazione MP3 | La barra mostra `Connessione…`, poi `Buffering…`, poi `In riproduzione`; si sente l'audio. |
| Attendere il metadata ICY | Il titolo nella barra e nell'overlay SMTC cambia con il brano corrente. |
| Premere Pausa/Riprendi durante la radio | L'audio si ferma e riprende; lo stato dello stream resta coerente. |
| Disconnettere la rete per 10–20 s | Compare `Riconnessione…`; al ritorno della rete la stazione riprende. |
| Lasciare la rete disconnessa | Dopo alcuni tentativi compare `Offline`; la UI non si blocca e i comandi rispondono. |
| Chiudere Musify mentre la radio suona e riaprire | Nessuno stream viene riprodotto automaticamente e nessun URL radio viene salvato in `state.json`. |
| Cercare con la rete assente | Se esistono risultati in cache, compaiono con l'avviso `Offline: mostro i risultati salvati`. |
| Controllare il Task Manager a radio ferma | Il runtime Tokio del thread stream non resta attivo. |
| Provare una stazione HLS | Deve fallire con un errore chiaro, non bloccare la UI. |

## 7. Riposo

| Passo | Risultato atteso |
| --- | --- |
| Aprire Musify senza riprodurre e non toccare nulla per 30 s | La CPU resta a 0,0–0,3%; nessun redraw continuo. |
| Controllare il Task Manager | Nessun thread di rete attivo in M4.5; il runtime Tokio non è ancora introdotto. |
| Ridurre a icona e ripristinare | La UI si ridisegna solo quando serve. |

## Registro delle prove

| Data | DPI | Dispositivo audio | Esito | Note |
| --- | --- | --- | --- | --- |
| | | | | |
