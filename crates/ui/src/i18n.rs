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
pub const SOURCES_TITLE: &str = "Fonti";
pub const SOURCES_HINT: &str = "Scegli da dove viene la musica.";
pub const SOURCE_OPEN: &str = "Apri";
pub const SOURCES_RADIO_CAPTION: &str = "Radio Browser · preferiti offline";
pub const SOURCES_PODCAST_CAPTION: &str = "iTunes Search · RSS · episodi e ripresa";
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
pub const SHORTCUTS: &str = "Spazio: play/pausa · ←/→: cerca ±5 s · Ctrl+←/→: brano precedente/successivo · Ctrl+F: cerca · Ctrl+R: riscansiona · Esc: chiudi ricerca";
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
