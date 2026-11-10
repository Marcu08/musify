use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

use crate::error::RadioError;
use crate::station::Station;

pub struct RadioStore {
    connection: Mutex<Connection>,
    data_dir: std::path::PathBuf,
}

impl RadioStore {
    pub fn open(path: &Path) -> Result<Self, RadioError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| RadioError::Database(error.to_string()))?;
        }
        let connection =
            Connection::open(path).map_err(|error| RadioError::Database(error.to_string()))?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 CREATE TABLE IF NOT EXISTS stations (
                     stationuuid TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     url TEXT NOT NULL,
                     url_resolved TEXT NOT NULL,
                     homepage TEXT NOT NULL,
                     favicon TEXT NOT NULL,
                     tags TEXT NOT NULL,
                     country TEXT NOT NULL,
                     country_code TEXT NOT NULL,
                     codec TEXT NOT NULL,
                     bitrate INTEGER NOT NULL,
                     votes INTEGER NOT NULL,
                     last_check_ok INTEGER NOT NULL,
                     cached_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS favorites (
                     stationuuid TEXT PRIMARY KEY,
                     added_at INTEGER NOT NULL
                 );",
            )
            .map_err(|error| RadioError::Database(error.to_string()))?;
        Ok(Self {
            connection: Mutex::new(connection),
            data_dir: path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        })
    }

    pub fn icon_dir(&self) -> std::path::PathBuf {
        self.data_dir.join("radio-icons")
    }

    pub fn upsert_many(&self, stations: &[Station]) -> Result<(), RadioError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        let transaction = connection
            .transaction()
            .map_err(|error| RadioError::Database(error.to_string()))?;
        let now = unix_now();
        for station in stations {
            upsert_station(&transaction, station, now)?;
        }
        transaction
            .commit()
            .map_err(|error| RadioError::Database(error.to_string()))
    }

    pub fn upsert(&self, station: &Station) -> Result<(), RadioError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        upsert_station(&connection, station, unix_now())
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), RadioError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        if favorite {
            connection
                .execute(
                    "INSERT OR REPLACE INTO favorites (stationuuid, added_at) VALUES (?1, ?2)",
                    params![id, unix_now()],
                )
                .map_err(|error| RadioError::Database(error.to_string()))?;
        } else {
            connection
                .execute("DELETE FROM favorites WHERE stationuuid = ?1", params![id])
                .map_err(|error| RadioError::Database(error.to_string()))?;
        }
        Ok(())
    }

    pub fn is_favorite(&self, id: &str) -> Result<bool, RadioError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM favorites WHERE stationuuid = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| RadioError::Database(error.to_string()))?;
        Ok(count > 0)
    }

    pub fn favorite_ids(&self) -> Result<HashSet<String>, RadioError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        let mut statement = connection
            .prepare("SELECT stationuuid FROM favorites")
            .map_err(|error| RadioError::Database(error.to_string()))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| RadioError::Database(error.to_string()))?;
        let mut ids = HashSet::new();
        for row in rows {
            ids.insert(row.map_err(|error| RadioError::Database(error.to_string()))?);
        }
        Ok(ids)
    }

    pub fn favorites(&self) -> Result<Vec<Station>, RadioError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        let mut statement = connection
            .prepare(
                "SELECT s.stationuuid, s.name, s.url, s.url_resolved, s.homepage, s.favicon,
                        s.tags, s.country, s.country_code, s.codec, s.bitrate, s.votes,
                        s.last_check_ok
                 FROM favorites f JOIN stations s ON s.stationuuid = f.stationuuid
                 ORDER BY f.added_at DESC",
            )
            .map_err(|error| RadioError::Database(error.to_string()))?;
        let rows = statement
            .query_map([], station_from_row)
            .map_err(|error| RadioError::Database(error.to_string()))?;
        collect_stations(rows)
    }

    pub fn station(&self, id: &str) -> Result<Option<Station>, RadioError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        let mut statement = connection
            .prepare(
                "SELECT stationuuid, name, url, url_resolved, homepage, favicon, tags,
                        country, country_code, codec, bitrate, votes, last_check_ok
                 FROM stations WHERE stationuuid = ?1",
            )
            .map_err(|error| RadioError::Database(error.to_string()))?;
        let mut rows = statement
            .query(params![id])
            .map_err(|error| RadioError::Database(error.to_string()))?;
        rows.next()
            .map_err(|error| RadioError::Database(error.to_string()))?
            .map(station_from_row)
            .transpose()
            .map_err(|error| RadioError::Database(error.to_string()))
    }

    #[cfg(test)]
    pub fn age_cache(&self, seconds: i64) -> Result<(), RadioError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        connection
            .execute(
                "UPDATE stations SET cached_at = cached_at - ?1",
                params![seconds.max(0)],
            )
            .map_err(|error| RadioError::Database(error.to_string()))?;
        Ok(())
    }

    pub fn cached_search(
        &self,
        query: &Query,
        limit: usize,
        max_age_secs: i64,
    ) -> Result<Vec<Station>, RadioError> {
        let name = format!("%{}%", query.name.trim());
        let country = format!("%{}%", query.country.trim());
        let tag = format!("%{}%", query.tag.trim());
        let connection = self
            .connection
            .lock()
            .map_err(|_| RadioError::Database("radio database lock poisoned".into()))?;
        let mut statement = connection
            .prepare(
                "SELECT stationuuid, name, url, url_resolved, homepage, favicon, tags,
                        country, country_code, codec, bitrate, votes, last_check_ok
                 FROM stations
                 WHERE (?1 = '%%' OR name LIKE ?1)
                   AND (?2 = '%%' OR country LIKE ?2 OR country_code LIKE ?2)
                   AND (?3 = '%%' OR tags LIKE ?3)
                   AND (?5 = 0 OR cached_at >= CAST(strftime('%s','now') AS INTEGER) - ?5)
                 ORDER BY votes DESC, name
                 LIMIT ?4",
            )
            .map_err(|error| RadioError::Database(error.to_string()))?;
        let rows = statement
            .query_map(
                params![
                    name,
                    country,
                    tag,
                    i64::try_from(limit).unwrap_or(100),
                    max_age_secs
                ],
                station_from_row,
            )
            .map_err(|error| RadioError::Database(error.to_string()))?;
        collect_stations(rows)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub name: String,
    pub country: String,
    pub tag: String,
}

fn upsert_station(connection: &Connection, station: &Station, now: i64) -> Result<(), RadioError> {
    connection
        .execute(
            "INSERT INTO stations (stationuuid, name, url, url_resolved, homepage, favicon,
                                   tags, country, country_code, codec, bitrate, votes,
                                   last_check_ok, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(stationuuid) DO UPDATE SET
                 name = excluded.name,
                 url = excluded.url,
                 url_resolved = excluded.url_resolved,
                 homepage = excluded.homepage,
                 favicon = excluded.favicon,
                 tags = excluded.tags,
                 country = excluded.country,
                 country_code = excluded.country_code,
                 codec = excluded.codec,
                 bitrate = excluded.bitrate,
                 votes = excluded.votes,
                 last_check_ok = excluded.last_check_ok,
                 cached_at = excluded.cached_at",
            params![
                station.id,
                station.name,
                station.url,
                station.url_resolved,
                station.homepage,
                station.favicon,
                station.tags,
                station.country,
                station.country_code,
                station.codec,
                station.bitrate,
                station.votes,
                station.last_check_ok,
                now,
            ],
        )
        .map_err(|error| RadioError::Database(error.to_string()))?;
    Ok(())
}

fn station_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Station> {
    Ok(Station {
        id: row.get(0)?,
        name: row.get(1)?,
        url: row.get(2)?,
        url_resolved: row.get(3)?,
        homepage: row.get(4)?,
        favicon: row.get(5)?,
        tags: row.get(6)?,
        country: row.get(7)?,
        country_code: row.get(8)?,
        codec: row.get(9)?,
        bitrate: row.get(10)?,
        votes: row.get(11)?,
        last_check_ok: row.get(12)?,
    })
}

fn collect_stations<F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<Station>, RadioError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Station>,
{
    let mut stations = Vec::new();
    for row in rows {
        stations.push(row.map_err(|error| RadioError::Database(error.to_string()))?);
    }
    Ok(stations)
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

    fn station(id: &str) -> Station {
        Station {
            id: id.into(),
            name: format!("Station {id}"),
            url_resolved: format!("https://stream.example/{id}"),
            country: "Italy".into(),
            tags: "jazz".into(),
            ..Station::default()
        }
    }

    #[test]
    fn favorites_persist_and_cache_search_works_offline() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = RadioStore::open(&dir.path().join("radio.sqlite3")).expect("open store");
        store
            .upsert_many(&[station("a"), station("b")])
            .expect("cache");
        assert!(store.favorites().expect("empty favorites").is_empty());
        store.set_favorite("a", true).expect("favorite");
        assert!(store.is_favorite("a").expect("favorite flag"));
        let favorites = store.favorites().expect("favorites");
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].id, "a");
        let query = Query {
            tag: "jazz".into(),
            ..Query::default()
        };
        assert_eq!(store.cached_search(&query, 10, 0).expect("cache").len(), 2);
        store.age_cache(120).expect("age cache");
        assert!(
            store
                .cached_search(&query, 10, 60)
                .expect("fresh cache")
                .is_empty()
        );
        assert_eq!(store.cached_search(&query, 10, 0).expect("cache").len(), 2);
        assert_eq!(
            store.station("a").expect("station").expect("exists").id,
            "a"
        );
    }
}
