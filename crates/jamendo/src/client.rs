use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use musify_core::{
    ResolvedSource, SourceError, SourceId, SourceItem, SourcePlayback, SourceProvider, SourceQuery,
    SourceSearch, Track,
};
use serde::Deserialize;

use crate::error::JamendoError;
use crate::model::JamendoTrack;
use crate::store::JamendoStore;

const USER_AGENT: &str = "Musify/0.1.0 (+https://github.com/Marcu08/musify)";
const DEFAULT_API_URL: &str = "https://api.jamendo.com/v3.0";
const MAX_ATTEMPTS: u32 = 3;

/// Runtime knobs. Production uses 1 request/second, a 1 s retry base and a
/// 24 h result cache; tests use a fake server and a tiny interval.
#[derive(Debug, Clone)]
pub struct JamendoOptions {
    pub base_url: String,
    pub min_interval: Duration,
    pub retry_base: Duration,
    pub cache_ttl_secs: i64,
}

impl Default for JamendoOptions {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_API_URL.to_string(),
            min_interval: Duration::from_secs(1),
            retry_base: Duration::from_secs(1),
            cache_ttl_secs: 24 * 3600,
        }
    }
}

pub struct JamendoSource {
    store: Arc<JamendoStore>,
    artwork_dir: PathBuf,
    client_id: String,
    options: JamendoOptions,
    limiter: RateLimiter,
}

impl JamendoSource {
    /// Builds the source only when `MUSIFY_JAMENDO_CLIENT_ID` is set. The
    /// client id is never logged, stored or exposed in an error message.
    #[must_use]
    pub fn from_env(store: Arc<JamendoStore>, artwork_dir: PathBuf) -> Option<Self> {
        let client_id = std::env::var("MUSIFY_JAMENDO_CLIENT_ID")
            .ok()?
            .trim()
            .to_string();
        if client_id.is_empty() {
            return None;
        }
        let mut options = JamendoOptions::default();
        if let Ok(url) = std::env::var("MUSIFY_JAMENDO_API_URL") {
            let url = url.trim().trim_end_matches('/').to_string();
            if !url.is_empty() {
                options.base_url = url;
            }
        }
        if let Ok(hours) = std::env::var("MUSIFY_JAMENDO_CACHE_TTL_HOURS")
            && let Ok(hours) = hours.parse::<i64>()
        {
            options.cache_ttl_secs = hours.clamp(1, 24 * 7) * 3600;
        }
        Some(Self::with_options(store, artwork_dir, client_id, options))
    }

    #[must_use]
    pub fn with_options(
        store: Arc<JamendoStore>,
        artwork_dir: PathBuf,
        client_id: String,
        options: JamendoOptions,
    ) -> Self {
        let limiter = RateLimiter::new(options.min_interval);
        Self {
            store,
            artwork_dir,
            client_id,
            options,
            limiter,
        }
    }

    pub fn search(&self, query: &SourceQuery) -> Result<SourceSearch, JamendoError> {
        let params = SearchParams {
            text: query.text.trim().to_string(),
            artist: query
                .filter("artist")
                .unwrap_or_default()
                .trim()
                .to_string(),
            tag: query.filter("tag").unwrap_or_default().trim().to_string(),
            name: query.filter("name").unwrap_or_default().trim().to_string(),
        };
        if params.is_empty() {
            return Ok(SourceSearch {
                items: self.favorites()?,
                offline: false,
            });
        }
        let key = params.cache_key();
        match self.fetch_tracks(&params) {
            Ok(tracks) => {
                self.store.upsert_tracks(&tracks)?;
                self.store.store_search(&key, &tracks)?;
                Ok(SourceSearch {
                    items: self.items(&tracks)?,
                    offline: false,
                })
            }
            Err(error) => match self
                .store
                .cached_search(&key, self.options.cache_ttl_secs)?
            {
                Some(tracks) => Ok(SourceSearch {
                    items: self.items(&tracks)?,
                    offline: true,
                }),
                None => Err(error),
            },
        }
    }

    pub fn favorites(&self) -> Result<Vec<SourceItem>, JamendoError> {
        let tracks = self.store.favorites()?;
        self.items(&tracks)
    }

    pub fn toggle_favorite(&self, id: &str) -> Result<bool, JamendoError> {
        self.store
            .track(id)?
            .ok_or_else(|| JamendoError::NotFound(id.to_string()))?;
        let favorite = !self.store.is_favorite(id)?;
        self.store.set_favorite(id, favorite)?;
        Ok(favorite)
    }

    pub fn resolve(&self, id: &str) -> Result<ResolvedSource, JamendoError> {
        let track = self
            .store
            .track(id)?
            .ok_or_else(|| JamendoError::NotFound(id.to_string()))?;
        let duration = track.duration();
        let cover_path = self.cover_path(&track);
        let artist = if track.artist_name.trim().is_empty() {
            "Jamendo".to_string()
        } else {
            track.artist_name.clone()
        };
        let title = if track.name.trim().is_empty() {
            format!("Jamendo track {}", track.id)
        } else {
            track.name.clone()
        };
        Ok(ResolvedSource {
            track: Track {
                id: track.track_id(),
                source: SourceId::jamendo(track.audio_url.clone()),
                path: PathBuf::new(),
                title,
                artist,
                album: track.display_album(),
                album_artist: None,
                track_number: None,
                disc_number: None,
                duration,
                year: None,
                genre: Some(track.license_label().to_string()),
                cover_path,
                added_at: 0,
            },
            playback: SourcePlayback::OnDemand {
                duration,
                seekable: true,
            },
        })
    }

    pub fn artwork(&self, id: &str) -> Result<Option<PathBuf>, JamendoError> {
        let track = self
            .store
            .track(id)?
            .ok_or_else(|| JamendoError::NotFound(id.to_string()))?;
        self.cache_artwork(&track)
    }

    fn items(&self, tracks: &[JamendoTrack]) -> Result<Vec<SourceItem>, JamendoError> {
        let favorites = self.store.favorite_ids()?;
        Ok(tracks
            .iter()
            .map(|track| self.item(track, favorites.contains(&track.id)))
            .collect())
    }

    fn item(&self, track: &JamendoTrack, favorite: bool) -> SourceItem {
        let subtitle = if track.album_name.trim().is_empty() {
            track.artist_name.clone()
        } else {
            format!("{} · {}", track.artist_name, track.album_name)
        };
        SourceItem {
            id: track.id.clone(),
            source: SourceId::jamendo(track.audio_url.clone()),
            title: track.name.clone(),
            subtitle,
            detail: track.license_label().to_string(),
            uri: track.audio_url.clone(),
            favorite,
            image_path: self.cover_path(track),
            duration: Some(track.duration()),
            license_url: Some(track.license_url.clone()).filter(|url| !url.trim().is_empty()),
        }
    }

    fn fetch_tracks(&self, params: &SearchParams) -> Result<Vec<JamendoTrack>, JamendoError> {
        let runtime = runtime()?;
        let client = client()?;
        let url = format!("{}/tracks/", self.options.base_url.trim_end_matches('/'));
        let mut attempt = 0;
        loop {
            self.limiter.wait();
            let mut query = vec![
                ("client_id".to_string(), self.client_id.clone()),
                ("format".to_string(), "json".to_string()),
                ("limit".to_string(), "100".to_string()),
                ("audioformat".to_string(), "mp32".to_string()),
                ("imagesize".to_string(), "300".to_string()),
                ("include".to_string(), "licenses".to_string()),
            ];
            if !params.text.is_empty() {
                query.push(("search".to_string(), params.text.clone()));
            }
            if !params.artist.is_empty() {
                query.push(("artist_name".to_string(), params.artist.clone()));
            }
            if !params.tag.is_empty() {
                query.push(("tags".to_string(), params.tag.clone()));
            }
            if !params.name.is_empty() {
                query.push(("namesearch".to_string(), params.name.clone()));
            }
            let response = runtime.block_on(async { client.get(&url).query(&query).send().await });
            match response {
                Ok(response) if response.status().is_success() => {
                    let body = runtime.block_on(response.text()).map_err(|_| {
                        JamendoError::Network("cannot read Jamendo response".into())
                    })?;
                    return parse_response(&body).map_err(|error| self.redact(error));
                }
                Ok(response)
                    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                        || response.status().is_server_error() =>
                {
                    if attempt + 1 >= MAX_ATTEMPTS {
                        return Err(JamendoError::Network(
                            "Jamendo rate limit or server error".into(),
                        ));
                    }
                    let delay = self
                        .options
                        .retry_base
                        .saturating_mul(1u32 << attempt)
                        .min(Duration::from_secs(8));
                    std::thread::sleep(delay);
                    attempt += 1;
                }
                Ok(response) => {
                    return Err(JamendoError::Network(format!(
                        "Jamendo HTTP {}",
                        response.status().as_u16()
                    )));
                }
                Err(error) => {
                    if attempt + 1 >= MAX_ATTEMPTS {
                        return Err(network_error(error));
                    }
                    let delay = self
                        .options
                        .retry_base
                        .saturating_mul(1u32 << attempt)
                        .min(Duration::from_secs(8));
                    std::thread::sleep(delay);
                    attempt += 1;
                }
            }
        }
    }

    fn redact(&self, error: JamendoError) -> JamendoError {
        match error {
            JamendoError::Network(message) => {
                JamendoError::Network(redact_client(&message, &self.client_id))
            }
            JamendoError::Api(message) => {
                JamendoError::Api(redact_client(&message, &self.client_id))
            }
            other => other,
        }
    }

    fn cover_path(&self, track: &JamendoTrack) -> Option<PathBuf> {
        let url = track.image_url.trim();
        if url.is_empty() {
            return None;
        }
        let path = self
            .artwork_dir
            .join(format!("{}.jpg", blake3::hash(url.as_bytes()).to_hex()));
        path.exists().then_some(path)
    }

    fn cache_artwork(&self, track: &JamendoTrack) -> Result<Option<PathBuf>, JamendoError> {
        if let Some(path) = self.cover_path(track) {
            return Ok(Some(path));
        }
        let url = track.image_url.trim();
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
        let response =
            response.map_err(|_| JamendoError::Network("cannot download Jamendo cover".into()))?;
        if response
            .content_length()
            .is_some_and(|length| length > 2 * 1024 * 1024)
        {
            return Ok(None);
        }
        let bytes = runtime
            .block_on(response.bytes())
            .map_err(|_| JamendoError::Network("cannot read Jamendo cover".into()))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|error| JamendoError::Storage(format!("invalid Jamendo cover: {error}")))?;
        let image = image.resize_to_fill(256, 256, image::imageops::FilterType::Triangle);
        std::fs::create_dir_all(&self.artwork_dir)
            .map_err(|error| JamendoError::Storage(error.to_string()))?;
        let path = self
            .artwork_dir
            .join(format!("{}.jpg", blake3::hash(url.as_bytes()).to_hex()));
        image
            .save(&path)
            .map_err(|error| JamendoError::Storage(error.to_string()))?;
        prune_artwork(&self.artwork_dir);
        Ok(Some(path))
    }
}

impl SourceProvider for JamendoSource {
    fn scheme(&self) -> &'static str {
        "jamendo"
    }

    fn search(&self, query: &SourceQuery) -> Result<SourceSearch, SourceError> {
        JamendoSource::search(self, query).map_err(source_error)
    }

    fn favorites(&self) -> Result<Vec<SourceItem>, SourceError> {
        JamendoSource::favorites(self).map_err(source_error)
    }

    fn toggle_favorite(&self, id: &str) -> Result<bool, SourceError> {
        JamendoSource::toggle_favorite(self, id).map_err(source_error)
    }

    fn resolve(&self, id: &str) -> Result<ResolvedSource, SourceError> {
        JamendoSource::resolve(self, id).map_err(source_error)
    }

    fn artwork(&self, id: &str) -> Result<Option<PathBuf>, SourceError> {
        JamendoSource::artwork(self, id).map_err(source_error)
    }
}

#[derive(Debug, Clone, Default)]
struct SearchParams {
    text: String,
    artist: String,
    tag: String,
    name: String,
}

impl SearchParams {
    fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.artist.is_empty()
            && self.tag.is_empty()
            && self.name.is_empty()
    }

    fn cache_key(&self) -> String {
        let raw = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.text, self.artist, self.tag, self.name
        );
        blake3::hash(raw.as_bytes()).to_hex().to_string()
    }
}

#[derive(Debug, Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    headers: ApiHeaders,
    #[serde(default)]
    results: Vec<ApiTrack>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiHeaders {
    #[serde(default)]
    status: String,
    #[serde(default)]
    code: i32,
    #[serde(default)]
    error_message: String,
}

#[derive(Debug, Deserialize, Default)]
struct ApiTrack {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    album_name: String,
    #[serde(default)]
    album_image: String,
    #[serde(default)]
    audio: String,
    #[serde(default)]
    license_ccurl: String,
    #[serde(default)]
    shareurl: String,
}

fn parse_response(body: &str) -> Result<Vec<JamendoTrack>, JamendoError> {
    let response: ApiResponse = serde_json::from_str(body)
        .map_err(|_| JamendoError::Network("invalid Jamendo response".into()))?;
    if response.headers.status != "success" || response.headers.code != 0 {
        let message = if response.headers.error_message.trim().is_empty() {
            "Jamendo request rejected".to_string()
        } else {
            response.headers.error_message
        };
        return Err(JamendoError::Api(message));
    }
    Ok(response
        .results
        .into_iter()
        .filter_map(|item| {
            let track = JamendoTrack {
                id: item.id.trim().to_string(),
                name: item.name.trim().to_string(),
                artist_name: item.artist_name.trim().to_string(),
                album_name: item.album_name.trim().to_string(),
                duration_secs: item.duration,
                audio_url: item.audio.trim().to_string(),
                image_url: item.album_image.trim().to_string(),
                license_url: item.license_ccurl.trim().to_string(),
                share_url: item.shareurl.trim().to_string(),
            };
            track.is_playable().then_some(track)
        })
        .collect())
}

fn redact_client(message: &str, client_id: &str) -> String {
    if client_id.is_empty() {
        message.to_string()
    } else {
        message.replace(client_id, "[client id]")
    }
}

fn source_error(error: JamendoError) -> SourceError {
    match error {
        JamendoError::Network(message) | JamendoError::Api(message) => {
            SourceError::Network(message)
        }
        JamendoError::Storage(message) => SourceError::Storage(message),
        JamendoError::NotFound(message) => SourceError::NotFound(message),
    }
}

fn runtime() -> Result<tokio::runtime::Runtime, JamendoError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| JamendoError::Network("cannot start Jamendo runtime".into()))
}

fn client() -> Result<reqwest::Client, JamendoError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| JamendoError::Network("cannot build Jamendo HTTP client".into()))
}

/// Avoids `reqwest::Error`'s Display, which can include the request URL and
/// therefore the `client_id` query parameter.
fn network_error(error: reqwest::Error) -> JamendoError {
    if error.is_timeout() {
        JamendoError::Network("Jamendo request timed out".into())
    } else if error.is_connect() {
        JamendoError::Network("cannot connect to Jamendo".into())
    } else {
        JamendoError::Network("Jamendo network request failed".into())
    }
}

struct RateLimiter {
    interval: Duration,
    last: Mutex<Option<Instant>>,
}

impl RateLimiter {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: Mutex::new(None),
        }
    }

    fn wait(&self) {
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < self.interval {
                std::thread::sleep(self.interval - elapsed);
            }
        }
        *last = Some(Instant::now());
    }
}

fn prune_artwork(dir: &Path) {
    const MAX_FILES: usize = 64;
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, std::time::SystemTime, u64)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            Some((entry.path(), metadata.modified().ok()?, metadata.len()))
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::io::Cursor;

    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    fn client_id() -> String {
        format!("synthetic-client-{}", std::process::id())
    }

    fn source(server: &MockServer, store: Arc<JamendoStore>, client_id: String) -> JamendoSource {
        JamendoSource::with_options(
            store,
            tempfile::tempdir().expect("artwork dir").keep(),
            client_id,
            JamendoOptions {
                base_url: server.uri(),
                min_interval: Duration::ZERO,
                retry_base: Duration::from_millis(1),
                cache_ttl_secs: 3600,
            },
        )
    }

    fn response() -> serde_json::Value {
        json!({
            "headers": {"status": "success", "code": 0, "error_message": ""},
            "results": [{
                "id": "42",
                "name": "Track One",
                "duration": 180,
                "artist_name": "Artist",
                "album_name": "Album",
                "album_image": "https://img.example/42.jpg",
                "audio": "https://audio.example/42.mp3",
                "license_ccurl": "http://creativecommons.org/licenses/by-sa/3.0/",
                "shareurl": "https://www.jamendo.com/track/42",
                "audiodownload": "https://download.example/42.mp3",
                "audiodownload_allowed": true
            }]
        })
    }

    #[test]
    fn search_parses_metadata_and_ignores_download_fields() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .and(query_param("format", "json"))
                .and(query_param("search", "jazz"))
                .and(query_param("tags", "lounge"))
                .and(query_param("audioformat", "mp32"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, Arc::clone(&store), client.clone());
        let outcome = source
            .search(&SourceQuery {
                text: "jazz".into(),
                filters: vec![("tag".into(), "lounge".into())],
            })
            .expect("search");
        assert!(!outcome.offline);
        assert_eq!(outcome.items.len(), 1);
        let item = &outcome.items[0];
        assert_eq!(item.title, "Track One");
        assert_eq!(item.subtitle, "Artist · Album");
        assert_eq!(item.detail, "CC BY-SA");
        assert_eq!(item.duration, Some(Duration::from_secs(180)));
        assert_eq!(item.uri, "https://audio.example/42.mp3");
        assert_eq!(
            item.license_url.as_deref(),
            Some("http://creativecommons.org/licenses/by-sa/3.0/")
        );
        let cached = store
            .cached_search(
                &SearchParams {
                    text: "jazz".into(),
                    tag: "lounge".into(),
                    ..SearchParams::default()
                }
                .cache_key(),
                3600,
            )
            .expect("cache")
            .expect("cached tracks");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].license_label(), "CC BY-SA");
        let resolved = source.resolve("42").expect("resolve");
        assert_eq!(resolved.track.source.scheme(), "jamendo");
        assert_eq!(resolved.track.source.uri(), "https://audio.example/42.mp3");
        assert!(resolved.track.is_on_demand());
        assert_eq!(resolved.track.album, "Album · CC BY-SA · Jamendo");
        assert_eq!(resolved.track.genre.as_deref(), Some("CC BY-SA"));
        assert_eq!(
            resolved.playback,
            SourcePlayback::OnDemand {
                duration: Duration::from_secs(180),
                seekable: true,
            }
        );
    }

    #[test]
    fn search_falls_back_to_a_fresh_cache_when_the_server_is_down() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let online = source(&server, Arc::clone(&store), client.clone());
        online
            .search(&SourceQuery {
                text: "jazz".into(),
                ..SourceQuery::default()
            })
            .expect("online search");
        let offline = JamendoSource::with_options(
            Arc::clone(&store),
            dir.path().join("covers"),
            client,
            JamendoOptions {
                base_url: "http://127.0.0.1:1".into(),
                min_interval: Duration::ZERO,
                retry_base: Duration::from_millis(1),
                cache_ttl_secs: 3600,
            },
        );
        let outcome = offline
            .search(&SourceQuery {
                text: "jazz".into(),
                ..SourceQuery::default()
            })
            .expect("cached search");
        assert!(outcome.offline);
        assert_eq!(outcome.items.len(), 1);
    }

    #[test]
    fn rate_limit_retries_then_succeeds() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .respond_with(ResponseTemplate::new(429))
                .up_to_n_times(1)
                .expect(1)
                .mount(&server),
        );
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client);
        let outcome = source
            .search(&SourceQuery {
                text: "jazz".into(),
                ..SourceQuery::default()
            })
            .expect("search after retry");
        assert_eq!(outcome.items.len(), 1);
    }

    #[test]
    fn rate_limit_exhaustion_does_not_leak_the_client_id() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .respond_with(ResponseTemplate::new(429))
                .expect(3)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client.clone());
        let error = source
            .search(&SourceQuery {
                text: "jazz".into(),
                ..SourceQuery::default()
            })
            .expect_err("rate limit");
        let message = error.to_string();
        assert!(message.contains("rate limit"));
        assert!(!message.contains(&client));
    }

    #[test]
    fn api_error_message_is_redacted() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "headers": {
                        "status": "failed",
                        "code": 1,
                        "error_message": format!("bad client {client}")
                    },
                    "results": []
                })))
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client.clone());
        let error = source
            .search(&SourceQuery {
                text: "jazz".into(),
                ..SourceQuery::default()
            })
            .expect_err("api error");
        let message = error.to_string();
        assert!(message.contains("[client id]"));
        assert!(!message.contains(&client));
    }

    #[test]
    fn favorites_persist_and_resolve_is_db_only() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, Arc::clone(&store), client);
        source
            .search(&SourceQuery {
                text: "jazz".into(),
                ..SourceQuery::default()
            })
            .expect("search");
        assert!(source.toggle_favorite("42").expect("favorite"));
        assert_eq!(source.favorites().expect("favorites").len(), 1);
        let resolved = source.resolve("42").expect("resolve");
        assert_eq!(resolved.track.title, "Track One");
        assert!(source.favorites().expect("favorites")[0].favorite);
        drop(server);
        let resolved_offline = source.resolve("42").expect("resolve offline");
        assert_eq!(resolved_offline.track.source.scheme(), "jamendo");
    }

    #[test]
    fn artwork_is_downloaded_and_cached() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([255, 0, 0, 255]),
        ))
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("png");
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/cover.jpg"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(png))
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        store
            .upsert_tracks(&[JamendoTrack {
                id: "42".into(),
                name: "Track".into(),
                artist_name: "Artist".into(),
                album_name: "Album".into(),
                duration_secs: 180,
                audio_url: "https://audio.example/42.mp3".into(),
                image_url: format!("{}/cover.jpg", server.uri()),
                license_url: String::new(),
                share_url: String::new(),
            }])
            .expect("track");
        let source = JamendoSource::with_options(
            store,
            dir.path().join("covers"),
            client_id(),
            JamendoOptions {
                base_url: server.uri(),
                min_interval: Duration::ZERO,
                retry_base: Duration::from_millis(1),
                cache_ttl_secs: 3600,
            },
        );
        let path = source.artwork("42").expect("artwork").expect("cached");
        assert!(path.exists());
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("jpg"));
    }
}
