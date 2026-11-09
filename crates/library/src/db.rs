use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use musify_core::{
    Album, AlbumId, Artist, ArtistId, LibraryError as CoreLibraryError, LibraryProvider,
    LibraryStats, SearchResults, SortDirection, Track, TrackId, TrackSort,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::{LibraryError, ScanProgress, ScanReport, scanner};

const TRACK_COLUMNS: &str = "t.id, t.path, t.title, t.artist, t.album, t.album_artist, \
     t.track_number, t.disc_number, t.duration_ms, t.year, t.genre, t.cover_path, t.added_at";

pub struct Library {
    db_path: PathBuf,
    cover_dir: PathBuf,
    connection: Mutex<Connection>,
    fts: bool,
}

impl Library {
    pub fn open(
        db_path: impl AsRef<Path>,
        cover_dir: impl AsRef<Path>,
    ) -> Result<Self, LibraryError> {
        let db_path = db_path.as_ref().to_path_buf();
        let cover_dir = cover_dir.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&cover_dir)?;
        let connection = Connection::open(&db_path)?;
        configure_connection(&connection)?;
        let fts = init_schema(&connection)?;
        Ok(Self {
            db_path,
            cover_dir,
            connection: Mutex::new(connection),
            fts,
        })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn cover_dir(&self) -> &Path {
        &self.cover_dir
    }

    pub fn uses_fts(&self) -> bool {
        self.fts
    }

    pub fn scan(&self, folders: &[PathBuf]) -> Result<ScanReport, LibraryError> {
        self.scan_with_progress(folders, |_| {})
    }

    pub fn scan_with_progress(
        &self,
        folders: &[PathBuf],
        progress: impl FnMut(ScanProgress),
    ) -> Result<ScanReport, LibraryError> {
        scanner::scan(&self.db_path, &self.cover_dir, folders, progress)
    }

    pub fn stats(&self) -> Result<LibraryStats, LibraryError> {
        let connection = self.lock()?;
        let (tracks, albums, artists, duration_ms): (i64, i64, i64, i64) = connection.query_row(
            "SELECT (SELECT COUNT(*) FROM tracks), \
                    (SELECT COUNT(*) FROM albums), \
                    (SELECT COUNT(*) FROM artists), \
                    (SELECT COALESCE(SUM(duration_ms), 0) FROM tracks)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        Ok(LibraryStats {
            tracks: tracks.max(0) as u64,
            albums: albums.max(0) as u64,
            artists: artists.max(0) as u64,
            total_duration: Duration::from_millis(duration_ms.max(0) as u64),
        })
    }

    pub fn tracks_sorted(
        &self,
        sort: TrackSort,
        direction: SortDirection,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Track>, LibraryError> {
        let connection = self.lock()?;
        let order = track_order(sort, direction);
        let sql =
            format!("SELECT {TRACK_COLUMNS} FROM tracks t ORDER BY {order} LIMIT ?1 OFFSET ?2");
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params![limit as i64, offset as i64], track_from_row)?;
        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(row?);
        }
        Ok(tracks)
    }

    pub fn tracks_for_album(
        &self,
        album_id: AlbumId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Track>, LibraryError> {
        let connection = self.lock()?;
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks t WHERE t.album_id = ?1 \
             ORDER BY t.disc_number ASC, t.track_number ASC, t.title COLLATE NOCASE ASC \
             LIMIT ?2 OFFSET ?3"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![album_id.get(), limit as i64, offset as i64],
            track_from_row,
        )?;
        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(row?);
        }
        Ok(tracks)
    }

    pub fn tracks_for_artist(
        &self,
        artist_id: ArtistId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Track>, LibraryError> {
        let connection = self.lock()?;
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks t WHERE t.artist_id = ?1 \
             ORDER BY t.album COLLATE NOCASE ASC, t.disc_number ASC, t.track_number ASC, \
                      t.title COLLATE NOCASE ASC LIMIT ?2 OFFSET ?3"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![artist_id.get(), limit as i64, offset as i64],
            track_from_row,
        )?;
        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(row?);
        }
        Ok(tracks)
    }

    pub fn track_by_path(&self, path: &Path) -> Result<Option<Track>, LibraryError> {
        let connection = self.lock()?;
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks t WHERE t.path = ?1");
        Ok(connection
            .query_row(&sql, params![path.to_string_lossy()], track_from_row)
            .optional()?)
    }

    pub fn albums_page(&self, limit: usize, offset: usize) -> Result<Vec<Album>, LibraryError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT a.id, a.title, a.artist, a.year, a.cover_path, COUNT(t.id) AS track_count \
             FROM albums a LEFT JOIN tracks t ON t.album_id = a.id \
             GROUP BY a.id ORDER BY a.title COLLATE NOCASE, a.artist COLLATE NOCASE \
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = statement.query_map(params![limit as i64, offset as i64], album_from_row)?;
        let mut albums = Vec::new();
        for row in rows {
            albums.push(row?);
        }
        Ok(albums)
    }

    pub fn artists_page(&self, limit: usize, offset: usize) -> Result<Vec<Artist>, LibraryError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT ar.id, ar.name, \
                    COUNT(DISTINCT t.album_id) AS album_count, \
                    COUNT(t.id) AS track_count \
             FROM artists ar LEFT JOIN tracks t ON t.artist_id = ar.id \
             GROUP BY ar.id ORDER BY ar.name COLLATE NOCASE LIMIT ?1 OFFSET ?2",
        )?;
        let rows = statement.query_map(params![limit as i64, offset as i64], artist_from_row)?;
        let mut artists = Vec::new();
        for row in rows {
            artists.push(row?);
        }
        Ok(artists)
    }

    pub fn search_all(&self, query: &str, limit: usize) -> Result<SearchResults, LibraryError> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Ok(SearchResults::default());
        }
        let connection = self.lock()?;
        let tracks = if self.fts {
            self.search_tracks_fts(&connection, query, limit)?
        } else {
            search_tracks_like(&connection, query, limit)?
        };
        let albums = search_albums(&connection, query, limit)?;
        let artists = search_artists(&connection, query, limit)?;
        Ok(SearchResults {
            tracks,
            albums,
            artists,
        })
    }

    fn search_tracks_fts(
        &self,
        connection: &Connection,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Track>, LibraryError> {
        let Some(query) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks_fts f \
             JOIN tracks t ON t.id = f.rowid \
             WHERE tracks_fts MATCH ?1 ORDER BY rank LIMIT ?2"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params![query, limit as i64], track_from_row)?;
        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(row?);
        }
        Ok(tracks)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, LibraryError> {
        self.connection
            .lock()
            .map_err(|_| LibraryError::Database("database lock poisoned".into()))
    }
}

impl LibraryProvider for Library {
    fn tracks(
        &self,
        sort: TrackSort,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Track>, CoreLibraryError> {
        self.tracks_sorted(sort, SortDirection::Ascending, limit, offset)
            .map_err(core_error)
    }

    fn albums(&self, limit: usize, offset: usize) -> Result<Vec<Album>, CoreLibraryError> {
        self.albums_page(limit, offset).map_err(core_error)
    }

    fn artists(&self, limit: usize, offset: usize) -> Result<Vec<Artist>, CoreLibraryError> {
        self.artists_page(limit, offset).map_err(core_error)
    }

    fn search(&self, query: &str, limit: usize) -> Result<SearchResults, CoreLibraryError> {
        self.search_all(query, limit).map_err(core_error)
    }
}

fn core_error(error: LibraryError) -> CoreLibraryError {
    match error {
        LibraryError::Database(message) => CoreLibraryError::Database(message),
        LibraryError::Io(error) => CoreLibraryError::Io(error.to_string()),
        LibraryError::Cover(message) => CoreLibraryError::Query(message),
        LibraryError::Path(message) => CoreLibraryError::Query(message),
        LibraryError::Watcher(message) => CoreLibraryError::Io(message),
    }
}

pub(crate) fn configure_connection(connection: &Connection) -> Result<(), LibraryError> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = NORMAL;\
         PRAGMA foreign_keys = ON;\
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

pub(crate) fn init_schema(connection: &Connection) -> Result<bool, LibraryError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS artists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            year INTEGER,
            cover_path TEXT,
            UNIQUE(title, artist)
        );
        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            album TEXT NOT NULL,
            album_artist TEXT,
            artist_id INTEGER NOT NULL REFERENCES artists(id),
            album_id INTEGER NOT NULL REFERENCES albums(id),
            track_number INTEGER,
            disc_number INTEGER,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            year INTEGER,
            genre TEXT,
            cover_path TEXT,
            mtime_ns INTEGER NOT NULL,
            file_size INTEGER NOT NULL,
            added_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS tracks_album_idx ON tracks(album_id, disc_number, track_number);
        CREATE INDEX IF NOT EXISTS tracks_artist_idx ON tracks(artist_id);
        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS playlist_tracks (
            playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            PRIMARY KEY (playlist_id, position)
        );
        "#,
    )?;

    let had_fts: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'tracks_fts')",
        [],
        |row| row.get(0),
    )?;
    let had_triggers: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'trigger' AND name = 'tracks_ai')",
        [],
        |row| row.get(0),
    )?;
    let fts = match connection.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
            title, artist, album,
            content = 'tracks', content_rowid = 'id',
            tokenize = "unicode61 remove_diacritics 2"
        );
        CREATE TRIGGER IF NOT EXISTS tracks_ai AFTER INSERT ON tracks BEGIN
            INSERT INTO tracks_fts(rowid, title, artist, album)
            VALUES (new.id, new.title, new.artist, new.album);
        END;
        CREATE TRIGGER IF NOT EXISTS tracks_ad AFTER DELETE ON tracks BEGIN
            INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album)
            VALUES ('delete', old.id, old.title, old.artist, old.album);
        END;
        CREATE TRIGGER IF NOT EXISTS tracks_au AFTER UPDATE ON tracks BEGIN
            INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album)
            VALUES ('delete', old.id, old.title, old.artist, old.album);
            INSERT INTO tracks_fts(rowid, title, artist, album)
            VALUES (new.id, new.title, new.artist, new.album);
        END;
        "#,
    ) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(%error, "FTS5 unavailable; search will use LIKE");
            false
        }
    };
    if fts && (!had_fts || !had_triggers) {
        connection.execute("INSERT INTO tracks_fts(tracks_fts) VALUES ('rebuild')", [])?;
    }
    Ok(fts)
}

fn track_order(sort: TrackSort, direction: SortDirection) -> &'static str {
    let descending = matches!(direction, SortDirection::Descending);
    match (sort, descending) {
        (TrackSort::Title, false) => "t.title COLLATE NOCASE ASC, t.artist COLLATE NOCASE ASC",
        (TrackSort::Title, true) => "t.title COLLATE NOCASE DESC, t.artist COLLATE NOCASE DESC",
        (TrackSort::Artist, false) => {
            "t.artist COLLATE NOCASE ASC, t.album COLLATE NOCASE ASC, t.disc_number ASC, t.track_number ASC"
        }
        (TrackSort::Artist, true) => {
            "t.artist COLLATE NOCASE DESC, t.album COLLATE NOCASE DESC, t.disc_number DESC, t.track_number DESC"
        }
        (TrackSort::Album, false) => {
            "t.album COLLATE NOCASE ASC, t.disc_number ASC, t.track_number ASC, t.title COLLATE NOCASE ASC"
        }
        (TrackSort::Album, true) => {
            "t.album COLLATE NOCASE DESC, t.disc_number DESC, t.track_number DESC, t.title COLLATE NOCASE DESC"
        }
        (TrackSort::AddedAt, false) => "t.added_at ASC, t.id ASC",
        (TrackSort::AddedAt, true) => "t.added_at DESC, t.id DESC",
        (TrackSort::Duration, false) => "t.duration_ms ASC, t.title COLLATE NOCASE ASC",
        (TrackSort::Duration, true) => "t.duration_ms DESC, t.title COLLATE NOCASE DESC",
    }
}

fn track_from_row(row: &Row<'_>) -> rusqlite::Result<Track> {
    let duration_ms: i64 = row.get("duration_ms")?;
    let track_number: Option<i64> = row.get("track_number")?;
    let disc_number: Option<i64> = row.get("disc_number")?;
    let year: Option<i64> = row.get("year")?;
    let cover: Option<String> = row.get("cover_path")?;
    let path = PathBuf::from(row.get::<_, String>("path")?);
    Ok(Track {
        id: TrackId::new(row.get("id")?),
        source: musify_core::SourceId::local(&path),
        path,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        album_artist: row.get("album_artist")?,
        track_number: track_number.and_then(|value| u32::try_from(value).ok()),
        disc_number: disc_number.and_then(|value| u32::try_from(value).ok()),
        duration: Duration::from_millis(duration_ms.max(0) as u64),
        year: year.and_then(|value| i32::try_from(value).ok()),
        genre: row.get("genre")?,
        cover_path: cover.map(PathBuf::from),
        added_at: row.get("added_at")?,
    })
}

fn album_from_row(row: &Row<'_>) -> rusqlite::Result<Album> {
    let year: Option<i64> = row.get("year")?;
    let cover: Option<String> = row.get("cover_path")?;
    let count: i64 = row.get("track_count")?;
    Ok(Album {
        id: AlbumId::new(row.get("id")?),
        title: row.get("title")?,
        artist: row.get("artist")?,
        year: year.and_then(|value| i32::try_from(value).ok()),
        cover_path: cover.map(PathBuf::from),
        track_count: count.max(0) as u32,
    })
}

fn artist_from_row(row: &Row<'_>) -> rusqlite::Result<Artist> {
    let albums: i64 = row.get("album_count")?;
    let tracks: i64 = row.get("track_count")?;
    Ok(Artist {
        id: ArtistId::new(row.get("id")?),
        name: row.get("name")?,
        album_count: albums.max(0) as u32,
        track_count: tracks.max(0) as u32,
    })
}

fn search_tracks_like(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<Track>, LibraryError> {
    let pattern = like_pattern(query);
    let sql = format!(
        "SELECT {TRACK_COLUMNS} FROM tracks t \
         WHERE t.title LIKE ?1 ESCAPE '\\' OR t.artist LIKE ?1 ESCAPE '\\' OR t.album LIKE ?1 ESCAPE '\\' \
         ORDER BY t.title COLLATE NOCASE LIMIT ?2"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![pattern, limit as i64], track_from_row)?;
    let mut tracks = Vec::new();
    for row in rows {
        tracks.push(row?);
    }
    Ok(tracks)
}

fn search_albums(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<Album>, LibraryError> {
    let pattern = like_pattern(query);
    let mut statement = connection.prepare(
        "SELECT a.id, a.title, a.artist, a.year, a.cover_path, COUNT(t.id) AS track_count \
         FROM albums a LEFT JOIN tracks t ON t.album_id = a.id \
         WHERE a.title LIKE ?1 ESCAPE '\\' OR a.artist LIKE ?1 ESCAPE '\\' \
         GROUP BY a.id ORDER BY a.title COLLATE NOCASE LIMIT ?2",
    )?;
    let rows = statement.query_map(params![pattern, limit as i64], album_from_row)?;
    let mut albums = Vec::new();
    for row in rows {
        albums.push(row?);
    }
    Ok(albums)
}

fn search_artists(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<Artist>, LibraryError> {
    let pattern = like_pattern(query);
    let mut statement = connection.prepare(
        "SELECT ar.id, ar.name, \
                COUNT(DISTINCT t.album_id) AS album_count, \
                COUNT(t.id) AS track_count \
         FROM artists ar LEFT JOIN tracks t ON t.artist_id = ar.id \
         WHERE ar.name LIKE ?1 ESCAPE '\\' \
         GROUP BY ar.id ORDER BY ar.name COLLATE NOCASE LIMIT ?2",
    )?;
    let rows = statement.query_map(params![pattern, limit as i64], artist_from_row)?;
    let mut artists = Vec::new();
    for row in rows {
        artists.push(row?);
    }
    Ok(artists)
}

fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn fts_query(query: &str) -> Option<String> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .filter_map(|token| {
            let cleaned: String = token
                .chars()
                .filter(|character| character.is_alphanumeric())
                .collect();
            (!cleaned.is_empty()).then(|| format!("\"{cleaned}\"*"))
        })
        .collect();
    (!tokens.is_empty()).then(|| tokens.join(" AND "))
}

pub(crate) fn find_track_stamp(
    connection: &Connection,
    path: &str,
) -> Result<Option<(i64, i64)>, LibraryError> {
    Ok(connection
        .query_row(
            "SELECT mtime_ns, file_size FROM tracks WHERE path = ?1",
            params![path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temporary_library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().expect("temp dir");
        let library = Library::open(
            dir.path().join("library.sqlite3"),
            dir.path().join("covers"),
        )
        .expect("open library");
        (dir, library)
    }

    fn sample_track(path: &str) -> Track {
        Track {
            id: TrackId::new(1),
            source: musify_core::SourceId::local(path),
            path: PathBuf::from(path),
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            album_artist: Some("Artist".into()),
            track_number: Some(1),
            disc_number: Some(1),
            duration: Duration::from_secs(3),
            year: Some(2026),
            genre: Some("Test".into()),
            cover_path: None,
            added_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
        }
    }

    #[test]
    fn schema_is_idempotent_and_stats_start_at_zero() {
        let (_dir, library) = temporary_library();
        let stats = library.stats().expect("stats");
        assert_eq!(stats.tracks, 0);
        assert_eq!(stats.albums, 0);
        assert_eq!(stats.artists, 0);
    }

    #[test]
    fn empty_search_returns_empty_results() {
        let (_dir, library) = temporary_library();
        let results = library.search_all("  ", 20).expect("search");
        assert!(results.tracks.is_empty());
        assert!(results.albums.is_empty());
        assert!(results.artists.is_empty());
    }

    #[test]
    fn fts_query_sanitizes_punctuation() {
        assert_eq!(
            fts_query("città dei balocchi"),
            Some("\"città\"* AND \"dei\"* AND \"balocchi\"*".into())
        );
        assert_eq!(fts_query("!!!"), None);
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("100%_real"), "%100\\%\\_real%");
    }

    #[test]
    fn sample_track_round_trips_through_the_database() {
        let (_dir, library) = temporary_library();
        {
            let connection = library.lock().expect("lock");
            connection
                .execute("INSERT INTO artists(name) VALUES ('Artist')", [])
                .expect("artist");
            let artist_id: i64 = connection
                .query_row("SELECT id FROM artists WHERE name = 'Artist'", [], |row| {
                    row.get(0)
                })
                .expect("artist id");
            connection
                .execute(
                    "INSERT INTO albums(title, artist, year) VALUES ('Album', 'Artist', 2026)",
                    [],
                )
                .expect("album");
            let album_id: i64 = connection
                .query_row("SELECT id FROM albums WHERE title = 'Album'", [], |row| {
                    row.get(0)
                })
                .expect("album id");
            connection
                .execute(
                    "INSERT INTO tracks(path, title, artist, album, album_artist, artist_id, album_id, \
                     track_number, disc_number, duration_ms, year, genre, mtime_ns, file_size, added_at, updated_at) \
                     VALUES (?1, 'Song', 'Artist', 'Album', 'Artist', ?2, ?3, 1, 1, 3000, 2026, 'Test', 0, 0, 0, 0)",
                    params![sample_track("song.flac").path.to_string_lossy(), artist_id, album_id],
                )
                .expect("track");
        }
        let tracks = library
            .tracks_sorted(TrackSort::Title, SortDirection::Ascending, 10, 0)
            .expect("tracks");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Song");
        assert_eq!(tracks[0].duration, Duration::from_secs(3));
        let results = library.search_all("Song", 10).expect("search");
        assert_eq!(results.tracks.len(), 1);
    }

    #[test]
    fn fts_search_is_diacritic_insensitive() {
        let (_dir, library) = temporary_library();
        assert!(library.uses_fts(), "FTS5 is not available");
        {
            let connection = library.lock().expect("lock");
            connection
                .execute("INSERT INTO artists(name) VALUES ('Artista')", [])
                .expect("artist");
            let artist_id: i64 = connection
                .query_row("SELECT id FROM artists WHERE name = 'Artista'", [], |row| {
                    row.get(0)
                })
                .expect("artist id");
            connection
                .execute(
                    "INSERT INTO albums(title, artist) VALUES ('Album', 'Artista')",
                    [],
                )
                .expect("album");
            let album_id: i64 = connection
                .query_row("SELECT id FROM albums WHERE title = 'Album'", [], |row| {
                    row.get(0)
                })
                .expect("album id");
            connection
                .execute(
                    "INSERT INTO tracks(path, title, artist, album, album_artist, artist_id, album_id, \
                     duration_ms, mtime_ns, file_size, added_at, updated_at) \
                     VALUES ('citta.mp3', 'Città di notte', 'Artista', 'Album', 'Artista', ?1, ?2, 1000, 0, 0, 0, 0)",
                    params![artist_id, album_id],
                )
                .expect("track");
        }
        let results = library.search_all("citta", 10).expect("search");
        assert_eq!(results.tracks.len(), 1);
        assert_eq!(results.tracks[0].title, "Città di notte");
    }
}
