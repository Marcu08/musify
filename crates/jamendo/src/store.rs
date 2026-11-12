use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::JamendoError;
use crate::model::JamendoTrack;

pub struct JamendoStore {
    connection: Mutex<Connection>,
    data_dir: PathBuf,
}

impl JamendoStore {
    pub fn open(path: &Path) -> Result<Self, JamendoError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| JamendoError::Storage(error.to_string()))?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS tracks (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 artist_name TEXT NOT NULL,
                 album_name TEXT NOT NULL,
                 duration_secs INTEGER NOT NULL,
                 audio_url TEXT NOT NULL,
                 image_url TEXT NOT NULL,
                 license_url TEXT NOT NULL,
                 share_url TEXT NOT NULL,
                 cached_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS favorites (
                 track_id TEXT PRIMARY KEY,
                 added_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS searches (
                 query_key TEXT PRIMARY KEY,
                 payload TEXT NOT NULL,
                 cached_at INTEGER NOT NULL
             );",
        )?;
        let store = Self {
            connection: Mutex::new(connection),
            data_dir: path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        };
        store.prune()?;
        Ok(store)
    }

    #[must_use]
    pub fn artwork_dir(&self) -> PathBuf {
        self.data_dir.join("jamendo-covers")
    }

    pub fn upsert_tracks(&self, tracks: &[JamendoTrack]) -> Result<(), JamendoError> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let now = unix_now();
        for track in tracks {
            upsert_track(&transaction, track, now)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn track(&self, id: &str) -> Result<Option<JamendoTrack>, JamendoError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, name, artist_name, album_name, duration_secs, audio_url,
                    image_url, license_url, share_url
             FROM tracks WHERE id = ?1",
        )?;
        let mut rows = statement.query(params![id])?;
        rows.next()?
            .map(track_from_row)
            .transpose()
            .map_err(JamendoError::from)
    }

    pub fn is_favorite(&self, id: &str) -> Result<bool, JamendoError> {
        let connection = self.lock()?;
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM favorites WHERE track_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn favorite_ids(&self) -> Result<HashSet<String>, JamendoError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare("SELECT track_id FROM favorites")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = HashSet::new();
        for row in rows {
            ids.insert(row?);
        }
        Ok(ids)
    }

    pub fn favorites(&self) -> Result<Vec<JamendoTrack>, JamendoError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT t.id, t.name, t.artist_name, t.album_name, t.duration_secs,
                    t.audio_url, t.image_url, t.license_url, t.share_url
             FROM favorites f JOIN tracks t ON t.id = f.track_id
             ORDER BY f.added_at DESC",
        )?;
        let rows = statement.query_map([], track_from_row)?;
        collect_tracks(rows)
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), JamendoError> {
        let connection = self.lock()?;
        if favorite {
            connection.execute(
                "INSERT OR REPLACE INTO favorites (track_id, added_at) VALUES (?1, ?2)",
                params![id, unix_now()],
            )?;
        } else {
            connection.execute("DELETE FROM favorites WHERE track_id = ?1", params![id])?;
        }
        Ok(())
    }

    pub fn store_search(
        &self,
        query_key: &str,
        tracks: &[JamendoTrack],
    ) -> Result<(), JamendoError> {
        let payload = serde_json::to_string(tracks)
            .map_err(|error| JamendoError::Storage(error.to_string()))?;
        let connection = self.lock()?;
        connection.execute(
            "INSERT OR REPLACE INTO searches (query_key, payload, cached_at)
             VALUES (?1, ?2, ?3)",
            params![query_key, payload, unix_now()],
        )?;
        Ok(())
    }

    pub fn cached_search(
        &self,
        query_key: &str,
        max_age_secs: i64,
    ) -> Result<Option<Vec<JamendoTrack>>, JamendoError> {
        let connection = self.lock()?;
        let row: Option<(String, i64)> = connection
            .query_row(
                "SELECT payload, cached_at FROM searches WHERE query_key = ?1",
                params![query_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((payload, cached_at)) = row else {
            return Ok(None);
        };
        if cached_at < unix_now() - max_age_secs.max(0) {
            return Ok(None);
        }
        match serde_json::from_str::<Vec<JamendoTrack>>(&payload) {
            Ok(tracks) => Ok(Some(tracks)),
            Err(_) => {
                connection.execute(
                    "DELETE FROM searches WHERE query_key = ?1",
                    params![query_key],
                )?;
                Ok(None)
            }
        }
    }

    #[cfg(test)]
    pub fn age_search_cache(&self, seconds: i64) -> Result<(), JamendoError> {
        let connection = self.lock()?;
        connection.execute(
            "UPDATE searches SET cached_at = cached_at - ?1",
            params![seconds.max(0)],
        )?;
        Ok(())
    }

    fn prune(&self) -> Result<(), JamendoError> {
        let connection = self.lock()?;
        let now = unix_now();
        connection.execute(
            "DELETE FROM searches WHERE cached_at < ?1",
            params![now - 7 * 86_400],
        )?;
        connection.execute(
            "DELETE FROM tracks
             WHERE cached_at < ?1
               AND id NOT IN (SELECT track_id FROM favorites)",
            params![now - 30 * 86_400],
        )?;
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, JamendoError> {
        self.connection
            .lock()
            .map_err(|_| JamendoError::Storage("Jamendo database lock poisoned".into()))
    }
}

fn upsert_track(
    connection: &Connection,
    track: &JamendoTrack,
    now: i64,
) -> Result<(), JamendoError> {
    connection.execute(
        "INSERT INTO tracks
            (id, name, artist_name, album_name, duration_secs, audio_url, image_url,
             license_url, share_url, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
             name = excluded.name,
             artist_name = excluded.artist_name,
             album_name = excluded.album_name,
             duration_secs = excluded.duration_secs,
             audio_url = excluded.audio_url,
             image_url = excluded.image_url,
             license_url = excluded.license_url,
             share_url = excluded.share_url,
             cached_at = excluded.cached_at",
        params![
            track.id,
            track.name,
            track.artist_name,
            track.album_name,
            i64::try_from(track.duration_secs).unwrap_or(i64::MAX),
            track.audio_url,
            track.image_url,
            track.license_url,
            track.share_url,
            now,
        ],
    )?;
    Ok(())
}

fn track_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JamendoTrack> {
    Ok(JamendoTrack {
        id: row.get(0)?,
        name: row.get(1)?,
        artist_name: row.get(2)?,
        album_name: row.get(3)?,
        duration_secs: u64::try_from(row.get::<_, i64>(4)?.max(0)).unwrap_or(0),
        audio_url: row.get(5)?,
        image_url: row.get(6)?,
        license_url: row.get(7)?,
        share_url: row.get(8)?,
    })
}

fn collect_tracks<F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<JamendoTrack>, JamendoError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<JamendoTrack>,
{
    let mut tracks = Vec::new();
    for row in rows {
        tracks.push(row?);
    }
    Ok(tracks)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn track(id: &str) -> JamendoTrack {
        JamendoTrack {
            id: id.into(),
            name: format!("Track {id}"),
            artist_name: "Artist".into(),
            album_name: "Album".into(),
            duration_secs: 180,
            audio_url: format!("https://audio.example/{id}.mp3"),
            image_url: format!("https://img.example/{id}.jpg"),
            license_url: "http://creativecommons.org/licenses/by-sa/3.0/".into(),
            share_url: format!("https://www.jamendo.com/track/{id}"),
        }
    }

    #[test]
    fn favorites_persist_and_search_cache_expires() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("jamendo.sqlite3");
        let store = JamendoStore::open(&path).expect("open");
        store
            .upsert_tracks(&[track("a"), track("b")])
            .expect("cache tracks");
        assert!(store.favorites().expect("empty favorites").is_empty());
        store.set_favorite("a", true).expect("favorite");
        assert!(store.is_favorite("a").expect("favorite flag"));
        assert_eq!(store.favorites().expect("favorites").len(), 1);
        store
            .store_search("jazz", &[track("a"), track("b")])
            .expect("cache search");
        assert_eq!(
            store
                .cached_search("jazz", 60)
                .expect("fresh cache")
                .expect("cached")
                .len(),
            2
        );
        store.age_search_cache(120).expect("age cache");
        assert!(
            store
                .cached_search("jazz", 60)
                .expect("expired cache")
                .is_none()
        );
        assert!(
            store
                .cached_search("jazz", 300)
                .expect("older cache")
                .is_some()
        );
        drop(store);
        let store = JamendoStore::open(&path).expect("reopen");
        assert_eq!(store.favorites().expect("favorites").len(), 1);
        assert_eq!(
            store.track("a").expect("track").expect("exists").name,
            "Track a"
        );
    }
}
