//! All user-facing strings. Keep this module the only place with Italian copy.

use musify_core::RepeatMode;

pub const APP_NAME: &str = "Musify";
pub const APP_TAGLINE: &str = "La tua musica, locale";

pub const NAV_LIBRARY: &str = "Raccolta";
pub const NAV_PLAYBACK: &str = "Riproduzione";
pub const NAV_TRACKS: &str = "Brani";
pub const NAV_ALBUMS: &str = "Album";
pub const NAV_ARTISTS: &str = "Artisti";
pub const NAV_QUEUE: &str = "Coda";
pub const NAV_SOURCES: &str = "Fonti";
pub const NAV_SETTINGS: &str = "Impostazioni";
pub const NAV_LOCAL: &str = "Locale";
pub const NAV_RADIO: &str = "Radio";
pub const NAV_PODCAST: &str = "Podcast";

pub const SOURCE_LOCAL: &str = "Locale";
pub const SOURCE_RADIO: &str = "Radio";
pub const SOURCE_PODCAST: &str = "Podcast";
pub const SOURCE_JAMENDO: &str = "Jamendo";
pub const SOURCES_TITLE: &str = "Fonti";
pub const SOURCES_HINT: &str = "Scegli da dove viene la musica.";
pub const SOURCE_OPEN: &str = "Apri";
pub const SOURCES_RADIO_CAPTION: &str = "Radio Browser · preferiti offline";
pub const SOURCES_PODCAST_CAPTION: &str = "iTunes Search · RSS · episodi e ripresa";
pub const SOURCES_JAMENDO_CAPTION: &str = "Generi e selezioni · streaming · Creative Commons";
pub const RADIO_TITLE: &str = "Radio internet";
pub const RADIO_HINT: &str = "Cerca stazioni con Radio Browser. I preferiti restano sul computer e funzionano anche offline.";
pub const RADIO_NAME: &str = "Nome";
pub const RADIO_COUNTRY: &str = "Paese";
pub const RADIO_TAG: &str = "Genere / tag";
pub const RADIO_SEARCH: &str = "Cerca";
pub const RADIO_FAVORITES: &str = "Preferiti";
pub const RADIO_EMPTY: &str = "Nessuna stazione trovata";
pub const RADIO_EMPTY_HINT: &str = "Prova un altro nome, paese o tag, oppure salva un preferito.";
pub const RADIO_LOADING: &str = "Ricerca stazioni…";
pub const RADIO_OFFLINE: &str = "Offline: mostro i risultati salvati";
pub const RADIO_ADD_FAVORITE: &str = "☆";
pub const RADIO_REMOVE_FAVORITE: &str = "★";
pub const RADIO_PLAY: &str = "Ascolta";

pub const PODCAST_TITLE: &str = "Podcast";
pub const PODCAST_HINT: &str = "Cerca un podcast con iTunes Search, iscriviti e ascolta gli episodi. I feed si aggiornano solo su richiesta.";
pub const PODCAST_SEARCH_PLACEHOLDER: &str = "Cerca un podcast";
pub const PODCAST_SEARCH: &str = "Cerca";
pub const PODCAST_SUBSCRIPTIONS: &str = "Iscrizioni";
pub const PODCAST_EMPTY: &str = "Nessun podcast";
pub const PODCAST_EMPTY_HINT: &str = "Cerca un podcast oppure mostra le iscrizioni salvate.";
pub const PODCAST_LOADING: &str = "Ricerca podcast…";
pub const PODCAST_SUBSCRIBE: &str = "Iscriviti";
pub const PODCAST_UNSUBSCRIBE: &str = "Annulla";
pub const PODCAST_OPEN: &str = "Apri";
pub const PODCAST_EPISODES_LOADING: &str = "Aggiornamento feed…";
pub const PODCAST_REFRESH: &str = "Aggiorna feed";
pub const PODCAST_NO_EPISODES: &str = "Nessun episodio";
pub const PODCAST_NO_EPISODES_HINT: &str = "Aggiorna il feed per scaricare l’elenco degli episodi.";
pub const PODCAST_NO_SELECTION: &str = "Scegli un podcast";
pub const PODCAST_NO_SELECTION_HINT: &str = "Apri un podcast per vedere gli episodi disponibili.";
pub const PODCAST_PLAY: &str = "Ascolta";
pub const PODCAST_MARK_WATCHED: &str = "Segna ascoltato";
pub const PODCAST_MARK_UNWATCHED: &str = "Non ascoltato";
pub const PODCAST_WATCHED: &str = "Ascoltato";
pub const PODCAST_RESUME: &str = "Riprendi a";
pub const NOT_SEEKABLE: &str = "Stream non seekable";

pub const JAMENDO_TITLE: &str = "Jamendo";
pub const JAMENDO_HINT: &str = "Scegli un genere o cerca per titolo, artista o album. Solo streaming Creative Commons, nessun download.";
pub const JAMENDO_SEARCH_PLACEHOLDER: &str = "Titolo, artista o album";
pub const JAMENDO_SEARCH: &str = "Cerca";
pub const JAMENDO_FAVORITES: &str = "Preferiti";
pub const JAMENDO_EMPTY: &str = "Nessun brano trovato";
pub const JAMENDO_EMPTY_HINT: &str =
    "Prova un altro genere, un altro ordinamento o un testo diverso.";
pub const JAMENDO_LOADING: &str = "Caricamento brani…";
pub const JAMENDO_LOADING_MORE: &str = "Caricamento altri brani…";
pub const JAMENDO_LOAD_MORE: &str = "Carica altri";
pub const JAMENDO_OFFLINE: &str = "Offline: mostro i risultati salvati";
pub const JAMENDO_PLAY: &str = "Ascolta";
pub const JAMENDO_TRACK_PAGE: &str = "Jamendo";
pub const JAMENDO_NOT_CONFIGURED: &str = "Jamendo non configurato";
pub const JAMENDO_CONFIG_HINT: &str = "Imposta la variabile d'ambiente MUSIFY_JAMENDO_CLIENT_ID con il client id del tuo account sviluppatore Jamendo e riavvia Musify.";
pub const JAMENDO_SORT_WEEK: &str = "Popolari settimana";
pub const JAMENDO_SORT_MONTH: &str = "Popolari mese";
pub const JAMENDO_SORT_ALL: &str = "Sempre popolari";
pub const JAMENDO_SORT_NEWEST: &str = "Novità";
pub const JAMENDO_SELECTIONS: &str = "Selezioni Jamendo";
pub const JAMENDO_SELECTIONS_LOADING: &str = "Carico le selezioni…";
pub const JAMENDO_REFRESH_SELECTIONS: &str = "Aggiorna selezioni";
pub const JAMENDO_BACK_TO_GENRES: &str = "Torna ai generi";
pub const JAMENDO_SAVE_PLAYLIST: &str = "Salva come playlist";
pub const JAMENDO_SHUFFLE: &str = "Mescola";
pub const JAMENDO_ENQUEUE_ALL: &str = "Aggiungi tutto alla coda";
pub const JAMENDO_KEYBOARD_HINT: &str = "Alt+←/→ cambia genere · Enter riproduce il brano selezionato · Ctrl+Enter riproduce tutto · Alt+Enter mescola";

/// API tag and Italian label for the home genre grid. The tag is what Jamendo
/// accepts in `tags`; the label is the only user-facing copy.
pub const JAMENDO_GENRES: &[(&str, &str)] = &[
    ("jazz", "Jazz"),
    ("rock", "Rock"),
    ("electronic", "Elettronica"),
    ("ambient", "Ambient"),
    ("pop", "Pop"),
    ("hiphop", "Hip hop"),
    ("classical", "Classica"),
    ("lounge", "Lounge"),
    ("folk", "Folk"),
    ("world", "World"),
    ("metal", "Metal"),
    ("soundtrack", "Colonne sonore"),
];

pub fn jamendo_genre_label(tag: &str) -> String {
    JAMENDO_GENRES
        .iter()
        .find(|(candidate, _)| *candidate == tag)
        .map_or_else(|| tag.to_string(), |(_, label)| (*label).to_string())
}

pub fn jamendo_latency(ms: u128, cached: bool) -> String {
    if cached {
        format!("Da cache · {ms} ms")
    } else {
        format!("Risultati in {ms} ms")
    }
}

pub fn jamendo_playing(count: usize) -> String {
    match count {
        1 => "Riproduco 1 brano.".into(),
        n => format!("Riproduco {n} brani."),
    }
}

pub fn jamendo_enqueued(count: usize) -> String {
    match count {
        1 => "1 brano aggiunto alla coda.".into(),
        n => format!("{n} brani aggiunti alla coda."),
    }
}

pub fn jamendo_playlist_name(genre: &str, sort: &str) -> String {
    let genre = jamendo_genre_label(genre);
    format!("Jamendo · {genre} · {sort}")
}

pub fn jamendo_playlist_name_selection(selection: &str) -> String {
    format!("Jamendo · {selection}")
}

pub fn jamendo_playlist_saved(name: &str) -> String {
    format!("Playlist salvata: {name}")
}

pub const NAV_PLAYLISTS: &str = "Playlist";
pub const PLAYLISTS_TITLE: &str = "Playlist";
pub const PLAYLISTS_HINT: &str = "Crea raccolte, riordina i brani e importa o esporta file M3U8.";
pub const PLAYLIST_NAME: &str = "Nome playlist";
pub const PLAYLIST_CREATE: &str = "Crea";
pub const PLAYLIST_RENAME: &str = "Rinomina";
pub const PLAYLIST_DELETE: &str = "Elimina";
pub const PLAYLIST_PLAY: &str = "Riproduci playlist";
pub const PLAYLIST_ENQUEUE: &str = "Aggiungi alla coda";
pub const PLAYLIST_SAVE_QUEUE: &str = "Salva coda come playlist";
pub const PLAYLIST_IMPORT: &str = "Importa M3U8";
pub const PLAYLIST_EXPORT: &str = "Esporta M3U8";
pub const PLAYLIST_ADD_CURRENT: &str = "Aggiungi brano corrente";
pub const PLAYLIST_ADD_QUEUE: &str = "Aggiungi coda";
pub const PLAYLIST_ADD: &str = "+";
pub const PLAYLIST_REMOVE_MISSING: &str = "Rimuovi mancanti";
pub const PLAYLIST_EMPTY: &str = "Nessuna playlist";
pub const PLAYLIST_EMPTY_HINT: &str = "Crea una playlist o importa un file M3U8.";
pub const PLAYLIST_SELECT: &str = "Scegli una playlist";
pub const PLAYLIST_SELECT_HINT: &str = "Seleziona una playlist per vedere e gestire i brani.";
pub const PLAYLIST_NO_ITEMS: &str = "Playlist vuota";
pub const PLAYLIST_NO_ITEMS_HINT: &str =
    "Aggiungi brani dalla libreria, dalla ricerca, dalla coda o dal brano corrente.";
pub const PLAYLIST_MISSING: &str = "File non trovato";
pub const PLAYLIST_MOVE_UP: &str = "↑";
pub const PLAYLIST_MOVE_DOWN: &str = "↓";
pub const PLAYLIST_REMOVE_ITEM: &str = "×";
pub const PLAYLIST_REORDER_HINT: &str =
    "Seleziona un brano e usa Alt+↑/↓ oppure i pulsanti Su/Giù.";
pub const PLAYLIST_NAME_REQUIRED: &str = "Inserisci un nome per la playlist.";
pub const PLAYLIST_CREATED: &str = "Playlist creata.";
pub const PLAYLIST_RENAMED: &str = "Playlist rinominata.";
pub const PLAYLIST_DELETED: &str = "Playlist eliminata.";
pub const PLAYLIST_IMPORTED: &str = "Playlist importata.";
pub const PLAYLIST_EXPORTED: &str = "Playlist esportata.";
pub const PLAYLIST_SAVED_QUEUE: &str = "Coda salvata come playlist.";
pub const PLAYLIST_NO_CURRENT: &str = "Nessun brano in riproduzione.";
pub const PLAYLIST_NO_TRACKS: &str =
    "Nessun brano aggiungibile: radio e podcast non sono supportati nelle playlist.";

pub const SEARCH_PLACEHOLDER: &str = "Cerca brani, album, artisti";
pub const SEARCH_EMPTY: &str = "Nessun risultato";
pub const SEARCH_HINT: &str = "Cerca un brano, un album o un artista";

pub const PLAY: &str = "Riproduci";
pub const PAUSE: &str = "Pausa";
pub const RESUME: &str = "Riprendi";
pub const PREVIOUS: &str = "Prec";
pub const NEXT: &str = "Succ";
pub const SHUFFLE: &str = "Shuffle";
pub const VOLUME: &str = "Volume";
pub const QUEUE: &str = "Coda";
pub const NOW_PLAYING: &str = "In riproduzione";
pub const STOPPED: &str = "Fermo";
pub const PAUSED: &str = "In pausa";
pub const NO_TRACK: &str = "Nessun brano";
pub const NO_TRACK_HINT: &str = "Scegli un brano dalla libreria";

pub const PLAY_ALL: &str = "Riproduci tutto";
pub const ADD_FOLDER: &str = "Aggiungi cartella";
pub const REMOVE: &str = "Rimuovi";
pub const RESCAN: &str = "Riscansiona";
pub const CLEAR_QUEUE: &str = "Svuota coda";
pub const BACK: &str = "Indietro";
pub const UP: &str = "Su";
pub const DOWN: &str = "Giù";
pub const OPEN_SETTINGS: &str = "Apri impostazioni";
pub const CLOSE: &str = "Chiudi";

pub const TRACKS_TITLE: &str = "Brani";
pub const ALBUMS_TITLE: &str = "Album";
pub const ARTISTS_TITLE: &str = "Artisti";
pub const QUEUE_TITLE: &str = "Coda di riproduzione";
pub const SETTINGS_TITLE: &str = "Impostazioni";

pub const COL_TITLE: &str = "Titolo";
pub const COL_ALBUM: &str = "Album";
pub const COL_DURATION: &str = "Durata";

pub const EMPTY_LIBRARY: &str = "La libreria è vuota";
pub const EMPTY_LIBRARY_HINT: &str =
    "Aggiungi una cartella musicale per iniziare. La scansione parte in automatico.";
pub const EMPTY_QUEUE: &str = "La coda è vuota";
pub const EMPTY_QUEUE_HINT: &str = "Scegli un brano dalla libreria o riproduci un album.";
pub const EMPTY_FILTER: &str = "Nessun brano in questa raccolta";
pub const EMPTY_FOLDERS: &str = "Nessuna cartella musicale configurata";
pub const FOLDERS_TITLE: &str = "Cartelle musicali";
pub const FOLDERS_HINT: &str =
    "Musify osserva queste cartelle e aggiorna la libreria quando cambiano.";
pub const AUDIO_TITLE: &str = "Uscita audio";
pub const AUDIO_UNAVAILABLE: &str = "Nessun dispositivo audio disponibile";
pub const LIBRARY_TITLE: &str = "Libreria";
pub const SHORTCUTS_TITLE: &str = "Scorciatoie";
pub const SHORTCUTS: &str = "Spazio: play/pausa · ←/→: cerca ±5 s · Ctrl+←/→: brano precedente/successivo · Alt+↑/↓: riordina playlist · Ctrl+F: cerca · Ctrl+R: riscansiona · Esc: chiudi ricerca · Jamendo: Alt+←/→ genere, Enter brano, Ctrl+Enter tutto, Alt+Enter mescola";
pub const SCAN_RUNNING: &str = "Scansione in corso";
pub const SCAN_IDLE: &str = "Libreria aggiornata";

pub const UNKNOWN_ARTIST: &str = "Artista sconosciuto";
pub const UNKNOWN_ALBUM: &str = "Album sconosciuto";
pub const UNKNOWN_TITLE: &str = "Senza titolo";

pub fn tracks_count(count: usize) -> String {
    match count {
        1 => "1 brano".into(),
        n => format!("{n} brani"),
    }
}

pub fn albums_count(count: usize) -> String {
    match count {
        1 => "1 album".into(),
        n => format!("{n} album"),
    }
}

pub fn artists_count(count: usize) -> String {
    match count {
        1 => "1 artista".into(),
        n => format!("{n} artisti"),
    }
}

pub fn queue_count(count: usize) -> String {
    match count {
        1 => "1 brano in coda".into(),
        n => format!("{n} brani in coda"),
    }
}

pub fn playlists_count(count: usize) -> String {
    match count {
        1 => "1 playlist".into(),
        n => format!("{n} playlist"),
    }
}

pub fn playlist_items_count(count: usize) -> String {
    match count {
        1 => "1 brano".into(),
        n => format!("{n} brani"),
    }
}

pub fn playlist_added(count: usize) -> String {
    match count {
        1 => "1 brano aggiunto alla playlist.".into(),
        n => format!("{n} brani aggiunti alla playlist."),
    }
}

pub fn playlist_missing_removed(count: usize) -> String {
    match count {
        0 => "Nessun file mancante da rimuovere.".into(),
        1 => "1 elemento mancante rimosso.".into(),
        n => format!("{n} elementi mancanti rimossi."),
    }
}

pub fn scan_progress(scanned: u64, total: u64) -> String {
    if total == 0 {
        SCAN_RUNNING.into()
    } else {
        format!("{scanned} / {total}")
    }
}

pub fn scan_finished(added: u64, updated: u64, removed: u64) -> String {
    format!("+{added} nuovi · {updated} aggiornati · {removed} rimossi")
}

pub fn results_for(query: &str) -> String {
    format!("Risultati per «{query}»")
}

pub fn album_meta(artist: &str, year: Option<i32>, track_count: u32) -> String {
    let artist = if artist.is_empty() {
        UNKNOWN_ARTIST
    } else {
        artist
    };
    match year {
        Some(year) => format!("{artist} · {year} · {track_count} brani"),
        None => format!("{artist} · {track_count} brani"),
    }
}

pub fn album_meta_short(year: Option<i32>, track_count: u32) -> String {
    match year {
        Some(year) => format!("{year} · {track_count} brani"),
        None => format!("{track_count} brani"),
    }
}

pub fn artist_meta(album_count: u32, track_count: u32) -> String {
    format!("{album_count} album · {track_count} brani")
}

pub fn duration_text(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub fn duration_long(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    let hours = seconds / 3600;
    let minutes = (seconds / 60) % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m {:02}s", seconds % 60)
    }
}

pub fn error_prefix(context: &str) -> String {
    format!("Errore ({context})")
}

pub fn repeat_label(repeat: RepeatMode) -> &'static str {
    match repeat {
        RepeatMode::Off => "Repeat off",
        RepeatMode::All => "Repeat all",
        RepeatMode::One => "Repeat one",
    }
}

pub fn source_label(scheme: &str) -> &'static str {
    match scheme {
        "local" => SOURCE_LOCAL,
        "radio" => SOURCE_RADIO,
        "podcast" => SOURCE_PODCAST,
        "jamendo" => SOURCE_JAMENDO,
        _ => "Sorgente",
    }
}

pub fn stream_status_label(status: &musify_core::StreamStatus) -> String {
    use musify_core::StreamStatus;
    match status {
        StreamStatus::Idle => String::new(),
        StreamStatus::Connecting => "Connessione…".into(),
        StreamStatus::Buffering => "Buffering…".into(),
        StreamStatus::Playing => NOW_PLAYING.into(),
        StreamStatus::Reconnecting { attempt } => {
            format!("Riconnessione… ({attempt})")
        }
        StreamStatus::Offline => "Offline".into(),
        StreamStatus::Error(message) => format!("Errore stream: {message}"),
    }
}

pub fn state_label(state: musify_core::PlaybackState) -> &'static str {
    match state {
        musify_core::PlaybackState::Playing => NOW_PLAYING,
        musify_core::PlaybackState::Paused => PAUSED,
        musify_core::PlaybackState::Stopped => STOPPED,
    }
}
