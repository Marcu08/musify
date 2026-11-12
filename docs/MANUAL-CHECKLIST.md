# Checklist manuale — Musify

Data: 2026-11-11
Ambito: M7.2 (Jamendo), M7.1 (playlist), M6.1 (HTTP Range e podcast),
M5.2 (registro fonti), M5.1/M5 (radio) e M4 (Windows).
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
| Lanciare una seconda ricerca mentre la prima è in corso | Arriva solo il risultato della ricerca più recente; la UI non resta bloccata. |
| Attendere il caricamento delle icone | Le righe passano dal badge FM alla favicon; le icone sono in `radio-icons/`. |
| Cercare con cache più vecchia di 7 giorni e rete assente | La cache scaduta non viene mostrata; compare l'errore di rete. |
| Salvare una stazione nei preferiti e riavviare | La stazione resta nei preferiti; la cache è in `radio.sqlite3`. |
| Avviare una stazione MP3 | La barra mostra `Connessione…`, poi `Buffering…`, poi `In riproduzione`; si sente l'audio. |
| Attendere il metadata ICY | Il titolo nella barra e nell'overlay SMTC cambia con il brano corrente. |
| Premere Pausa/Riprendi durante la radio | L'audio si ferma e riprende; lo stato dello stream resta coerente. |
| Disconnettere la rete per 10–20 s | Compare `Riconnessione…`; al ritorno della rete la stazione riprende. |
| Lasciare la rete disconnessa | Dopo alcuni tentativi compare `Offline`; la UI non si blocca e i comandi rispondono. |
| Chiudere Musify mentre la radio suona e riaprire | Nessuno stream viene riprodotto automaticamente e nessun URL radio viene salvato in `state.json`. |
| Cercare con la rete assente | Se esistono risultati in cache, compaiono con l'avviso `Offline: mostro i risultati salvati`. |
| Controllare il Task Manager a radio ferma | Il runtime Tokio del thread stream non resta attivo; a riposo nessun runtime rete. |
| Provare una stazione HLS | Deve fallire con "gli stream HLS non sono supportati", non bloccare la UI. |
| Avviare `scripts/soak-radio.ps1` per 2 ore | Private bytes stabili; working set può essere trim quando la finestra perde il foreground. |

## 7. Registro fonti e backend modulare (M5.2)

| Passo | Risultato atteso |
| --- | --- |
| Aprire Fonti → Radio dopo l'aggiornamento | La vista è identica a M5.1: stessi campi, preferiti e badge `Radio`. |
| Cercare, aprire i preferiti e avviare una stazione | Le tre azioni passano dal registro fonti e producono lo stesso risultato di M5.1. |
| Avviare due ricerche in rapida successione | Arriva solo il risultato dell'ultima; nessun errore residuo della prima. |
| Controllare i log all'avvio | Nessun errore di registrazione della fonte `radio`; una fonte sconosciuta produce un errore chiaro, non un panic. |
| Chiudere e riaprire con una coda locale | `state.json` viene ripristinato in pausa; la radio non viene salvata. |
| Con la riproduzione attiva, controllare l'overlay Windows | Titolo, artista, copertina e posizione si aggiornano come in M5.1. |
| Aprire `%APPDATA%\Musify\` | `config.toml`, `state.json`, `library.sqlite3`, `radio.sqlite3` e `radio-icons/` sono gli unici file attesi. |

## 8. HTTP Range e podcast (M6.1)

| Passo | Risultato atteso |
| --- | --- |
| Aprire Fonti → Podcast | La vista mostra ricerca, pulsante Iscrizioni e l'elenco vuoto o salvato. |
| Cercare `The Daily` | Arrivano risultati iTunes con copertina; durante la ricerca compare lo stato di caricamento. |
| Iscriviti a un podcast e apri la scheda | Il feed viene letto una volta; compaiono titolo, data, durata e descrizione degli episodi. |
| Premere Aggiorna feed | Gli episodi vengono riletti; watched e posizione degli episodi esistenti non vengono azzerati. |
| Avviare un episodio MP3 con Range | L'audio parte; la barra mostra la durata e il trascinamento esegue un seek. |
| Portare la posizione a metà episodio, mettere in pausa e chiudere Musify | Alla riapertura e al nuovo avvio dell'episodio la riproduzione riprende dalla posizione salvata. |
| Arrivare a meno di 5 secondi dalla fine | L'episodio viene marcato `Ascoltato`; al riavvio non riparte dalla fine. |
| Premere `Segna ascoltato` / `Non ascoltato` | Lo stato cambia e resta dopo il riavvio. |
| Controllare l'overlay Windows durante un episodio | Mostra titolo episodio, autore/podcast, copertina e durata; i comandi play/pausa/seek funzionano. |
| Provare un server senza `Accept-Ranges` (test mock) | L'episodio parte ma la UI mostra `Stream non seekable` e il trascinamento è disabilitato. |
| Controllare Task Manager a riposo con Podcast aperto | Nessun polling o runtime di rete attivo: la CPU resta a 0,0-0,3%. |
| Controllare `%APPDATA%\Musify\` | `podcasts.sqlite3` e `podcast-covers/` contengono solo iscrizioni, episodi e copertine usate. |

## 9. Playlist (M7.1)

| Passo | Risultato atteso |
| --- | --- |
| Aprire Playlist nella barra laterale | La vista mostra il campo nome, Crea, l'elenco playlist e Importa M3U8. |
| Creare una playlist dal nome | Compare nell'elenco con conteggio 0 e viene selezionata. |
| Rinominarla e riavviare Musify | Il nuovo nome resta; nessun file aggiuntivo in `%APPDATA%\Musify\`. |
| Aggiungere un brano dalla libreria con `+` | Il brano compare nella playlist selezionata; il conteggio si aggiorna. |
| Cercare un brano e usare `+` nel risultato | Il brano viene aggiunto alla playlist selezionata. |
| Aggiungere dalla coda con `+` e con `Aggiungi coda` | I brani locali entrano nella playlist; radio e podcast vengono ignorati con un messaggio chiaro. |
| Con un brano in riproduzione, premere `Aggiungi brano corrente` | Il brano corrente entra nella playlist. |
| Selezionare un elemento e usare ↑/↓ e `Alt+↑`/`Alt+↓` | L'ordine cambia e resta dopo il riavvio; il drag del mouse non è richiesto. |
| Premere `Riproduci playlist` | La coda viene sostituita e parte il primo brano disponibile. |
| Premere `Aggiungi alla coda` | I brani vengono accodati senza interrompere la riproduzione corrente. |
| Con una coda locale, premere `Salva coda come playlist` | Viene creata una playlist con i brani della coda. |
| Rimuovere un file dal disco e riaprire la playlist | La riga resta visibile in grigio con `File non trovato`. |
| Riprodurre una playlist con un file mancante | Il file viene saltato senza errori; gli altri brani suonano. |
| Premere `Rimuovi mancanti` | Gli elementi mancanti spariscono e le posizioni restano compatte. |
| Esportare in M3U8 con percorsi relativi e assoluti | Il file è UTF-8, contiene `#EXTM3U`/`#EXTINF` e si riapre con un player esterno. |
| Importare lo stesso M3U8 | Viene creata una playlist con titolo, artista e durata corretti. |
| Eliminare una playlist | Sparisce dall'elenco; i file musicali restano su disco. |
| Controllare `%APPDATA%\Musify\library.sqlite3` | Le tabelle `playlists` e `playlist_items` contengono i dati; non esiste `playlist_tracks`. |

Verificare che una migrazione da una build precedente mantenga le playlist
esistenti e che `PRAGMA user_version` valga 2.

## 10. Jamendo (M7.2)

Configurare il client id solo nella sessione corrente (non scriverlo nel
repository, nei log o negli screenshot):

```powershell
$env:MUSIFY_JAMENDO_CLIENT_ID = "<client id>"
```

| Passo | Risultato atteso |
| --- | --- |
| Avviare Musify con la variabile impostata | In Fonti compare la scheda Jamendo; il log dice solo `Jamendo source enabled`, mai la chiave. |
| Aprire Fonti → Jamendo | La vista mostra i campi Brano o artista, Tag/genere, Cerca e Preferiti. |
| Cercare un brano o un artista | Arrivano risultati con titolo, artista, album, durata, copertina, etichetta CC e link Jamendo. |
| Cliccare l'etichetta CC e il link Jamendo | Il browser apre rispettivamente la licenza Creative Commons e la pagina del brano su Jamendo. |
| Premere Ascolta | L'audio parte; la barra mostra durata e stato; l'overlay SMTC mostra titolo, artista, album con licenza/Jamendo e copertina. |
| Trascinare la barra di seek | Se il server supporta Range, la riproduzione riparte dalla posizione scelta; altrimenti compare `Stream non seekable`. |
| Salvare un preferito e riavviare | Il preferito resta; la ricerca con rete assente entro 24 h mostra i risultati in cache con l'avviso offline. |
| Aggiungere un risultato alla playlist M7.1 con `+` | Il brano compare nella playlist selezionata con lo snapshot; riproducendolo funziona lo streaming OnDemand. |
| Controllare `%APPDATA%\Musify\` | Esistono solo `jamendo.sqlite3` e `jamendo-covers/` (max 64 file / 8 MB); nessun file audio scaricato. |
| Rimuovere la variabile e riavviare | La scheda Jamendo non compare; Fonti mostra `Jamendo non configurato` con la variabile da impostare. |
| Controllare i log | Nessuna occorrenza del client id, nessun URL con `client_id`, nessun runtime di rete a riposo. |

La prova manuale completa (ricerca, ascolto, seek, playlist) richiede una
sessione audio reale e la chiave dell'utente; i test automatici usano solo un
server finto e non toccano la rete.

## 11. Riposo

| Passo | Risultato atteso |
| --- | --- |
| Aprire Musify senza riprodurre e non toccare nulla per 30 s | La CPU resta a 0,0–0,3%; nessun redraw continuo. |
| Controllare il Task Manager | Nessun runtime di rete attivo a riposo: Tokio esiste solo durante una ricerca o uno stream online. |
| Ridurre a icona e ripristinare | La UI si ridisegna solo quando serve. |

## Registro delle prove

| Data | DPI | Dispositivo audio | Esito | Note |
| --- | --- | --- | --- | --- |
| | | | | |
