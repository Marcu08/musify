use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use musify_core::{Playlist, PlaylistId, PlaylistItem, PlaylistItemId, SourceId, Track, TrackId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{Library, LibraryError};

const PLAYLIST_COLUMNS: &str = "p.id, p.name, p.created_at, p.updated_at, \
    (SELECT COUNT(*) FROM playlist_items i WHERE i.playlist_id = p.id) AS item_count";
const ITEM_COLUMNS: &str = "id, playlist_id, position, source_scheme, item_id, title, artist, \
    album, duration_ms, cover_path";

#[derive(Debug)]
struct NewItem {
    source_scheme: String,
    item_id: String,
    title: String,
    artist: String,
    album: String,
    duration: Duration,
    cover_path: Option<String>,
}

impl Library {
    pub fn playlists(&self) -> Result<Vec<Playlist>, LibraryError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(&format!(
            "SELECT {PLAYLIST_COLUMNS} FROM playlists p ORDER BY p.name COLLATE NOCASE"
        ))?;
        let rows = statement
            .query_map([], playlist_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn playlist(&self, id: PlaylistId) -> Result<Option<Playlist>, LibraryError> {
        let connection = self.lock()?;
        Ok(connection
            .query_row(
                &format!("SELECT {PLAYLIST_COLUMNS} FROM playlists p WHERE p.id = ?1"),
                params![id.get()],
                playlist_from_row,
            )
            .optional()?)
    }

    pub fn create_playlist(&self, name: &str) -> Result<PlaylistId, LibraryError> {
        let name = clean_name(name)?;
        let connection = self.lock()?;
        let now = now();
        connection.execute(
            "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![name, now],
        )?;
        Ok(PlaylistId::new(connection.last_insert_rowid()))
    }

    pub fn rename_playlist(&self, id: PlaylistId, name: &str) -> Result<(), LibraryError> {
        let name = clean_name(name)?;
        let connection = self.lock()?;
        let changed = connection.execute(
            "UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.get(), name, now()],
        )?;
        if changed == 0 {
            return Err(LibraryError::Query("playlist not found".into()));
        }
        Ok(())
    }

    pub fn delete_playlist(&self, id: PlaylistId) -> Result<(), LibraryError> {
        let connection = self.lock()?;
        connection.execute("DELETE FROM playlists WHERE id = ?1", params![id.get()])?;
        Ok(())
    }

    pub fn playlist_items(&self, id: PlaylistId) -> Result<Vec<PlaylistItem>, LibraryError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(&format!(
            "SELECT {ITEM_COLUMNS} FROM playlist_items WHERE playlist_id = ?1 \
             ORDER BY position ASC"
        ))?;
        let rows = statement
            .query_map(params![id.get()], item_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn add_tracks_to_playlist(
        &self,
        playlist_id: PlaylistId,
        tracks: &[Track],
    ) -> Result<usize, LibraryError> {
        if tracks.is_empty() {
            return Ok(0);
        }
        let mut connection = self.lock()?;
        ensure_playlist(&connection, playlist_id)?;
        let tracks: Vec<&Track> = tracks
            .iter()
            .filter(|track| !matches!(track.source.scheme(), "radio" | "podcast"))
            .collect();
        if tracks.is_empty() {
            return Ok(0);
        }
        let items: Vec<NewItem> = tracks
            .into_iter()
            .map(|track| {
                let local = track.is_local();
                NewItem {
                    source_scheme: track.source.scheme().to_string(),
                    item_id: if local {
                        track.path.to_string_lossy().into_owned()
                    } else {
                        track.source.uri().to_string()
                    },
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    duration: track.duration,
                    cover_path: track
                        .cover_path
                        .as_ref()
                        .map(|path| path.to_string_lossy().into_owned()),
                }
            })
            .collect();
        let transaction = connection.transaction()?;
        let count = insert_items(&transaction, playlist_id, &items)?;
        touch_playlist(&transaction, playlist_id)?;
        transaction.commit()?;
        Ok(count)
    }

    pub fn remove_playlist_item(&self, item_id: PlaylistItemId) -> Result<bool, LibraryError> {
        let mut connection = self.lock()?;
        let playlist_id: Option<i64> = connection
            .query_row(
                "SELECT playlist_id FROM playlist_items WHERE id = ?1",
                params![item_id.get()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(playlist_id) = playlist_id else {
            return Ok(false);
        };
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM playlist_items WHERE id = ?1",
            params![item_id.get()],
        )?;
        renumber(&transaction, PlaylistId::new(playlist_id))?;
        touch_playlist(&transaction, PlaylistId::new(playlist_id))?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn move_playlist_item(
        &self,
        playlist_id: PlaylistId,
        item_id: PlaylistItemId,
        delta: i32,
    ) -> Result<bool, LibraryError> {
        if delta == 0 {
            return Ok(false);
        }
        let mut connection = self.lock()?;
        ensure_playlist(&connection, playlist_id)?;
        let mut ids: Vec<i64> = {
            let mut statement = connection.prepare(
                "SELECT id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position ASC",
            )?;
            statement
                .query_map(params![playlist_id.get()], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let Some(index) = ids.iter().position(|id| *id == item_id.get()) else {
            return Ok(false);
        };
        let target = index as i64 + i64::from(delta);
        if target < 0 || target >= ids.len() as i64 {
            return Ok(false);
        }
        ids.swap(index, target as usize);
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE playlist_items SET position = position + 1000000 WHERE playlist_id = ?1",
            params![playlist_id.get()],
        )?;
        for (position, id) in ids.iter().enumerate() {
            transaction.execute(
                "UPDATE playlist_items SET position = ?2 WHERE id = ?1",
                params![id, position as i64],
            )?;
        }
        touch_playlist(&transaction, playlist_id)?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn remove_missing_playlist_items(
        &self,
        playlist_id: PlaylistId,
    ) -> Result<usize, LibraryError> {
        let mut connection = self.lock()?;
        ensure_playlist(&connection, playlist_id)?;
        let items = {
            let mut statement = connection.prepare(&format!(
                "SELECT {ITEM_COLUMNS} FROM playlist_items WHERE playlist_id = ?1 \
                 ORDER BY position ASC"
            ))?;
            statement
                .query_map(params![playlist_id.get()], item_from_row)?
                .collect::<Result<Vec<_>, _>>()?
        };
        let missing: Vec<i64> = items
            .iter()
            .filter(|item| !item.available)
            .map(|item| item.id.get())
            .collect();
        if missing.is_empty() {
            return Ok(0);
        }
        let transaction = connection.transaction()?;
        for id in &missing {
            transaction.execute("DELETE FROM playlist_items WHERE id = ?1", params![id])?;
        }
        renumber(&transaction, playlist_id)?;
        touch_playlist(&transaction, playlist_id)?;
        transaction.commit()?;
        Ok(missing.len())
    }

    /// Tracks ready for the audio engine. Missing local files are skipped.
    pub fn playlist_tracks(&self, playlist_id: PlaylistId) -> Result<Vec<Track>, LibraryError> {
        let items = self.playlist_items(playlist_id)?;
        Ok(items
            .into_iter()
            .filter(|item| item.available)
            .map(item_track)
            .collect())
    }

    pub fn save_queue_as_playlist(
        &self,
        name: &str,
        tracks: &[Track],
    ) -> Result<PlaylistId, LibraryError> {
        let id = self.create_playlist(name)?;
        self.add_tracks_to_playlist(id, tracks)?;
        Ok(id)
    }

    pub fn export_m3u8(&self, playlist_id: PlaylistId, path: &Path) -> Result<(), LibraryError> {
        let items = self.playlist_items(playlist_id)?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
        let mut text = String::from("#EXTM3U\n");
        for item in items {
            let location = m3u8_location(&item, parent);
            let label = if item.artist.trim().is_empty() {
                item.title.clone()
            } else {
                format!("{} - {}", item.artist, item.title)
            };
            text.push_str(&format!(
                "#EXTINF:{},{}\n{}\n",
                item.duration.as_secs(),
                label,
                location
            ));
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    pub fn import_m3u8(&self, path: &Path) -> Result<PlaylistId, LibraryError> {
        let text = std::fs::read_to_string(path)?;
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let entries = parse_m3u8(&text, base);
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("Playlist importata");
        let mut connection = self.lock()?;
        let now = now();
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![name, now],
        )?;
        let playlist_id = PlaylistId::new(transaction.last_insert_rowid());
        let items: Vec<NewItem> = entries
            .into_iter()
            .map(|entry| NewItem {
                source_scheme: "local".into(),
                item_id: entry.path.to_string_lossy().into_owned(),
                title: entry.title,
                artist: entry.artist,
                album: String::new(),
                duration: entry.duration,
                cover_path: None,
            })
            .collect();
        insert_items(&transaction, playlist_id, &items)?;
        touch_playlist(&transaction, playlist_id)?;
        transaction.commit()?;
        Ok(playlist_id)
    }
}

fn clean_name(name: &str) -> Result<&str, LibraryError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LibraryError::Query("playlist name is empty".into()));
    }
    if name.chars().count() > 120 {
        return Err(LibraryError::Query("playlist name is too long".into()));
    }
    Ok(name)
}

fn ensure_playlist(connection: &Connection, id: PlaylistId) -> Result<(), LibraryError> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
        params![id.get()],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(LibraryError::Query("playlist not found".into()))
    }
}

fn insert_items(
    connection: &Connection,
    playlist_id: PlaylistId,
    items: &[NewItem],
) -> Result<usize, LibraryError> {
    let start: i64 = connection.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_items WHERE playlist_id = ?1",
        params![playlist_id.get()],
        |row| row.get(0),
    )?;
    for (offset, item) in items.iter().enumerate() {
        connection.execute(
            "INSERT INTO playlist_items
                (playlist_id, position, source_scheme, item_id, title, artist, album,
                 duration_ms, cover_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                playlist_id.get(),
                start + offset as i64,
                item.source_scheme,
                item.item_id,
                item.title,
                item.artist,
                item.album,
                i64::try_from(item.duration.as_millis()).unwrap_or(i64::MAX),
                item.cover_path,
            ],
        )?;
    }
    Ok(items.len())
}

fn renumber(connection: &Connection, playlist_id: PlaylistId) -> Result<(), LibraryError> {
    let ids: Vec<i64> = {
        let mut statement = connection.prepare(
            "SELECT id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position ASC",
        )?;
        statement
            .query_map(params![playlist_id.get()], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    connection.execute(
        "UPDATE playlist_items SET position = position + 1000000 WHERE playlist_id = ?1",
        params![playlist_id.get()],
    )?;
    for (position, id) in ids.iter().enumerate() {
        connection.execute(
            "UPDATE playlist_items SET position = ?2 WHERE id = ?1",
            params![id, position as i64],
        )?;
    }
    Ok(())
}

fn touch_playlist(connection: &Connection, playlist_id: PlaylistId) -> Result<(), LibraryError> {
    connection.execute(
        "UPDATE playlists SET updated_at = ?2 WHERE id = ?1",
        params![playlist_id.get(), now()],
    )?;
    Ok(())
}

fn playlist_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Playlist> {
    let count: i64 = row.get("item_count")?;
    Ok(Playlist {
        id: PlaylistId::new(row.get("id")?),
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        item_count: count.max(0) as u32,
    })
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlaylistItem> {
    let source_scheme: String = row.get("source_scheme")?;
    let item_id: String = row.get("item_id")?;
    let duration_ms: i64 = row.get("duration_ms")?;
    let cover: Option<String> = row.get("cover_path")?;
    let available = source_scheme != "local" || Path::new(&item_id).is_file();
    Ok(PlaylistItem {
        id: PlaylistItemId::new(row.get("id")?),
        playlist_id: PlaylistId::new(row.get("playlist_id")?),
        position: u32::try_from(row.get::<_, i64>("position")?).unwrap_or_default(),
        source: SourceId::new(source_scheme, item_id.clone()),
        item_id,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        duration: Duration::from_millis(duration_ms.max(0) as u64),
        cover_path: cover.map(PathBuf::from),
        available,
    })
}

fn item_track(item: PlaylistItem) -> Track {
    let local = item.source.is_local();
    Track {
        // Playlist entries are not library rows: a negative id keeps them from
        // matching a real library track in the UI's "now playing" highlight.
        id: TrackId::new(-item.id.get()),
        source: item.source,
        path: if local {
            PathBuf::from(&item.item_id)
        } else {
            PathBuf::new()
        },
        title: item.title,
        artist: item.artist,
        album: item.album,
        album_artist: None,
        track_number: None,
        disc_number: None,
        duration: item.duration,
        year: None,
        genre: None,
        cover_path: item.cover_path,
        added_at: 0,
    }
}

fn m3u8_location(item: &PlaylistItem, parent: &Path) -> String {
    if !item.source.is_local() {
        return item.item_id.clone();
    }
    let path = Path::new(&item.item_id);
    if let (Ok(parent), Ok(full)) = (std::fs::canonicalize(parent), std::fs::canonicalize(path))
        && let Ok(relative) = full.strip_prefix(&parent)
    {
        return relative.to_string_lossy().into_owned();
    }
    item.item_id.clone()
}

#[derive(Debug)]
struct M3uEntry {
    path: PathBuf,
    title: String,
    artist: String,
    duration: Duration,
}

fn parse_m3u8(text: &str, base: &Path) -> Vec<M3uEntry> {
    let mut entries = Vec::new();
    let mut pending: Option<(Duration, String, String)> = None;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') && !line.starts_with("#EXTINF:") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let (seconds, label) = rest.split_once(',').unwrap_or((rest, ""));
            let duration = seconds
                .trim()
                .parse::<u64>()
                .map(Duration::from_secs)
                .unwrap_or_default();
            let (artist, title) = label
                .split_once(" - ")
                .map(|(artist, title)| (artist.trim().to_string(), title.trim().to_string()))
                .unwrap_or_else(|| (String::new(), label.trim().to_string()));
            pending = Some((duration, artist, title));
            continue;
        }
        if line.contains("://") {
            pending = None;
            continue;
        }
        let path = Path::new(line);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        };
        let (duration, artist, mut title) = pending.take().unwrap_or_default();
        if title.is_empty() {
            title = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Brano")
                .to_string();
        }
        entries.push(M3uEntry {
            path,
            title,
            artist,
            duration,
        });
    }
    entries
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::fs;

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

    fn track(path: &Path, title: &str) -> Track {
        Track {
            id: TrackId::new(1),
            source: SourceId::local(path),
            path: path.to_path_buf(),
            title: title.into(),
            artist: "Artist".into(),
            album: "Album".into(),
            album_artist: None,
            track_number: None,
            disc_number: None,
            duration: Duration::from_secs(120),
            year: None,
            genre: None,
            cover_path: None,
            added_at: 0,
        }
    }

    fn source_track(source: SourceId, title: &str) -> Track {
        Track {
            id: TrackId::new(2),
            source,
            path: PathBuf::new(),
            title: title.into(),
            artist: "Artist".into(),
            album: String::new(),
            album_artist: None,
            track_number: None,
            disc_number: None,
            duration: Duration::from_secs(30),
            year: None,
            genre: None,
            cover_path: None,
            added_at: 0,
        }
    }

    #[test]
    fn playlist_crud_order_and_cascade() {
        let (dir, library) = temporary_library();
        let id = library.create_playlist("Test").expect("create");
        let a = dir.path().join("a.mp3");
        let b = dir.path().join("b.mp3");
        fs::write(&a, b"a").expect("a");
        fs::write(&b, b"b").expect("b");
        assert_eq!(
            library
                .add_tracks_to_playlist(id, &[track(&a, "A"), track(&b, "B")])
                .expect("add"),
            2
        );
        let items = library.playlist_items(id).expect("items");
        assert_eq!(
            items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["A", "B"]
        );
        let first = items[0].id;
        let second = items[1].id;
        assert!(library.move_playlist_item(id, first, 1).expect("move"));
        let items = library.playlist_items(id).expect("items");
        assert_eq!(
            items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["B", "A"]
        );
        assert!(library.remove_playlist_item(second).expect("remove"));
        assert_eq!(library.playlist_items(id).expect("items").len(), 1);
        library.rename_playlist(id, "Renamed").expect("rename");
        assert_eq!(
            library.playlist(id).expect("get").expect("found").name,
            "Renamed"
        );
        library.delete_playlist(id).expect("delete");
        assert!(library.playlist(id).expect("get").is_none());
        assert!(library.playlist_items(id).expect("items").is_empty());
    }

    #[test]
    fn missing_items_stay_visible_and_are_skipped_in_playback() {
        let (dir, library) = temporary_library();
        let id = library.create_playlist("Missing").expect("create");
        let present = dir.path().join("present.mp3");
        let missing = dir.path().join("missing.mp3");
        fs::write(&present, b"present").expect("present");
        library
            .add_tracks_to_playlist(
                id,
                &[track(&missing, "Missing"), track(&present, "Present")],
            )
            .expect("add");
        let items = library.playlist_items(id).expect("items");
        assert!(!items[0].available);
        assert!(items[1].available);
        assert_eq!(library.playlist_tracks(id).expect("tracks").len(), 1);
        assert_eq!(
            library.remove_missing_playlist_items(id).expect("remove"),
            1
        );
        assert_eq!(library.playlist_items(id).expect("items").len(), 1);
    }

    #[test]
    fn m3u8_round_trip_uses_utf8_metadata_and_relative_paths() {
        let (dir, library) = temporary_library();
        let audio_dir = dir.path().join("audio");
        fs::create_dir_all(&audio_dir).expect("audio dir");
        let a = audio_dir.join("a.mp3");
        let b = audio_dir.join("b.mp3");
        fs::write(&a, b"a").expect("a");
        fs::write(&b, b"b").expect("b");
        let id = library.create_playlist("Originale").expect("create");
        library
            .add_tracks_to_playlist(id, &[track(&a, "Città"), track(&b, "Brano due")])
            .expect("add");
        let export = dir.path().join("lista.m3u8");
        library.export_m3u8(id, &export).expect("export");
        let text = fs::read_to_string(&export).expect("read");
        assert!(text.contains("#EXTM3U"));
        assert!(text.contains("#EXTINF:120,Artist - Città"));
        assert!(text.contains("audio\\a.mp3") || text.contains("audio/a.mp3"));

        let imported = library.import_m3u8(&export).expect("import");
        let items = library.playlist_items(imported).expect("imported items");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Città");
        assert_eq!(items[0].artist, "Artist");
        assert_eq!(items[0].duration, Duration::from_secs(120));
        assert_eq!(items[1].title, "Brano due");
        assert!(items[0].available);
    }

    #[test]
    fn radio_and_podcast_tracks_are_not_added() {
        let (_dir, library) = temporary_library();
        let id = library.create_playlist("No streams").expect("create");
        let radio = source_track(SourceId::radio("https://stream.example/jazz"), "Radio");
        let podcast = source_track(
            SourceId::podcast("https://example.com/episode.mp3"),
            "Episode",
        );
        assert_eq!(
            library
                .add_tracks_to_playlist(id, &[radio, podcast])
                .expect("add"),
            0
        );
        assert!(library.playlist_items(id).expect("items").is_empty());
    }

    #[test]
    fn m3u8_round_trip_keeps_absolute_paths() {
        let (dir, library) = temporary_library();
        let outside = dir.path().join("outside.mp3");
        fs::write(&outside, b"x").expect("file");
        let id = library.create_playlist("Absolute").expect("create");
        library
            .add_tracks_to_playlist(id, &[track(&outside, "Assoluto")])
            .expect("add");
        let export_dir = dir.path().join("exports");
        fs::create_dir_all(&export_dir).expect("exports");
        let export = export_dir.join("absolute.m3u8");
        library.export_m3u8(id, &export).expect("export");
        let text = fs::read_to_string(&export).expect("read");
        assert!(text.contains(&outside.to_string_lossy().to_string()));
        let imported = library.import_m3u8(&export).expect("import");
        let items = library.playlist_items(imported).expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_id, outside.to_string_lossy());
        assert!(items[0].available);
    }

    #[test]
    fn m3u8_ignores_stream_uris() {
        let (dir, library) = temporary_library();
        let path = dir.path().join("streams.m3u8");
        fs::write(
            &path,
            "#EXTM3U\n#EXTINF:0,Radio\nhttps://stream.example/a\n#EXTINF:120,Locale\na.mp3\n",
        )
        .expect("write");
        let id = library.import_m3u8(&path).expect("import");
        let items = library.playlist_items(id).expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Locale");
        assert_eq!(items[0].duration, Duration::from_secs(120));
    }

    #[test]
    fn legacy_playlist_tracks_are_migrated() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("library.sqlite3");
        {
            let library = Library::open(&db_path, dir.path().join("covers")).expect("open");
            let connection = library.lock().expect("lock");
            connection
                .execute_batch(
                    "CREATE TABLE playlist_tracks (
                        playlist_id INTEGER NOT NULL,
                        track_id INTEGER NOT NULL,
                        position INTEGER NOT NULL,
                        PRIMARY KEY (playlist_id, position)
                    );",
                )
                .expect("legacy table");
            connection
                .execute(
                    "INSERT INTO artists(name) VALUES ('Artist') ON CONFLICT(name) DO NOTHING",
                    [],
                )
                .expect("artist");
            let artist_id: i64 = connection
                .query_row("SELECT id FROM artists WHERE name='Artist'", [], |row| {
                    row.get(0)
                })
                .expect("artist id");
            connection
                .execute(
                    "INSERT INTO albums(title, artist) VALUES ('Album', 'Artist')",
                    [],
                )
                .expect("album");
            let album_id: i64 = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO tracks
                        (path, title, artist, album, artist_id, album_id, duration_ms,
                         mtime_ns, file_size, added_at, updated_at)
                     VALUES ('/old/song.mp3', 'Old', 'Artist', 'Album', ?1, ?2, 5000, 0, 0, 0, 0)",
                    params![artist_id, album_id],
                )
                .expect("track");
            let track_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO playlists(name, created_at, updated_at) VALUES ('Legacy', 1, 1)",
                    [],
                )
                .expect("playlist");
            let playlist_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO playlist_tracks(playlist_id, track_id, position) VALUES (?1, ?2, 0)",
                    params![playlist_id, track_id],
                )
                .expect("legacy item");
        }
        let library = Library::open(&db_path, dir.path().join("covers")).expect("reopen");
        let playlists = library.playlists().expect("playlists");
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Legacy");
        let items = library.playlist_items(playlists[0].id).expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Old");
        assert_eq!(items[0].item_id, "/old/song.mp3");
        let legacy_exists: bool = library
            .lock()
            .expect("lock")
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='playlist_tracks')",
                [],
                |row| row.get(0),
            )
            .expect("legacy check");
        assert!(!legacy_exists);
        let version: i64 = library
            .lock()
            .expect("lock")
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, 2);
        drop(library);
        let library = Library::open(&db_path, dir.path().join("covers")).expect("reopen again");
        assert_eq!(library.playlists().expect("playlists").len(), 1);
        assert_eq!(
            library
                .playlist_items(playlists[0].id)
                .expect("items")
                .len(),
            1
        );
    }
}
