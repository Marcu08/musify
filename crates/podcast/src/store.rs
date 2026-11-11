use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::error::PodcastError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodcastRecord {
    pub id: String,
    pub feed_url: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub website: String,
    pub artwork_url: String,
    pub genre: String,
    pub subscribed: bool,
    pub last_refresh: i64,
}

#[derive(Debug, Clone, Default)]
pub struct EpisodeRecord {
    pub id: String,
    pub podcast_id: String,
    pub title: String,
    pub description: String,
    pub published_at: Option<i64>,
    pub duration: Option<Duration>,
    pub audio_url: String,
    pub audio_type: Option<String>,
    pub watched: bool,
    pub position: Duration,
}

pub struct PodcastStore {
    connection: Mutex<Connection>,
}

impl PodcastStore {
    pub fn open(path: &Path) -> Result<Self, PodcastError> {
        let connection = Connection::open(path).map_err(storage)?;
        Self::prepare(connection)
    }

    pub fn open_in_memory() -> Result<Self, PodcastError> {
        let connection = Connection::open_in_memory().map_err(storage)?;
        Self::prepare(connection)
    }

    fn prepare(connection: Connection) -> Result<Self, PodcastError> {
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 PRAGMA busy_timeout=5000;
                 CREATE TABLE IF NOT EXISTS podcasts (
                     id TEXT PRIMARY KEY,
                     feed_url TEXT NOT NULL UNIQUE,
                     title TEXT NOT NULL,
                     author TEXT NOT NULL DEFAULT '',
                     description TEXT NOT NULL DEFAULT '',
                     website TEXT NOT NULL DEFAULT '',
                     artwork_url TEXT NOT NULL DEFAULT '',
                     genre TEXT NOT NULL DEFAULT '',
                     subscribed INTEGER NOT NULL DEFAULT 0,
                     last_refresh INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE IF NOT EXISTS episodes (
                     id TEXT PRIMARY KEY,
                     podcast_id TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
                     title TEXT NOT NULL,
                     description TEXT NOT NULL DEFAULT '',
                     published_at INTEGER,
                     duration_ms INTEGER,
                     audio_url TEXT NOT NULL,
                     audio_type TEXT,
                     watched INTEGER NOT NULL DEFAULT 0,
                     position_ms INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE INDEX IF NOT EXISTS episodes_podcast
                     ON episodes(podcast_id, published_at DESC);",
            )
            .map_err(storage)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn upsert_podcast(&self, podcast: &PodcastRecord) -> Result<(), PodcastError> {
        let connection = self.connection();
        upsert_podcast(&connection, podcast).map_err(storage)
    }

    pub fn upsert_podcasts(&self, podcasts: &[PodcastRecord]) -> Result<(), PodcastError> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        for podcast in podcasts {
            upsert_podcast(&transaction, podcast).map_err(storage)?;
        }
        transaction.commit().map_err(storage)
    }

    pub fn podcast(&self, id: &str) -> Result<Option<PodcastRecord>, PodcastError> {
        let connection = self.connection();
        connection
            .query_row(
                "SELECT id, feed_url, title, author, description, website, artwork_url,
                        genre, subscribed, last_refresh
                 FROM podcasts WHERE id = ?1",
                params![id],
                map_podcast,
            )
            .optional()
            .map_err(storage)
    }

    pub fn subscriptions(&self) -> Result<Vec<PodcastRecord>, PodcastError> {
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT id, feed_url, title, author, description, website, artwork_url,
                        genre, subscribed, last_refresh
                 FROM podcasts WHERE subscribed = 1 ORDER BY title COLLATE NOCASE",
            )
            .map_err(storage)?;
        let rows = statement
            .query_map([], map_podcast)
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        Ok(rows)
    }

    pub fn set_subscribed(&self, id: &str, subscribed: bool) -> Result<(), PodcastError> {
        let connection = self.connection();
        let changed = connection
            .execute(
                "UPDATE podcasts SET subscribed = ?2 WHERE id = ?1",
                params![id, i64::from(subscribed)],
            )
            .map_err(storage)?;
        if changed == 0 {
            return Err(PodcastError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn set_refreshed(&self, id: &str, timestamp: i64) -> Result<(), PodcastError> {
        let connection = self.connection();
        connection
            .execute(
                "UPDATE podcasts SET last_refresh = ?2 WHERE id = ?1",
                params![id, timestamp],
            )
            .map_err(storage)?;
        Ok(())
    }

    pub fn upsert_episodes(
        &self,
        podcast_id: &str,
        episodes: &[EpisodeRecord],
    ) -> Result<(), PodcastError> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        for episode in episodes {
            transaction
                .execute(
                    "INSERT INTO episodes
                        (id, podcast_id, title, description, published_at, duration_ms,
                         audio_url, audio_type)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(id) DO UPDATE SET
                        podcast_id = excluded.podcast_id,
                        title = excluded.title,
                        description = excluded.description,
                        published_at = excluded.published_at,
                        duration_ms = excluded.duration_ms,
                        audio_url = excluded.audio_url,
                        audio_type = excluded.audio_type",
                    params![
                        episode.id,
                        podcast_id,
                        episode.title,
                        episode.description,
                        episode.published_at,
                        episode.duration.map(duration_ms),
                        episode.audio_url,
                        episode.audio_type,
                    ],
                )
                .map_err(storage)?;
        }
        transaction.commit().map_err(storage)
    }

    pub fn episodes(&self, podcast_id: &str) -> Result<Vec<EpisodeRecord>, PodcastError> {
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT id, podcast_id, title, description, published_at, duration_ms,
                        audio_url, audio_type, watched, position_ms
                 FROM episodes WHERE podcast_id = ?1
                 ORDER BY published_at IS NULL, published_at DESC, title COLLATE NOCASE",
            )
            .map_err(storage)?;
        let rows = statement
            .query_map(params![podcast_id], map_episode)
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        Ok(rows)
    }

    pub fn episode(&self, id: &str) -> Result<Option<EpisodeRecord>, PodcastError> {
        let connection = self.connection();
        connection
            .query_row(
                "SELECT id, podcast_id, title, description, published_at, duration_ms,
                        audio_url, audio_type, watched, position_ms
                 FROM episodes WHERE id = ?1",
                params![id],
                map_episode,
            )
            .optional()
            .map_err(storage)
    }

    pub fn save_progress(
        &self,
        id: &str,
        position: Duration,
        watched: bool,
    ) -> Result<(), PodcastError> {
        let connection = self.connection();
        let changed = connection
            .execute(
                "UPDATE episodes SET position_ms = ?2, watched = ?3 WHERE id = ?1",
                params![id, duration_ms(position), i64::from(watched)],
            )
            .map_err(storage)?;
        if changed == 0 {
            return Err(PodcastError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn set_watched(&self, id: &str, watched: bool) -> Result<(), PodcastError> {
        let connection = self.connection();
        let changed = connection
            .execute(
                "UPDATE episodes SET watched = ?2 WHERE id = ?1",
                params![id, i64::from(watched)],
            )
            .map_err(storage)?;
        if changed == 0 {
            return Err(PodcastError::NotFound(id.to_string()));
        }
        Ok(())
    }
}

fn upsert_podcast(connection: &Connection, podcast: &PodcastRecord) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO podcasts
            (id, feed_url, title, author, description, website, artwork_url, genre,
             subscribed, last_refresh)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            feed_url = excluded.feed_url,
            title = excluded.title,
            author = excluded.author,
            description = excluded.description,
            website = excluded.website,
            artwork_url = excluded.artwork_url,
            genre = excluded.genre",
        params![
            podcast.id,
            podcast.feed_url,
            podcast.title,
            podcast.author,
            podcast.description,
            podcast.website,
            podcast.artwork_url,
            podcast.genre,
            i64::from(podcast.subscribed),
            podcast.last_refresh,
        ],
    )?;
    Ok(())
}

fn map_podcast(row: &rusqlite::Row<'_>) -> rusqlite::Result<PodcastRecord> {
    Ok(PodcastRecord {
        id: row.get(0)?,
        feed_url: row.get(1)?,
        title: row.get(2)?,
        author: row.get(3)?,
        description: row.get(4)?,
        website: row.get(5)?,
        artwork_url: row.get(6)?,
        genre: row.get(7)?,
        subscribed: row.get::<_, i64>(8)? != 0,
        last_refresh: row.get(9)?,
    })
}

fn map_episode(row: &rusqlite::Row<'_>) -> rusqlite::Result<EpisodeRecord> {
    let duration_ms: Option<i64> = row.get(5)?;
    let position_ms: i64 = row.get(9)?;
    Ok(EpisodeRecord {
        id: row.get(0)?,
        podcast_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        published_at: row.get(4)?,
        duration: duration_ms
            .and_then(|milliseconds| u64::try_from(milliseconds).ok())
            .map(Duration::from_millis),
        audio_url: row.get(6)?,
        audio_type: row.get(7)?,
        watched: row.get::<_, i64>(8)? != 0,
        position: u64::try_from(position_ms)
            .map(Duration::from_millis)
            .unwrap_or_default(),
    })
}

fn duration_ms(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn storage(error: rusqlite::Error) -> PodcastError {
    PodcastError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn subscriptions_and_episode_progress_persist() {
        let store = PodcastStore::open_in_memory().expect("store");
        let podcast = PodcastRecord {
            id: "podcast-1".into(),
            feed_url: "https://example.com/feed.xml".into(),
            title: "Test".into(),
            ..PodcastRecord::default()
        };
        store.upsert_podcast(&podcast).expect("podcast");
        store.set_subscribed("podcast-1", true).expect("subscribe");
        assert_eq!(store.subscriptions().expect("subscriptions").len(), 1);

        let episode = EpisodeRecord {
            id: "episode-1".into(),
            podcast_id: "podcast-1".into(),
            title: "Episode".into(),
            audio_url: "https://example.com/one.mp3".into(),
            duration: Some(Duration::from_secs(300)),
            ..EpisodeRecord::default()
        };
        store
            .upsert_episodes("podcast-1", &[episode])
            .expect("episode");
        store
            .save_progress("episode-1", Duration::from_secs(42), false)
            .expect("progress");
        let stored = store.episode("episode-1").expect("read").expect("found");
        assert_eq!(stored.position, Duration::from_secs(42));
        assert!(!stored.watched);
        store.set_watched("episode-1", true).expect("watched");
        assert!(
            store
                .episode("episode-1")
                .expect("read")
                .expect("found")
                .watched
        );
    }

    #[test]
    fn progress_survives_store_reopen() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("podcasts.sqlite3");
        {
            let store = PodcastStore::open(&path).expect("store");
            store
                .upsert_podcast(&PodcastRecord {
                    id: "podcast-1".into(),
                    feed_url: "https://example.com/feed.xml".into(),
                    title: "Test".into(),
                    subscribed: true,
                    ..PodcastRecord::default()
                })
                .expect("podcast");
            store
                .upsert_episodes(
                    "podcast-1",
                    &[EpisodeRecord {
                        id: "episode-1".into(),
                        podcast_id: "podcast-1".into(),
                        title: "Episode".into(),
                        audio_url: "https://example.com/one.mp3".into(),
                        duration: Some(Duration::from_secs(300)),
                        ..EpisodeRecord::default()
                    }],
                )
                .expect("episode");
            store
                .save_progress("episode-1", Duration::from_secs(42), false)
                .expect("progress");
        }
        let store = PodcastStore::open(&path).expect("reopen");
        let episode = store.episode("episode-1").expect("read").expect("found");
        assert_eq!(episode.position, Duration::from_secs(42));
        assert!(!episode.watched);
    }
}
