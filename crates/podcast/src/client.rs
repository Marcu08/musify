use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use musify_core::{
    ResolvedSource, SourceEntry, SourceError, SourceId, SourceItem, SourcePlayback, SourceProvider,
    SourceQuery, SourceSearch, Track, TrackId,
};
use serde::Deserialize;

use crate::error::PodcastError;
use crate::feed::{self, ParsedEpisode};
use crate::store::{EpisodeRecord, PodcastRecord, PodcastStore};

const USER_AGENT: &str = "Musify/0.1.0 (+https://github.com/Marcu08/musify)";
const DEFAULT_ITUNES_URL: &str = "https://itunes.apple.com";

pub struct PodcastSource {
    store: Arc<PodcastStore>,
    artwork_dir: PathBuf,
    itunes_url: String,
}

#[derive(Debug, Deserialize)]
struct ItunesResponse {
    #[serde(default)]
    results: Vec<ItunesPodcast>,
}

#[derive(Debug, Deserialize)]
struct ItunesPodcast {
    #[serde(rename = "collectionId")]
    collection_id: Option<i64>,
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "feedUrl")]
    feed_url: Option<String>,
    #[serde(rename = "artworkUrl600")]
    artwork_url_600: Option<String>,
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
    #[serde(rename = "primaryGenreName")]
    genre: Option<String>,
}

impl PodcastSource {
    pub fn new(store: Arc<PodcastStore>, artwork_dir: PathBuf) -> Self {
        let itunes_url = std::env::var("MUSIFY_ITUNES_URL")
            .ok()
            .map(|value| value.trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_ITUNES_URL.to_string());
        Self::with_itunes_url(store, artwork_dir, itunes_url)
    }

    pub fn with_itunes_url(
        store: Arc<PodcastStore>,
        artwork_dir: PathBuf,
        itunes_url: String,
    ) -> Self {
        Self {
            store,
            artwork_dir,
            itunes_url,
        }
    }

    pub fn search(&self, query: &SourceQuery) -> Result<SourceSearch, PodcastError> {
        let term = query.text.trim();
        if term.is_empty() {
            return Ok(SourceSearch {
                items: self.favorites()?,
                offline: false,
            });
        }
        let runtime = runtime()?;
        let client = client()?;
        let url = format!("{}/search", self.itunes_url);
        let response = runtime.block_on(async {
            client
                .get(url)
                .query(&[
                    ("term", term.to_string()),
                    ("media", "podcast".to_string()),
                    ("entity", "podcast".to_string()),
                    ("limit", "50".to_string()),
                ])
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
        });
        let response = response.map_err(|error| PodcastError::Network(error.to_string()))?;
        let body = runtime
            .block_on(response.text())
            .map_err(|error| PodcastError::Network(error.to_string()))?;
        let response: ItunesResponse = serde_json::from_str(&body)
            .map_err(|error| PodcastError::Network(format!("invalid iTunes response: {error}")))?;
        let mut records = Vec::new();
        for result in response.results {
            let Some(feed_url) = result.feed_url.filter(|url| !url.trim().is_empty()) else {
                continue;
            };
            let id = result.collection_id.map_or_else(
                || format!("feed-{}", blake3::hash(feed_url.as_bytes()).to_hex()),
                |id| format!("itunes-{id}"),
            );
            let title = result
                .collection_name
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| feed_url.clone());
            records.push(PodcastRecord {
                id,
                feed_url,
                title,
                author: result.artist_name.unwrap_or_default(),
                artwork_url: result
                    .artwork_url_600
                    .or(result.artwork_url_100)
                    .unwrap_or_default(),
                genre: result.genre.unwrap_or_default(),
                ..PodcastRecord::default()
            });
        }
        self.store.upsert_podcasts(&records)?;
        let items = records
            .iter()
            .filter_map(|record| {
                let stored = self.store.podcast(&record.id).ok().flatten()?;
                Some(self.item(&stored))
            })
            .collect();
        Ok(SourceSearch {
            items,
            offline: false,
        })
    }

    pub fn favorites(&self) -> Result<Vec<SourceItem>, PodcastError> {
        Ok(self
            .store
            .subscriptions()?
            .iter()
            .map(|record| self.item(record))
            .collect())
    }

    pub fn toggle_favorite(&self, id: &str) -> Result<bool, PodcastError> {
        let podcast = self
            .store
            .podcast(id)?
            .ok_or_else(|| PodcastError::NotFound(id.to_string()))?;
        let subscribed = !podcast.subscribed;
        self.store.set_subscribed(id, subscribed)?;
        Ok(subscribed)
    }

    pub fn entries(&self, id: &str) -> Result<Vec<SourceEntry>, PodcastError> {
        let podcast = self
            .store
            .podcast(id)?
            .ok_or_else(|| PodcastError::NotFound(id.to_string()))?;
        Ok(self
            .store
            .episodes(id)?
            .iter()
            .map(|episode| self.entry(&podcast, episode))
            .collect())
    }

    pub fn refresh(&self, id: &str) -> Result<Vec<SourceEntry>, PodcastError> {
        let mut podcast = self
            .store
            .podcast(id)?
            .ok_or_else(|| PodcastError::NotFound(id.to_string()))?;
        let runtime = runtime()?;
        let client = client()?;
        let response = runtime.block_on(async {
            client
                .get(&podcast.feed_url)
                .header(
                    reqwest::header::ACCEPT,
                    "application/rss+xml, application/xml, text/xml",
                )
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
        });
        let response = response.map_err(|error| PodcastError::Network(error.to_string()))?;
        if response
            .content_length()
            .is_some_and(|length| length > 8 * 1024 * 1024)
        {
            return Err(PodcastError::Feed("the feed is larger than 8 MB".into()));
        }
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if content_type
            .as_deref()
            .is_some_and(|value| value.to_ascii_lowercase().contains("html"))
        {
            return Err(PodcastError::Feed(
                "the feed URL returned an HTML page".into(),
            ));
        }
        let bytes = runtime
            .block_on(response.bytes())
            .map_err(|error| PodcastError::Network(error.to_string()))?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(PodcastError::Feed("the feed is larger than 8 MB".into()));
        }
        let parsed = feed::parse(id, &bytes, &final_url)?;
        if let Some(title) = parsed.title {
            podcast.title = title;
        }
        if let Some(author) = parsed.author {
            podcast.author = author;
        }
        if let Some(description) = parsed.description {
            podcast.description = description;
        }
        if let Some(website) = parsed.website {
            podcast.website = website;
        }
        if let Some(artwork_url) = parsed.artwork_url {
            podcast.artwork_url = artwork_url;
        }
        podcast.last_refresh = now();
        self.store.upsert_podcast(&podcast)?;
        let episodes: Vec<EpisodeRecord> = parsed
            .entries
            .into_iter()
            .map(|episode| episode_record(id, episode))
            .collect();
        self.store.upsert_episodes(id, &episodes)?;
        self.store.set_refreshed(id, now())?;
        if !podcast.artwork_url.is_empty()
            && let Err(error) = self.artwork(id)
        {
            tracing::warn!(%error, podcast = id, "cannot cache podcast artwork");
        }
        self.entries(id)
    }

    pub fn resolve(&self, episode_id: &str) -> Result<ResolvedSource, PodcastError> {
        let episode = self
            .store
            .episode(episode_id)?
            .ok_or_else(|| PodcastError::NotFound(episode_id.to_string()))?;
        let podcast = self
            .store
            .podcast(&episode.podcast_id)?
            .ok_or_else(|| PodcastError::NotFound(episode.podcast_id.clone()))?;
        if episode.audio_url == podcast.feed_url {
            return Err(PodcastError::Feed("episode has no audio enclosure".into()));
        }
        let duration = episode.duration.unwrap_or_default();
        let cover_path = self.cover_path(&podcast);
        let track = Track {
            id: track_id(episode_id),
            source: SourceId::podcast(episode.audio_url.clone()),
            path: PathBuf::new(),
            title: episode.title,
            artist: if podcast.author.is_empty() {
                podcast.title.clone()
            } else {
                podcast.author.clone()
            },
            album: podcast.title,
            album_artist: None,
            track_number: None,
            disc_number: None,
            duration,
            year: None,
            genre: None,
            cover_path,
            added_at: episode.published_at.unwrap_or_default(),
        };
        Ok(ResolvedSource {
            track,
            playback: SourcePlayback::OnDemand {
                duration,
                seekable: true,
            },
        })
    }

    pub fn resume_position(&self, episode_id: &str) -> Result<Option<Duration>, PodcastError> {
        let Some(episode) = self.store.episode(episode_id)? else {
            return Ok(None);
        };
        if episode.watched || episode.position < Duration::from_secs(3) {
            return Ok(None);
        }
        Ok(Some(episode.position))
    }

    pub fn save_progress(
        &self,
        episode_id: &str,
        position: Duration,
        watched: bool,
    ) -> Result<(), PodcastError> {
        let episode = self
            .store
            .episode(episode_id)?
            .ok_or_else(|| PodcastError::NotFound(episode_id.to_string()))?;
        let near_end = episode
            .duration
            .is_some_and(|duration| position + Duration::from_secs(5) >= duration);
        // Automatic progress must not clear a watched episode; only the
        // explicit Mark unwatched action does that.
        self.store
            .save_progress(episode_id, position, episode.watched || watched || near_end)
    }

    pub fn set_watched(&self, episode_id: &str, watched: bool) -> Result<(), PodcastError> {
        self.store.set_watched(episode_id, watched)
    }

    pub fn artwork(&self, id: &str) -> Result<Option<PathBuf>, PodcastError> {
        let podcast = self
            .store
            .podcast(id)?
            .ok_or_else(|| PodcastError::NotFound(id.to_string()))?;
        if let Some(path) = self.cover_path(&podcast) {
            return Ok(Some(path));
        }
        let url = podcast.artwork_url.trim();
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
        let response = response.map_err(|error| PodcastError::Network(error.to_string()))?;
        if response
            .content_length()
            .is_some_and(|length| length > 2 * 1024 * 1024)
        {
            return Ok(None);
        }
        let bytes = runtime
            .block_on(response.bytes())
            .map_err(|error| PodcastError::Network(error.to_string()))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|error| PodcastError::Feed(format!("invalid artwork: {error}")))?;
        let image = image.resize_to_fill(256, 256, image::imageops::FilterType::Triangle);
        std::fs::create_dir_all(&self.artwork_dir).map_err(storage)?;
        let path = self
            .artwork_dir
            .join(format!("{}.jpg", blake3::hash(url.as_bytes()).to_hex()));
        image
            .save(&path)
            .map_err(|error| PodcastError::Storage(error.to_string()))?;
        prune_artwork(&self.artwork_dir);
        Ok(Some(path))
    }

    fn item(&self, podcast: &PodcastRecord) -> SourceItem {
        SourceItem {
            id: podcast.id.clone(),
            source: SourceId::podcast(podcast.feed_url.clone()),
            title: podcast.title.clone(),
            subtitle: podcast.author.clone(),
            detail: podcast.genre.clone(),
            uri: podcast.feed_url.clone(),
            favorite: podcast.subscribed,
            image_path: self.cover_path(podcast),
            duration: None,
            license_url: None,
        }
    }

    fn entry(&self, podcast: &PodcastRecord, episode: &EpisodeRecord) -> SourceEntry {
        SourceEntry {
            id: episode.id.clone(),
            source: SourceId::podcast(episode.audio_url.clone()),
            title: episode.title.clone(),
            subtitle: entry_subtitle(episode),
            detail: episode.description.clone(),
            uri: episode.audio_url.clone(),
            duration: episode.duration,
            published_at: episode.published_at,
            watched: episode.watched,
            position: (episode.position > Duration::ZERO).then_some(episode.position),
            image_path: self.cover_path(podcast),
        }
    }

    fn cover_path(&self, podcast: &PodcastRecord) -> Option<PathBuf> {
        let url = podcast.artwork_url.trim();
        if url.is_empty() {
            return None;
        }
        let path = self
            .artwork_dir
            .join(format!("{}.jpg", blake3::hash(url.as_bytes()).to_hex()));
        path.exists().then_some(path)
    }
}

impl SourceProvider for PodcastSource {
    fn scheme(&self) -> &'static str {
        "podcast"
    }

    fn search(&self, query: &SourceQuery) -> Result<SourceSearch, SourceError> {
        PodcastSource::search(self, query).map_err(source_error)
    }

    fn favorites(&self) -> Result<Vec<SourceItem>, SourceError> {
        PodcastSource::favorites(self).map_err(source_error)
    }

    fn toggle_favorite(&self, id: &str) -> Result<bool, SourceError> {
        PodcastSource::toggle_favorite(self, id).map_err(source_error)
    }

    fn resolve(&self, id: &str) -> Result<ResolvedSource, SourceError> {
        PodcastSource::resolve(self, id).map_err(source_error)
    }

    fn artwork(&self, id: &str) -> Result<Option<PathBuf>, SourceError> {
        PodcastSource::artwork(self, id).map_err(source_error)
    }

    fn entries(&self, id: &str) -> Result<Vec<SourceEntry>, SourceError> {
        PodcastSource::entries(self, id).map_err(source_error)
    }

    fn refresh(&self, id: &str) -> Result<Vec<SourceEntry>, SourceError> {
        PodcastSource::refresh(self, id).map_err(source_error)
    }

    fn resume_position(&self, id: &str) -> Result<Option<Duration>, SourceError> {
        PodcastSource::resume_position(self, id).map_err(source_error)
    }

    fn save_progress(
        &self,
        id: &str,
        position: Duration,
        watched: bool,
    ) -> Result<(), SourceError> {
        PodcastSource::save_progress(self, id, position, watched).map_err(source_error)
    }

    fn set_watched(&self, id: &str, watched: bool) -> Result<(), SourceError> {
        PodcastSource::set_watched(self, id, watched).map_err(source_error)
    }
}

fn episode_record(podcast_id: &str, episode: ParsedEpisode) -> EpisodeRecord {
    EpisodeRecord {
        id: episode.id,
        podcast_id: podcast_id.to_string(),
        title: episode.title,
        description: episode.description,
        published_at: episode.published_at,
        duration: episode.duration,
        audio_url: episode.audio_url,
        audio_type: episode.audio_type,
        watched: false,
        position: Duration::ZERO,
    }
}

fn entry_subtitle(episode: &EpisodeRecord) -> String {
    let mut parts = Vec::new();
    if let Some(timestamp) = episode.published_at
        && let Some(date) = DateTime::<Utc>::from_timestamp(timestamp, 0)
    {
        parts.push(date.format("%Y-%m-%d").to_string());
    }
    if let Some(duration) = episode.duration {
        parts.push(duration_label(duration));
    }
    if episode.watched {
        parts.push("ascoltato".to_string());
    }
    if parts.is_empty() {
        "Episodio".to_string()
    } else {
        parts.join(" · ")
    }
}

fn duration_label(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    if minutes >= 60 {
        format!("{} h {} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

fn track_id(id: &str) -> TrackId {
    let hash = blake3::hash(id.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    TrackId::new((i64::from_le_bytes(bytes) & i64::MAX).max(1))
}

fn source_error(error: PodcastError) -> SourceError {
    match error {
        PodcastError::Network(message) | PodcastError::Feed(message) => {
            SourceError::Network(message)
        }
        PodcastError::Storage(message) => SourceError::Storage(message),
        PodcastError::NotFound(message) => SourceError::NotFound(message),
    }
}

fn storage(error: std::io::Error) -> PodcastError {
    PodcastError::Storage(error.to_string())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn runtime() -> Result<tokio::runtime::Runtime, PodcastError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| PodcastError::Network(error.to_string()))
}

fn client() -> Result<reqwest::Client, PodcastError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| PodcastError::Network(error.to_string()))
}

fn prune_artwork(dir: &Path) {
    const MAX_FILES: usize = 64;
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
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
    use std::sync::Arc;

    use image::{DynamicImage, ImageFormat};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const FEED: &str = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
  <channel>
    <title>Fake Podcast</title>
    <description>Feed di test</description>
    <itunes:author>Fake Author</itunes:author>
    <item>
      <title>Episode One</title>
      <guid>one</guid>
      <description>Descrizione</description>
      <pubDate>Tue, 10 Nov 2026 10:00:00 GMT</pubDate>
      <itunes:duration>00:30:00</itunes:duration>
      <enclosure url="EPISODE_URL" type="audio/mpeg" length="100"/>
    </item>
  </channel>
</rss>"#;

    fn jpeg() -> Vec<u8> {
        let image = DynamicImage::new_rgb8(8, 8);
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("jpeg");
        bytes
    }

    #[test]
    fn searches_subscribes_refreshes_and_resolves_against_fake_servers() {
        let runtime = runtime().expect("runtime");
        let server = runtime.block_on(MockServer::start());
        let feed = FEED.replace("EPISODE_URL", &format!("{}/episode.mp3", server.uri()));
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/search"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "results": [{
                        "collectionId": 42,
                        "collectionName": "Fake Podcast",
                        "artistName": "Fake Author",
                        "feedUrl": format!("{}/redirect.xml", server.uri()),
                        "artworkUrl600": format!("{}/cover.jpg", server.uri()),
                        "primaryGenreName": "Technology"
                    }]
                })))
                .mount(&server),
        );
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/redirect.xml"))
                .respond_with(
                    ResponseTemplate::new(302)
                        .insert_header("Location", format!("{}/feed.xml", server.uri())),
                )
                .mount(&server),
        );
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/feed.xml"))
                .respond_with(ResponseTemplate::new(200).set_body_string(feed))
                .mount(&server),
        );
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/cover.jpg"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(jpeg()))
                .mount(&server),
        );
        drop(runtime);

        let dir = tempfile::tempdir().expect("temp dir");
        let store = Arc::new(PodcastStore::open_in_memory().expect("store"));
        let source = PodcastSource::with_itunes_url(
            Arc::clone(&store),
            dir.path().join("covers"),
            server.uri(),
        );
        let provider: &dyn SourceProvider = &source;
        let search = provider
            .search(&SourceQuery {
                text: "fake".into(),
                ..SourceQuery::default()
            })
            .expect("search");
        assert_eq!(search.items.len(), 1);
        let id = search.items[0].id.clone();
        assert_eq!(search.items[0].title, "Fake Podcast");
        assert!(provider.toggle_favorite(&id).expect("subscribe"));
        assert_eq!(provider.favorites().expect("favorites").len(), 1);
        assert_eq!(
            provider.artwork(&id).expect("artwork"),
            Some(dir.path().join("covers").join(format!(
                "{}.jpg",
                blake3::hash(format!("{}/cover.jpg", server.uri()).as_bytes()).to_hex()
            )))
        );
        let entries = provider.refresh(&id).expect("refresh");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Episode One");
        assert_eq!(entries[0].duration, Some(Duration::from_secs(1800)));
        let episode_id = entries[0].id.clone();
        let resolved = provider.resolve(&episode_id).expect("resolve");
        assert_eq!(
            resolved.playback,
            SourcePlayback::OnDemand {
                duration: Duration::from_secs(1800),
                seekable: true,
            }
        );
        assert_eq!(resolved.track.source.scheme(), "podcast");
        provider
            .save_progress(&episode_id, Duration::from_secs(120), false)
            .expect("progress");
        assert_eq!(
            provider.resume_position(&episode_id).expect("resume"),
            Some(Duration::from_secs(120))
        );
        provider.set_watched(&episode_id, true).expect("watched");
        assert_eq!(provider.resume_position(&episode_id).expect("resume"), None);
        assert!(provider.refresh("missing").is_err());
    }

    #[test]
    #[ignore = "real network; run manually"]
    fn real_itunes_search_and_feed_smoke() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Arc::new(PodcastStore::open_in_memory().expect("store"));
        let source = PodcastSource::new(Arc::clone(&store), dir.path().join("covers"));
        let search = source
            .search(&SourceQuery {
                text: "The Daily".into(),
                ..SourceQuery::default()
            })
            .expect("real iTunes search");
        let item = search.items.first().expect("at least one podcast");
        println!("podcast: {} ({})", item.title, item.id);
        source
            .toggle_favorite(&item.id)
            .expect("subscribe real podcast");
        let entries = source.refresh(&item.id).expect("real feed refresh");
        assert!(!entries.is_empty(), "the real feed should have episodes");
        assert_ne!(
            entries[0].uri, item.uri,
            "enclosure must not be the feed URL"
        );
        println!(
            "episodes: {}; first: {} ({:?})",
            entries.len(),
            entries[0].title,
            entries[0].duration
        );
    }

    #[test]
    fn a_malformed_feed_is_reported() {
        let runtime = runtime().expect("runtime");
        let server = runtime.block_on(MockServer::start());
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/feed.xml"))
                .respond_with(ResponseTemplate::new(200).set_body_string("<rss><channel>"))
                .mount(&server),
        );
        drop(runtime);
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Arc::new(PodcastStore::open_in_memory().expect("store"));
        store
            .upsert_podcast(&PodcastRecord {
                id: "podcast-1".into(),
                feed_url: format!("{}/feed.xml", server.uri()),
                title: "Broken".into(),
                ..PodcastRecord::default()
            })
            .expect("podcast");
        let source = PodcastSource::with_itunes_url(store, dir.path().join("covers"), server.uri());
        let error = source.refresh("podcast-1").expect_err("malformed feed");
        assert!(error.to_string().contains("invalid podcast feed"));
    }
}
