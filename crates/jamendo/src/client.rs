use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use musify_core::{
    ResolvedSource, SourceError, SourceId, SourceItem, SourcePlayback, SourceProvider, SourceQuery,
    SourceSearch, Track,
};
use serde::Deserialize;
use serde::de::Deserializer;

use crate::error::JamendoError;
use crate::model::{JamendoSelection, JamendoTrack};
use crate::store::JamendoStore;

const USER_AGENT: &str = "Musify/0.1.0 (+https://github.com/Marcu08/musify)";
const DEFAULT_API_URL: &str = "https://api.jamendo.com/v3.0";
const MAX_ATTEMPTS: u32 = 3;
const DEFAULT_PAGE_SIZE: usize = 60;
const MAX_PAGE_SIZE: usize = 200;
const EMPTY_RETRIES: u32 = 1;
const PREFETCH_GENRES: &[&str] = &["jazz", "electronic", "rock", "pop"];
const HOT_SELECTION_IDS: usize = 8;
const MAX_SELECTIONS: usize = 4;

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
    /// Incremented by every user search and by `cancel`. A worker keeps its
    /// own generation and stops as soon as it no longer matches.
    generation: AtomicU64,
    prefetch_started: AtomicBool,
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
            generation: AtomicU64::new(0),
            prefetch_started: AtomicBool::new(false),
        }
    }

    pub fn search(&self, query: &SourceQuery) -> Result<SourceSearch, JamendoError> {
        let params = SearchParams::from_query(query);
        if params.is_empty() {
            return Ok(SourceSearch {
                items: self.favorites()?,
                offline: false,
                cached: false,
                has_more: false,
            });
        }

        let generation = self.begin_request();
        if let Some(tag) = params.usage_tag() {
            // A local counter only; no data is sent to Jamendo for this.
            let _ = self.store.record_genre_use(tag);
        }
        if !params.selection_id.is_empty() {
            return self.search_selection(&params, generation);
        }

        let key = params.cache_key();
        if !params.refresh
            && let Some(tracks) = self
                .store
                .cached_search(&key, self.options.cache_ttl_secs)?
        {
            return Ok(SourceSearch {
                items: self.items(&tracks)?,
                offline: false,
                cached: true,
                has_more: tracks.len() >= params.limit,
            });
        }

        match self.fetch_tracks(&params, generation) {
            Ok(tracks) if !tracks.is_empty() => {
                self.store.upsert_tracks(&tracks)?;
                self.store.store_search(&key, &tracks)?;
                Ok(SourceSearch {
                    items: self.items(&tracks)?,
                    offline: false,
                    cached: false,
                    has_more: tracks.len() >= params.limit,
                })
            }
            Ok(_) => {
                if params.is_browse()
                    && let Some(tracks) = self.store.cached_search(&key, i64::MAX)?
                {
                    return Ok(SourceSearch {
                        items: self.items(&tracks)?,
                        offline: true,
                        cached: true,
                        has_more: tracks.len() >= params.limit,
                    });
                }
                Ok(SourceSearch {
                    items: Vec::new(),
                    offline: false,
                    cached: false,
                    has_more: false,
                })
            }
            Err(error) => match self.store.cached_search(&key, i64::MAX)? {
                Some(tracks) => Ok(SourceSearch {
                    items: self.items(&tracks)?,
                    offline: true,
                    cached: true,
                    has_more: tracks.len() >= params.limit,
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

    /// Curated playlists from Jamendo's hot selection. The endpoint returns the
    /// playlist ids first, then the playlists with their tracks. Both responses
    /// are cached for the usual TTL, so opening the view again is a DB read.
    pub fn selections(&self) -> Result<Vec<SourceItem>, JamendoError> {
        let generation = self.generation.load(Ordering::Acquire);
        if let Some(selections) = self.store.cached_selections(self.options.cache_ttl_secs)? {
            return Ok(self.selection_items(&selections));
        }
        match self.fetch_selections(generation) {
            Ok(selections) => Ok(self.selection_items(&selections)),
            Err(error) => match self.store.cached_selections(i64::MAX)? {
                Some(selections) => Ok(self.selection_items(&selections)),
                None => Err(error),
            },
        }
    }

    /// Warms the first page of the most used genres. It is called once by the
    /// application after startup; `prefetch_started` makes it idempotent. A user
    /// search increments the generation and stops the remaining warm-up.
    pub fn prefetch(&self) {
        if self.prefetch_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let generation = self.generation.load(Ordering::Acquire);
        let mut genres = self.store.popular_genres(4).unwrap_or_default();
        if genres.is_empty() {
            genres = PREFETCH_GENRES
                .iter()
                .map(|tag| (*tag).to_string())
                .collect();
        }
        for tag in genres {
            if self.is_cancelled(generation) {
                break;
            }
            let params = SearchParams {
                tag,
                order: Some(SortOrder::PopularWeek),
                limit: DEFAULT_PAGE_SIZE,
                ..SearchParams::default()
            };
            if self
                .store
                .cached_search(&params.cache_key(), self.options.cache_ttl_secs)
                .ok()
                .flatten()
                .is_some()
            {
                continue;
            }
            if let Ok(tracks) = self.fetch_tracks(&params, generation)
                && !tracks.is_empty()
            {
                let _ = self.store.upsert_tracks(&tracks);
                let _ = self.store.store_search(&params.cache_key(), &tracks);
            }
        }
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    fn begin_request(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn is_cancelled(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) != generation
    }

    fn search_selection(
        &self,
        params: &SearchParams,
        generation: u64,
    ) -> Result<SourceSearch, JamendoError> {
        let selection = match self.store.selection(&params.selection_id)? {
            Some(selection) => selection,
            None => {
                self.fetch_selections(generation)?;
                self.store
                    .selection(&params.selection_id)?
                    .ok_or_else(|| JamendoError::NotFound(params.selection_id.clone()))?
            }
        };
        let total = selection.tracks.len();
        let start = params.offset.min(total);
        let end = (start + params.limit).min(total);
        let page = selection.tracks[start..end].to_vec();
        Ok(SourceSearch {
            items: self.items(&page)?,
            offline: false,
            cached: true,
            has_more: end < total,
        })
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

    fn selection_items(&self, selections: &[JamendoSelection]) -> Vec<SourceItem> {
        selections
            .iter()
            .map(|selection| SourceItem {
                id: selection.id.clone(),
                source: SourceId::jamendo(""),
                title: selection.name.clone(),
                subtitle: selection.user_name.clone(),
                detail: String::new(),
                uri: String::new(),
                favorite: false,
                image_path: selection
                    .tracks
                    .first()
                    .and_then(|track| self.cover_path(track)),
                duration: None,
                license_url: None,
            })
            .collect()
    }

    fn fetch_tracks(
        &self,
        params: &SearchParams,
        generation: u64,
    ) -> Result<Vec<JamendoTrack>, JamendoError> {
        let url = format!("{}/tracks/", self.options.base_url.trim_end_matches('/'));
        let query = self.track_query(params);
        let mut empty_retries = 0;
        loop {
            let body = self.get(&url, &query, generation)?;
            let tracks = parse_tracks(&body).map_err(|error| self.redact(error))?;
            // The live API occasionally answers `success` with zero rows for a
            // genre query that normally has results. A single retry is cheap and
            // keeps a cold genre click from showing a false empty list.
            if tracks.is_empty() && params.is_browse() && empty_retries < EMPTY_RETRIES {
                empty_retries += 1;
                if !sleep_cancellable(self.options.retry_base, generation, &self.generation) {
                    return Err(JamendoError::Cancelled);
                }
                continue;
            }
            return Ok(tracks);
        }
    }

    fn track_query(&self, params: &SearchParams) -> Vec<(String, String)> {
        let mut query = vec![
            ("client_id".to_string(), self.client_id.clone()),
            ("format".to_string(), "json".to_string()),
            ("limit".to_string(), params.limit.to_string()),
            ("offset".to_string(), params.offset.to_string()),
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
        if let Some(order) = params.order {
            // With free text, `order` destroys search relevance; the docs
            // recommend `boost` for rating sorts. "Newest" is the exception:
            // the user explicitly asked for release date order.
            if !params.text.is_empty() && order != SortOrder::Newest {
                query.push(("boost".to_string(), order.api_value().to_string()));
            } else {
                query.push(("order".to_string(), order.api_value().to_string()));
            }
        }
        query
    }

    fn fetch_selections(&self, generation: u64) -> Result<Vec<JamendoSelection>, JamendoError> {
        let base = self.options.base_url.trim_end_matches('/');
        let hots_url = format!("{base}/playlists/hots/");
        let hots_query = vec![
            ("client_id".to_string(), self.client_id.clone()),
            ("format".to_string(), "json".to_string()),
            ("limit".to_string(), HOT_SELECTION_IDS.to_string()),
        ];
        let hots_body = self.get(&hots_url, &hots_query, generation)?;
        let hots = parse_hot_playlists(&hots_body).map_err(|error| self.redact(error))?;
        let ids: Vec<String> = hots
            .into_iter()
            .map(|hot| hot.id)
            .filter(|id| !id.trim().is_empty())
            .take(HOT_SELECTION_IDS)
            .collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let tracks_url = format!("{base}/playlists/tracks/");
        let tracks_query = vec![
            ("client_id".to_string(), self.client_id.clone()),
            ("format".to_string(), "json".to_string()),
            ("id".to_string(), ids.join(" ")),
            ("limit".to_string(), ids.len().to_string()),
            ("audioformat".to_string(), "mp32".to_string()),
            ("imagesize".to_string(), "300".to_string()),
        ];
        let tracks_body = self.get(&tracks_url, &tracks_query, generation)?;
        let selections = parse_selections(&tracks_body).map_err(|error| self.redact(error))?;
        let selections: Vec<JamendoSelection> = selections
            .into_iter()
            .filter(|selection| !selection.tracks.is_empty())
            .take(MAX_SELECTIONS)
            .collect();
        if !selections.is_empty() {
            self.store.store_selections(&selections)?;
        }
        Ok(selections)
    }

    /// GET with the 1 request/second limiter, exponential backoff on 429/5xx
    /// and network errors, and cancellation checks around every blocking step.
    fn get(
        &self,
        url: &str,
        query: &[(String, String)],
        generation: u64,
    ) -> Result<String, JamendoError> {
        let mut attempt = 0;
        loop {
            if self.is_cancelled(generation) {
                return Err(JamendoError::Cancelled);
            }
            if !self.limiter.wait(generation, &self.generation) {
                return Err(JamendoError::Cancelled);
            }
            let runtime = runtime()?;
            let client = client()?;
            let response = runtime.block_on(async { client.get(url).query(query).send().await });
            match response {
                Ok(response) if response.status().is_success() => {
                    if self.is_cancelled(generation) {
                        return Err(JamendoError::Cancelled);
                    }
                    let body = runtime.block_on(response.text()).map_err(|_| {
                        JamendoError::Network("cannot read Jamendo response".into())
                    })?;
                    if self.is_cancelled(generation) {
                        return Err(JamendoError::Cancelled);
                    }
                    return Ok(body);
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
                    attempt += 1;
                    if !sleep_cancellable(delay, generation, &self.generation) {
                        return Err(JamendoError::Cancelled);
                    }
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
                    attempt += 1;
                    if !sleep_cancellable(delay, generation, &self.generation) {
                        return Err(JamendoError::Cancelled);
                    }
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

    fn cancel(&self) {
        JamendoSource::cancel(self);
    }

    fn prefetch(&self) {
        JamendoSource::prefetch(self);
    }

    fn selections(&self) -> Result<Vec<SourceItem>, SourceError> {
        JamendoSource::selections(self).map_err(source_error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortOrder {
    PopularWeek,
    PopularMonth,
    PopularTotal,
    Newest,
}

impl SortOrder {
    fn from_filter(value: Option<&str>) -> Option<Self> {
        match value? {
            "popular_week" => Some(Self::PopularWeek),
            "popular_month" => Some(Self::PopularMonth),
            "popular_total" => Some(Self::PopularTotal),
            "newest" => Some(Self::Newest),
            _ => None,
        }
    }

    fn api_value(self) -> &'static str {
        match self {
            Self::PopularWeek => "popularity_week",
            Self::PopularMonth => "popularity_month",
            Self::PopularTotal => "popularity_total",
            Self::Newest => "releasedate_desc",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::PopularWeek => "popular_week",
            Self::PopularMonth => "popular_month",
            Self::PopularTotal => "popular_total",
            Self::Newest => "newest",
        }
    }
}

#[derive(Debug, Clone)]
struct SearchParams {
    text: String,
    artist: String,
    tag: String,
    name: String,
    order: Option<SortOrder>,
    offset: usize,
    limit: usize,
    selection_id: String,
    refresh: bool,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            text: String::new(),
            artist: String::new(),
            tag: String::new(),
            name: String::new(),
            order: None,
            offset: 0,
            limit: DEFAULT_PAGE_SIZE,
            selection_id: String::new(),
            refresh: false,
        }
    }
}

impl SearchParams {
    fn from_query(query: &SourceQuery) -> Self {
        let limit = query
            .filter("limit")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = query
            .filter("offset")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        Self {
            text: query.text.trim().to_string(),
            artist: query
                .filter("artist")
                .unwrap_or_default()
                .trim()
                .to_string(),
            tag: query.filter("tag").unwrap_or_default().trim().to_string(),
            name: query.filter("name").unwrap_or_default().trim().to_string(),
            order: SortOrder::from_filter(query.filter("order")),
            offset,
            limit,
            selection_id: query
                .filter("selection")
                .unwrap_or_default()
                .trim()
                .to_string(),
            refresh: query.filter("refresh").is_some_and(|value| value == "1"),
        }
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.artist.is_empty()
            && self.tag.is_empty()
            && self.name.is_empty()
            && self.selection_id.is_empty()
    }

    fn is_browse(&self) -> bool {
        self.selection_id.is_empty()
            && self.text.is_empty()
            && self.artist.is_empty()
            && self.name.is_empty()
            && !self.tag.is_empty()
    }

    fn usage_tag(&self) -> Option<&str> {
        if self.selection_id.is_empty() && !self.tag.is_empty() {
            Some(self.tag.as_str())
        } else {
            None
        }
    }

    fn cache_key(&self) -> String {
        let raw = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.text,
            self.artist,
            self.tag,
            self.name,
            self.order.map_or("relevance", SortOrder::key),
            self.offset,
            self.limit,
            self.selection_id
        );
        blake3::hash(raw.as_bytes()).to_hex().to_string()
    }
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
struct ApiResponse {
    #[serde(default)]
    headers: ApiHeaders,
    #[serde(default)]
    results: Vec<ApiTrack>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiTrack {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default, deserialize_with = "deserialize_seconds")]
    duration: u64,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    album_name: String,
    #[serde(default)]
    album_image: String,
    #[serde(default)]
    image: String,
    #[serde(default)]
    audio: String,
    #[serde(default)]
    license_ccurl: String,
    #[serde(default)]
    shareurl: String,
}

#[derive(Debug, Deserialize, Default)]
struct ApiHot {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize, Default)]
struct ApiHots {
    #[serde(default)]
    headers: ApiHeaders,
    #[serde(default)]
    results: Vec<ApiHot>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiPlaylist {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    user_name: String,
    #[serde(default)]
    tracks: Vec<ApiTrack>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiPlaylists {
    #[serde(default)]
    headers: ApiHeaders,
    #[serde(default)]
    results: Vec<ApiPlaylist>,
}

fn deserialize_seconds<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumberOrText {
        Number(u64),
        Text(String),
    }

    let value = Option::<NumberOrText>::deserialize(deserializer)?;
    Ok(match value {
        Some(NumberOrText::Number(value)) => value,
        Some(NumberOrText::Text(value)) => value.trim().parse().unwrap_or(0),
        None => 0,
    })
}

fn parse_tracks(body: &str) -> Result<Vec<JamendoTrack>, JamendoError> {
    let response: ApiResponse = serde_json::from_str(body)
        .map_err(|_| JamendoError::Network("invalid Jamendo response".into()))?;
    check_headers(&response.headers)?;
    Ok(response
        .results
        .into_iter()
        .filter_map(track_from_api)
        .collect())
}

fn parse_hot_playlists(body: &str) -> Result<Vec<ApiHot>, JamendoError> {
    let response: ApiHots = serde_json::from_str(body)
        .map_err(|_| JamendoError::Network("invalid Jamendo selections response".into()))?;
    check_headers(&response.headers)?;
    Ok(response.results)
}

fn parse_selections(body: &str) -> Result<Vec<JamendoSelection>, JamendoError> {
    let response: ApiPlaylists = serde_json::from_str(body)
        .map_err(|_| JamendoError::Network("invalid Jamendo selections response".into()))?;
    check_headers(&response.headers)?;
    Ok(response
        .results
        .into_iter()
        .filter_map(|playlist| {
            if playlist.id.trim().is_empty() || playlist.name.trim().is_empty() {
                return None;
            }
            let tracks: Vec<JamendoTrack> = playlist
                .tracks
                .into_iter()
                .filter_map(track_from_api)
                .collect();
            if tracks.is_empty() {
                return None;
            }
            Some(JamendoSelection {
                id: playlist.id.trim().to_string(),
                name: playlist.name.trim().to_string(),
                user_name: playlist.user_name.trim().to_string(),
                tracks,
            })
        })
        .collect())
}

fn track_from_api(item: ApiTrack) -> Option<JamendoTrack> {
    let image = if item.album_image.trim().is_empty() {
        item.image.trim().to_string()
    } else {
        item.album_image.trim().to_string()
    };
    let track = JamendoTrack {
        id: item.id.trim().to_string(),
        name: item.name.trim().to_string(),
        artist_name: item.artist_name.trim().to_string(),
        album_name: item.album_name.trim().to_string(),
        duration_secs: item.duration,
        audio_url: item.audio.trim().to_string(),
        image_url: image,
        license_url: item.license_ccurl.trim().to_string(),
        share_url: item.shareurl.trim().to_string(),
    };
    track.is_playable().then_some(track)
}

fn check_headers(headers: &ApiHeaders) -> Result<(), JamendoError> {
    if headers.status != "success" || headers.code != 0 {
        let message = if headers.error_message.trim().is_empty() {
            "Jamendo request rejected".to_string()
        } else {
            headers.error_message.clone()
        };
        return Err(JamendoError::Api(message));
    }
    Ok(())
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
        JamendoError::Cancelled => SourceError::Network("Jamendo request cancelled".into()),
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

fn sleep_cancellable(duration: Duration, generation: u64, current: &AtomicU64) -> bool {
    let mut remaining = duration;
    while !remaining.is_zero() {
        if current.load(Ordering::Acquire) != generation {
            return false;
        }
        let step = remaining.min(Duration::from_millis(25));
        std::thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
    current.load(Ordering::Acquire) == generation
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

    /// Returns false when the request was cancelled while waiting. The mutex is
    /// held for the wait so concurrent workers queue behind the same interval.
    fn wait(&self, generation: u64, current: &AtomicU64) -> bool {
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(previous) = *last {
            let mut remaining = self.interval.saturating_sub(previous.elapsed());
            while !remaining.is_zero() {
                if current.load(Ordering::Acquire) != generation {
                    return false;
                }
                let step = remaining.min(Duration::from_millis(25));
                std::thread::sleep(step);
                remaining = remaining.saturating_sub(step);
            }
        }
        if current.load(Ordering::Acquire) != generation {
            return false;
        }
        *last = Some(Instant::now());
        true
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
    use std::sync::atomic::Ordering;

    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

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

    fn track_json(index: usize) -> serde_json::Value {
        json!({
            "id": index.to_string(),
            "name": format!("Track {index}"),
            "duration": 180,
            "artist_name": "Artist",
            "album_name": "Album",
            "album_image": format!("https://img.example/{index}.jpg"),
            "audio": format!("https://audio.example/{index}.mp3"),
            "license_ccurl": "http://creativecommons.org/licenses/by-sa/3.0/",
            "shareurl": format!("https://www.jamendo.com/track/{index}"),
            "audiodownload": format!("https://download.example/{index}.mp3"),
            "audiodownload_allowed": true
        })
    }

    fn response() -> serde_json::Value {
        json!({
            "headers": {"status": "success", "code": 0, "error_message": ""},
            "results": [track_json(42)]
        })
    }

    fn response_with(count: usize) -> serde_json::Value {
        json!({
            "headers": {"status": "success", "code": 0, "error_message": ""},
            "results": (0..count).map(track_json).collect::<Vec<_>>()
        })
    }

    fn selected_query(tag: &str, order: Option<&str>, offset: usize, limit: usize) -> SourceQuery {
        let mut query = SourceQuery {
            text: String::new(),
            filters: vec![
                ("tag".into(), tag.into()),
                ("offset".into(), offset.to_string()),
                ("limit".into(), limit.to_string()),
            ],
        };
        if let Some(order) = order {
            query.set_filter("order", order);
        }
        query
    }

    fn query_value(request: &Request, key: &str) -> Option<String> {
        request
            .url
            .query_pairs()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.into_owned())
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
                .expect(1)
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
        assert!(!outcome.cached);
        assert!(!outcome.has_more);
        assert_eq!(outcome.items.len(), 1);
        let item = &outcome.items[0];
        assert_eq!(item.title, "Track 42");
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
    fn search_uses_a_fresh_cache_without_network() {
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
        let source = source(&server, store, client);
        let query = selected_query("jazz", Some("popular_week"), 0, 60);
        let first = source.search(&query).expect("cold search");
        assert!(!first.cached);
        let second = source.search(&query).expect("warm search");
        assert!(second.cached);
        assert!(!second.offline);
        assert_eq!(second.items.len(), 1);
    }

    #[test]
    fn explicit_refresh_bypasses_the_fresh_cache() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .expect(2)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client);
        let query = selected_query("jazz", Some("popular_week"), 0, 60);
        let first = source.search(&query).expect("cold search");
        assert!(!first.cached);
        let mut refresh = query.clone();
        refresh.set_filter("refresh", "1");
        let second = source.search(&refresh).expect("refreshed search");
        assert!(!second.cached);
    }

    #[test]
    fn search_falls_back_to_a_stale_cache_when_the_server_is_down() {
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
        let query = selected_query("jazz", Some("popular_week"), 0, 60);
        online.search(&query).expect("online search");
        store.age_search_cache(7200).expect("age cache");
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
        let outcome = offline.search(&query).expect("stale cache fallback");
        assert!(outcome.offline);
        assert!(outcome.cached);
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
            .search(&selected_query("jazz", Some("popular_week"), 0, 60))
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
            .search(&selected_query("jazz", Some("popular_week"), 0, 60))
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
            .search(&selected_query("jazz", Some("popular_week"), 0, 60))
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
            .search(&selected_query("jazz", Some("popular_week"), 0, 60))
            .expect("search");
        assert!(source.toggle_favorite("42").expect("favorite"));
        assert_eq!(source.favorites().expect("favorites").len(), 1);
        let resolved = source.resolve("42").expect("resolve");
        assert_eq!(resolved.track.title, "Track 42");
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

    #[test]
    fn order_pagination_and_page_size_are_sent() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("tags", "jazz"))
                .and(query_param("order", "popularity_week"))
                .and(query_param("limit", "60"))
                .and(query_param("offset", "60"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response_with(60)))
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client);
        let query = selected_query("jazz", Some("popular_week"), 60, 60);
        let outcome = source.search(&query).expect("page");
        assert_eq!(outcome.items.len(), 60);
        assert!(outcome.has_more);
        assert!(!outcome.cached);
        let again = source.search(&query).expect("cached page");
        assert!(again.cached);
        assert_eq!(again.items.len(), 60);
    }

    #[test]
    fn newest_uses_releasedate_desc() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("tags", "rock"))
                .and(query_param("order", "releasedate_desc"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client);
        let outcome = source
            .search(&selected_query("rock", Some("newest"), 0, 60))
            .expect("newest");
        assert_eq!(outcome.items.len(), 1);
    }

    #[test]
    fn text_search_uses_boost_for_popularity() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client);
        let query = SourceQuery {
            text: "love".into(),
            filters: vec![
                ("tag".into(), "jazz".into()),
                ("order".into(), "popular_month".into()),
            ],
        };
        source.search(&query).expect("search");
        let requests = runtime
            .block_on(server.received_requests())
            .expect("received requests");
        let request = requests.last().expect("request");
        assert_eq!(query_value(request, "search").as_deref(), Some("love"));
        assert_eq!(query_value(request, "tags").as_deref(), Some("jazz"));
        assert_eq!(
            query_value(request, "boost").as_deref(),
            Some("popularity_month")
        );
        assert_eq!(query_value(request, "order"), None);
    }

    #[test]
    fn empty_browse_results_are_retried_once() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("client_id", client.clone()))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "headers": {"status": "success", "code": 0, "error_message": ""},
                    "results": []
                })))
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
            .search(&selected_query("jazz", Some("popular_week"), 0, 60))
            .expect("search after empty retry");
        assert_eq!(outcome.items.len(), 1);
        assert!(!outcome.cached);
    }

    #[test]
    fn cancellation_stops_a_delayed_request() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_delay(Duration::from_millis(400))
                        .set_body_json(response()),
                )
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, store, client);
        let query = selected_query("jazz", Some("popular_week"), 0, 60);
        std::thread::scope(|scope| {
            let handle = scope.spawn(|| source.search(&query));
            std::thread::sleep(Duration::from_millis(80));
            source.cancel();
            let outcome = handle.join().expect("search thread");
            assert!(matches!(outcome, Err(JamendoError::Cancelled)));
        });
    }

    #[test]
    fn prefetch_warms_the_first_page_cache() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .and(query_param("tags", "jazz"))
                .and(query_param("order", "popularity_week"))
                .and(query_param("offset", "0"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response_with(60)))
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        store.record_genre_use("jazz").expect("usage");
        let source = source(&server, store, client);
        source.prefetch();
        source.prefetch();
        let outcome = source
            .search(&selected_query("jazz", Some("popular_week"), 0, 60))
            .expect("warm search");
        assert!(outcome.cached);
        assert_eq!(outcome.items.len(), 60);
    }

    #[test]
    fn selections_are_fetched_parsed_and_cached() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/playlists/hots/"))
                .and(query_param("limit", "8"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "headers": {"status": "success", "code": 0, "error_message": ""},
                    "results": [{"id": "10"}, {"id": "11"}]
                })))
                .expect(1)
                .mount(&server),
        );
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/playlists/tracks/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "headers": {"status": "success", "code": 0, "error_message": ""},
                    "results": [
                        {
                            "id": "10",
                            "name": "Selection Ten",
                            "user_name": "Curator",
                            "tracks": [{
                                "id": "100",
                                "name": "Selection Track",
                                "duration": "216",
                                "artist_name": "Artist",
                                "album_name": "Album",
                                "album_image": "https://img.example/100.jpg",
                                "image": "https://img.example/100.jpg",
                                "audio": "https://audio.example/100.mp3",
                                "license_ccurl": "http://creativecommons.org/licenses/by/3.0/",
                                "shareurl": "https://www.jamendo.com/track/100"
                            }]
                        },
                        {
                            "id": "11",
                            "name": "Selection Eleven",
                            "user_name": "Curator",
                            "tracks": [{
                                "id": "101",
                                "name": "Second Track",
                                "duration": 120,
                                "artist_name": "Artist",
                                "album_name": "Album",
                                "album_image": "https://img.example/101.jpg",
                                "image": "https://img.example/101.jpg",
                                "audio": "https://audio.example/101.mp3",
                                "license_ccurl": "http://creativecommons.org/licenses/by-sa/3.0/",
                                "shareurl": "https://www.jamendo.com/track/101"
                            }]
                        }
                    ]
                })))
                .expect(1)
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, Arc::clone(&store), client);
        let selections = source.selections().expect("selections");
        assert_eq!(selections.len(), 2);
        assert_eq!(selections[0].title, "Selection Ten");
        assert_eq!(selections[0].subtitle, "Curator");
        let cached = source.selections().expect("cached selections");
        assert_eq!(cached.len(), 2);
        let query = SourceQuery {
            text: String::new(),
            filters: vec![("selection".into(), "10".into())],
        };
        let tracks = source.search(&query).expect("selection tracks");
        assert_eq!(tracks.items.len(), 1);
        assert!(tracks.cached);
        assert_eq!(tracks.items[0].duration, Some(Duration::from_secs(216)));
        assert!(store.selection("10").expect("db").is_some());
    }

    #[test]
    fn genre_usage_is_recorded_for_tag_searches() {
        let runtime = runtime();
        let server = runtime.block_on(MockServer::start());
        let client = client_id();
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/tracks/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response()))
                .mount(&server),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            Arc::new(JamendoStore::open(&dir.path().join("jamendo.sqlite3")).expect("store"));
        let source = source(&server, Arc::clone(&store), client);
        source
            .search(&selected_query("jazz", Some("popular_week"), 0, 60))
            .expect("search");
        assert_eq!(
            store.popular_genres(4).expect("popular"),
            vec!["jazz".to_string()]
        );
    }

    #[test]
    fn page_limits_are_clamped_and_cache_keys_include_paging() {
        let mut query = selected_query("jazz", Some("popular_week"), 0, 999);
        let params = SearchParams::from_query(&query);
        assert_eq!(params.limit, MAX_PAGE_SIZE);
        query.set_filter("limit", "0");
        assert_eq!(SearchParams::from_query(&query).limit, 1);
        let first = SearchParams::from_query(&selected_query("jazz", Some("popular_week"), 0, 60));
        let second =
            SearchParams::from_query(&selected_query("jazz", Some("popular_week"), 60, 60));
        let third = SearchParams::from_query(&selected_query("jazz", Some("popular_month"), 0, 60));
        assert_ne!(first.cache_key(), second.cache_key());
        assert_ne!(first.cache_key(), third.cache_key());
    }

    #[test]
    fn rate_limiter_waits_and_can_be_cancelled() {
        let limiter = RateLimiter::new(Duration::from_millis(120));
        let current = AtomicU64::new(0);
        assert!(limiter.wait(0, &current));
        let start = Instant::now();
        assert!(limiter.wait(0, &current));
        assert!(start.elapsed() >= Duration::from_millis(80));

        let limiter = RateLimiter::new(Duration::from_millis(200));
        assert!(limiter.wait(0, &current));
        current.store(1, Ordering::Release);
        assert!(!limiter.wait(0, &current));
    }
}
