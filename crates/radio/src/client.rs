use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use musify_core::SourceItem;
use serde::Deserialize;

use crate::error::RadioError;
use crate::station::Station;
use crate::store::{Query, RadioStore};

const USER_AGENT: &str = "Musify/0.1.0 (+https://github.com/Marcu08/musify)";
const DEFAULT_SERVER: &str = "https://all.api.radio-browser.info";

pub struct RadioBrowser {
    store: Arc<RadioStore>,
    servers: Mutex<Vec<String>>,
    override_url: Option<String>,
    cache_ttl_secs: i64,
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub name: String,
    pub country: String,
    pub tag: String,
}

#[derive(Debug, Clone)]
pub struct SearchOutcome {
    pub items: Vec<SourceItem>,
    pub offline: bool,
}

impl RadioBrowser {
    pub fn new(store: Arc<RadioStore>) -> Self {
        let override_url = std::env::var("MUSIFY_RADIO_BROWSER_URL")
            .ok()
            .map(|value| value.trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        Self::with_base_url(store, override_url)
    }

    pub fn with_base_url(store: Arc<RadioStore>, base_url: Option<String>) -> Self {
        let cache_ttl_hours = std::env::var("MUSIFY_RADIO_CACHE_TTL_HOURS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(24 * 7)
            .clamp(1, 24 * 365);
        Self {
            store,
            servers: Mutex::new(Vec::new()),
            override_url: base_url,
            cache_ttl_secs: cache_ttl_hours * 3600,
        }
    }

    pub fn search(&self, query: &SearchQuery, limit: usize) -> Result<SearchOutcome, RadioError> {
        let query = Query {
            name: query.name.trim().to_string(),
            country: query.country.trim().to_string(),
            tag: query.tag.trim().to_string(),
        };
        let mut last_error = None;
        for server in self.server_urls() {
            match self.search_server(&server, &query, limit) {
                Ok(stations) => {
                    self.store.upsert_many(&stations)?;
                    return self.outcome(stations, false);
                }
                Err(error) => {
                    tracing::warn!(%error, server, "radio browser request failed");
                    last_error = Some(error);
                }
            }
        }
        let cached = self
            .store
            .cached_search(&query, limit, self.cache_ttl_secs)?;
        if cached.is_empty() {
            Err(last_error.unwrap_or_else(|| RadioError::Network("no server available".into())))
        } else {
            self.outcome(cached, true)
        }
    }

    pub fn favorites(&self) -> Result<Vec<SourceItem>, RadioError> {
        let favorite_ids = self.store.favorite_ids()?;
        Ok(self
            .store
            .favorites()?
            .iter()
            .map(|station| {
                let mut item = station.to_source_item(favorite_ids.contains(&station.id));
                item.image_path = self.icon_path(station);
                item
            })
            .collect())
    }

    pub fn toggle_favorite(&self, station: &Station) -> Result<bool, RadioError> {
        self.store.upsert(station)?;
        let favorite = !self.store.is_favorite(&station.id)?;
        self.store.set_favorite(&station.id, favorite)?;
        Ok(favorite)
    }

    pub fn station(&self, id: &str) -> Result<Option<Station>, RadioError> {
        self.store.station(id)
    }

    pub fn icon_path(&self, station: &Station) -> Option<PathBuf> {
        let url = station.favicon.trim();
        if url.is_empty() {
            return None;
        }
        let key = blake3::hash(url.as_bytes()).to_hex().to_string();
        let path = self.store.icon_dir().join(format!("{key}.jpg"));
        path.exists().then_some(path)
    }

    /// Downloads and caches a station icon. Icons are kept in a bounded folder
    /// (256 files / 16 MB); this runs on a caller-provided background thread.
    pub fn cache_icon(&self, station: &Station) -> Result<Option<PathBuf>, RadioError> {
        if let Some(path) = self.icon_path(station) {
            return Ok(Some(path));
        }
        let url = station.favicon.trim();
        if url.is_empty() {
            return Ok(None);
        }
        let runtime = runtime()?;
        let client = client()?;
        let response = runtime.block_on(async {
            client
                .get(url)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
        });
        let response = response.map_err(|error| RadioError::Network(error.to_string()))?;
        if response
            .content_length()
            .is_some_and(|length| length > 2 * 1024 * 1024)
        {
            return Ok(None);
        }
        let bytes = runtime
            .block_on(response.bytes())
            .map_err(|error| RadioError::Network(error.to_string()))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|error| RadioError::Json(format!("invalid station icon: {error}")))?;
        let image = image.resize_to_fill(64, 64, image::imageops::FilterType::Triangle);
        let dir = self.store.icon_dir();
        std::fs::create_dir_all(&dir).map_err(|error| RadioError::Database(error.to_string()))?;
        let key = blake3::hash(url.as_bytes()).to_hex().to_string();
        let path = dir.join(format!("{key}.jpg"));
        image
            .save(&path)
            .map_err(|error| RadioError::Database(error.to_string()))?;
        prune_icons(&dir);
        Ok(Some(path))
    }

    fn outcome(&self, stations: Vec<Station>, offline: bool) -> Result<SearchOutcome, RadioError> {
        let favorites = self.store.favorite_ids()?;
        Ok(SearchOutcome {
            items: stations
                .iter()
                .map(|station| {
                    let mut item = station.to_source_item(favorites.contains(&station.id));
                    item.image_path = self.icon_path(station);
                    item
                })
                .collect(),
            offline,
        })
    }

    fn search_server(
        &self,
        server: &str,
        query: &Query,
        limit: usize,
    ) -> Result<Vec<Station>, RadioError> {
        let runtime = runtime()?;
        let client = client()?;
        let mut parameters = vec![
            ("hidebroken".to_string(), "true".to_string()),
            ("order".to_string(), "votes".to_string()),
            ("reverse".to_string(), "true".to_string()),
            (
                "limit".to_string(),
                i64::try_from(limit).unwrap_or(100).to_string(),
            ),
        ];
        if !query.name.is_empty() {
            parameters.push(("name".to_string(), query.name.clone()));
        }
        if !query.country.is_empty() {
            parameters.push(("country".to_string(), query.country.clone()));
        }
        if !query.tag.is_empty() {
            parameters.push(("tag".to_string(), query.tag.clone()));
        }
        let url = format!("{server}/json/stations/search");
        let response = runtime.block_on(async {
            client
                .get(url)
                .query(&parameters)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
        });
        let response = response.map_err(|error| RadioError::Network(error.to_string()))?;
        let body = runtime
            .block_on(response.text())
            .map_err(|error| RadioError::Network(error.to_string()))?;
        serde_json::from_str(&body).map_err(|error| RadioError::Json(error.to_string()))
    }

    fn server_urls(&self) -> Vec<String> {
        if let Some(url) = &self.override_url {
            return vec![url.clone()];
        }
        if let Ok(servers) = self.servers.lock()
            && !servers.is_empty()
        {
            return servers.clone();
        }
        let discovered = self.discover_servers();
        if let Ok(mut servers) = self.servers.lock() {
            *servers = discovered.clone();
        }
        discovered
    }

    fn discover_servers(&self) -> Vec<String> {
        let runtime = match runtime() {
            Ok(runtime) => runtime,
            Err(_) => return vec![DEFAULT_SERVER.to_string()],
        };
        let client = match client() {
            Ok(client) => client,
            Err(_) => return vec![DEFAULT_SERVER.to_string()],
        };
        let url = format!("{DEFAULT_SERVER}/json/servers");
        let response = runtime.block_on(async {
            client
                .get(url)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
        });
        let Ok(response) = response else {
            return vec![DEFAULT_SERVER.to_string()];
        };
        let Ok(body) = runtime.block_on(response.text()) else {
            return vec![DEFAULT_SERVER.to_string()];
        };
        let Ok(entries) = serde_json::from_str::<Vec<ServerEntry>>(&body) else {
            return vec![DEFAULT_SERVER.to_string()];
        };
        let mut names: Vec<String> = entries
            .into_iter()
            .map(|entry| entry.name.trim_end_matches('.').to_string())
            .filter(|name| !name.is_empty())
            .collect();
        names.sort();
        names.dedup();
        if names.is_empty() {
            return vec![DEFAULT_SERVER.to_string()];
        }
        // Start from a time-dependent position so repeated launches do not
        // always hit the same mirror, then try every discovered server.
        let start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos() as usize)
            % names.len();
        names.rotate_left(start);
        names
            .into_iter()
            .map(|name| format!("https://{name}"))
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct ServerEntry {
    #[serde(default)]
    name: String,
}

fn prune_icons(dir: &std::path::Path) {
    const MAX_FILES: usize = 256;
    const MAX_BYTES: u64 = 16 * 1024 * 1024;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, SystemTime, u64)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((entry.path(), modified, metadata.len()))
        })
        .collect();
    files.sort_by_key(|(_, modified, _)| std::cmp::Reverse(*modified));
    let mut total = 0;
    for (index, (path, _, size)) in files.iter().enumerate() {
        total += size;
        if index >= MAX_FILES || total > MAX_BYTES {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn runtime() -> Result<tokio::runtime::Runtime, RadioError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RadioError::Network(error.to_string()))
}

fn client() -> Result<reqwest::Client, RadioError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| RadioError::Network(error.to_string()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[test]
    fn search_uses_the_fake_server_then_falls_back_to_the_cache() {
        let runtime = runtime().expect("runtime");
        let server = runtime.block_on(MockServer::start());
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/json/stations/search"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                    "stationuuid": "station-1",
                    "name": "Fake Jazz",
                    "url_resolved": "http://127.0.0.1:1/live.mp3",
                    "country": "Italy",
                    "countrycode": "IT",
                    "tags": "jazz",
                    "codec": "MP3",
                    "bitrate": 128,
                    "votes": 10,
                    "lastcheckok": 1
                }])))
                .mount(&server),
        );
        drop(runtime);

        let dir = tempfile::tempdir().expect("temp dir");
        let store = Arc::new(RadioStore::open(&dir.path().join("radio.sqlite3")).expect("store"));
        let browser = RadioBrowser::with_base_url(Arc::clone(&store), Some(server.uri()));
        let outcome = browser
            .search(
                &SearchQuery {
                    name: "jazz".into(),
                    ..SearchQuery::default()
                },
                20,
            )
            .expect("search");
        assert!(!outcome.offline);
        assert_eq!(outcome.items.len(), 1);
        assert_eq!(outcome.items[0].title, "Fake Jazz");
        let favorite = browser
            .toggle_favorite(&Station {
                id: "station-1".into(),
                name: "Fake Jazz".into(),
                ..Station::default()
            })
            .expect("favorite");
        assert!(favorite);
        assert_eq!(browser.favorites().expect("favorites").len(), 1);

        let offline =
            RadioBrowser::with_base_url(Arc::clone(&store), Some("http://127.0.0.1:1".to_string()));
        let cached = offline
            .search(
                &SearchQuery {
                    name: "jazz".into(),
                    ..SearchQuery::default()
                },
                20,
            )
            .expect("cached search");
        assert!(cached.offline);
        assert_eq!(cached.items.len(), 1);
        assert!(cached.items[0].favorite);
        drop(server);
    }
}
