use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, PlaybackState, Playlist,
    PlaylistId, PlaylistItem, PlaylistItemId, QueueItem, QueueItemId, RepeatMode, SearchResults,
    SourceEntry, SourceItem, SourceQuery, StreamStatus, Track, TrackId, TrackSort,
};
use rune::{Image, ImageCache, Key, KeyEvent, TextEdit, Tx, Waker};

use crate::backend::Backend;
use crate::i18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tracks,
    Albums,
    Artists,
    Playlists,
    Queue,
    Sources,
    Radio,
    Podcast,
    Jamendo,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JamendoSort {
    PopularWeek,
    PopularMonth,
    PopularTotal,
    Newest,
}

impl JamendoSort {
    #[must_use]
    pub fn filter_value(self) -> &'static str {
        match self {
            Self::PopularWeek => "popular_week",
            Self::PopularMonth => "popular_month",
            Self::PopularTotal => "popular_total",
            Self::Newest => "newest",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::PopularWeek => i18n::JAMENDO_SORT_WEEK,
            Self::PopularMonth => i18n::JAMENDO_SORT_MONTH,
            Self::PopularTotal => i18n::JAMENDO_SORT_ALL,
            Self::Newest => i18n::JAMENDO_SORT_NEWEST,
        }
    }

    pub const ALL: [Self; 4] = [
        Self::PopularWeek,
        Self::PopularMonth,
        Self::PopularTotal,
        Self::Newest,
    ];
}

pub const JAMENDO_PAGE_SIZE: usize = 60;

#[derive(Debug, Default)]
pub struct RadioSearchState {
    pub name: TextEdit,
    pub country: TextEdit,
    pub tag: TextEdit,
}

#[derive(Debug, Clone)]
pub enum Filter {
    Album(Album),
    Artist(Artist),
}

impl Filter {
    pub fn title(&self) -> String {
        match self {
            Self::Album(album) => album.title.clone(),
            Self::Artist(artist) => artist.name.clone(),
        }
    }

    pub fn subtitle(&self) -> String {
        match self {
            Self::Album(album) => i18n::album_meta(&album.artist, album.year, album.track_count),
            Self::Artist(artist) => i18n::artist_meta(artist.album_count, artist.track_count),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanState {
    pub scanned: u64,
    pub total: u64,
}

pub struct CoverCache {
    cache: Option<ImageCache>,
    images: HashMap<PathBuf, Image>,
    order: VecDeque<PathBuf>,
    scale: f32,
}

const MAX_COVERS: usize = 128;
const COVER_SIZE_PT: f32 = 192.0;

impl CoverCache {
    fn new() -> Self {
        Self {
            cache: None,
            images: HashMap::new(),
            order: VecDeque::new(),
            scale: 1.0,
        }
    }

    fn install(&mut self, waker: Waker) {
        self.cache = Some(ImageCache::new(waker));
    }

    fn set_scale(&mut self, scale: f32) {
        if (self.scale - scale).abs() > 0.01 {
            self.scale = scale.max(1.0);
            self.images.clear();
            self.order.clear();
        }
    }

    fn get(&mut self, path: &Path) -> Option<Image> {
        if let Some(image) = self.images.get(path) {
            if let Some(position) = self.order.iter().position(|cached| cached == path)
                && let Some(cached) = self.order.remove(position)
            {
                self.order.push_back(cached);
            }
            return Some(image.clone());
        }
        let cache = self.cache.as_ref()?;
        let image = cache.file(path, COVER_SIZE_PT, self.scale);
        self.images.insert(path.to_path_buf(), image.clone());
        self.order.push_back(path.to_path_buf());
        while self.images.len() > MAX_COVERS {
            if let Some(oldest) = self.order.pop_front() {
                self.images.remove(&oldest);
            } else {
                break;
            }
        }
        Some(image)
    }
}

pub struct UiState {
    pub backend: Arc<dyn Backend>,
    pub view: View,
    pub query: TextEdit,
    pub tracks: Vec<Track>,
    pub track_keys: Vec<String>,
    pub albums: Vec<Album>,
    pub album_keys: Vec<String>,
    pub artists: Vec<Artist>,
    pub artist_keys: Vec<String>,
    pub queue: Vec<QueueItem>,
    pub queue_keys: Vec<String>,
    pub filter: Option<Filter>,
    pub filtered: Vec<Track>,
    pub filtered_keys: Vec<String>,
    pub search: Option<SearchResults>,
    pub current: Option<QueueItem>,
    pub playback: PlaybackState,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: u16,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub folders: Vec<PathBuf>,
    pub stats: LibraryStats,
    pub scan: Option<ScanState>,
    pub scan_last: Option<String>,
    pub error: Option<String>,
    pub output: (u32, u16),
    pub images: RefCell<CoverCache>,
    pub list_version: u64,
    pub radio_query: RadioSearchState,
    pub source_items: Vec<SourceItem>,
    pub source_keys: Vec<String>,
    pub source_loading: bool,
    pub source_offline: bool,
    pub podcast_query: TextEdit,
    pub podcast_items: Vec<SourceItem>,
    pub podcast_keys: Vec<String>,
    pub podcast_loading: bool,
    pub podcast_selected: Option<String>,
    pub podcast_selected_title: String,
    pub podcast_entries: Vec<SourceEntry>,
    pub podcast_entries_loading: bool,
    pub jamendo_available: bool,
    pub jamendo_query: TextEdit,
    pub jamendo_items: Vec<SourceItem>,
    pub jamendo_keys: Vec<String>,
    pub jamendo_loading: bool,
    pub jamendo_offline: bool,
    pub jamendo_genre: String,
    pub jamendo_sort: JamendoSort,
    pub jamendo_offset: usize,
    pub jamendo_has_more: bool,
    pub jamendo_loading_more: bool,
    pub jamendo_cached: bool,
    pub jamendo_selected: Option<usize>,
    pub jamendo_started: bool,
    pub jamendo_request_at: Option<Instant>,
    pub jamendo_latency_ms: Option<u128>,
    pub jamendo_latency_cached: bool,
    pub jamendo_selections: Vec<SourceItem>,
    pub jamendo_selection_keys: Vec<String>,
    pub jamendo_selections_loading: bool,
    pub jamendo_selections_requested: bool,
    pub jamendo_selections_attempts: u8,
    pub jamendo_selection: Option<String>,
    pub jamendo_selection_name: String,
    pub jamendo_action_status: Option<String>,
    pub stream_title: Option<String>,
    pub stream_status: StreamStatus,
    pub seekable: bool,
    pub playlists: Vec<Playlist>,
    pub playlist_keys: Vec<String>,
    pub selected_playlist: Option<PlaylistId>,
    pub selected_playlist_name: String,
    pub playlist_items: Vec<PlaylistItem>,
    pub playlist_item_keys: Vec<String>,
    pub playlist_selected_item: Option<PlaylistItemId>,
    pub playlist_name: TextEdit,
    pub playlist_status: Option<String>,
}

impl UiState {
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        let (volume, shuffle, repeat) = backend.playback_settings();
        let output = backend.output_info();
        let jamendo_available = backend.source_available("jamendo");
        let mut state = Self {
            backend,
            view: View::Tracks,
            query: TextEdit::default(),
            tracks: Vec::new(),
            track_keys: Vec::new(),
            albums: Vec::new(),
            album_keys: Vec::new(),
            artists: Vec::new(),
            artist_keys: Vec::new(),
            queue: Vec::new(),
            queue_keys: Vec::new(),
            filter: None,
            filtered: Vec::new(),
            filtered_keys: Vec::new(),
            search: None,
            current: None,
            playback: PlaybackState::Stopped,
            position: Duration::ZERO,
            duration: None,
            volume,
            shuffle,
            repeat,
            folders: Vec::new(),
            stats: LibraryStats::default(),
            scan: None,
            scan_last: None,
            error: None,
            output,
            images: RefCell::new(CoverCache::new()),
            list_version: 0,
            radio_query: RadioSearchState::default(),
            source_items: Vec::new(),
            source_keys: Vec::new(),
            source_loading: false,
            source_offline: false,
            podcast_query: TextEdit::default(),
            podcast_items: Vec::new(),
            podcast_keys: Vec::new(),
            podcast_loading: false,
            podcast_selected: None,
            podcast_selected_title: String::new(),
            podcast_entries: Vec::new(),
            podcast_entries_loading: false,
            jamendo_available,
            jamendo_query: TextEdit::default(),
            jamendo_items: Vec::new(),
            jamendo_keys: Vec::new(),
            jamendo_loading: false,
            jamendo_offline: false,
            jamendo_genre: i18n::JAMENDO_GENRES
                .first()
                .map_or("jazz", |(tag, _)| *tag)
                .to_string(),
            jamendo_sort: JamendoSort::PopularWeek,
            jamendo_offset: 0,
            jamendo_has_more: false,
            jamendo_loading_more: false,
            jamendo_cached: false,
            jamendo_selected: None,
            jamendo_started: false,
            jamendo_request_at: None,
            jamendo_latency_ms: None,
            jamendo_latency_cached: false,
            jamendo_selections: Vec::new(),
            jamendo_selection_keys: Vec::new(),
            jamendo_selections_loading: false,
            jamendo_selections_requested: false,
            jamendo_selections_attempts: 0,
            jamendo_selection: None,
            jamendo_selection_name: String::new(),
            jamendo_action_status: None,
            stream_title: None,
            stream_status: StreamStatus::Idle,
            seekable: true,
            playlists: Vec::new(),
            playlist_keys: Vec::new(),
            selected_playlist: None,
            selected_playlist_name: String::new(),
            playlist_items: Vec::new(),
            playlist_item_keys: Vec::new(),
            playlist_selected_item: None,
            playlist_name: TextEdit::default(),
            playlist_status: None,
        };
        state.reload();
        state
    }

    pub fn install_images(&self, waker: Waker) {
        self.images.borrow_mut().install(waker);
    }

    pub fn set_cover_scale(&self, scale: f32) {
        self.images.borrow_mut().set_scale(scale);
    }

    pub fn cover(&self, path: Option<&Path>) -> Option<Image> {
        let path = path?;
        self.images.borrow_mut().get(path)
    }

    pub fn reload(&mut self) {
        match self.backend.tracks(TrackSort::Title, 100_000, 0) {
            Ok(tracks) => {
                self.track_keys = tracks
                    .iter()
                    .map(|track| format!("track-{}", track.id.get()))
                    .collect();
                self.tracks = tracks;
            }
            Err(error) => self.error = Some(error),
        }
        match self.backend.albums() {
            Ok(albums) => {
                self.album_keys = albums
                    .iter()
                    .map(|album| format!("album-{}", album.id.get()))
                    .collect();
                self.albums = albums;
            }
            Err(error) => self.error = Some(error),
        }
        match self.backend.artists() {
            Ok(artists) => {
                self.artist_keys = artists
                    .iter()
                    .map(|artist| format!("artist-{}", artist.id.get()))
                    .collect();
                self.artists = artists;
            }
            Err(error) => self.error = Some(error),
        }
        if let Ok(stats) = self.backend.stats() {
            self.stats = stats;
        }
        self.folders = self.backend.folders();
        self.refresh_playlists();
        if self.selected_playlist.is_some() {
            self.reload_playlist_items();
        }
        self.reload_filter();
        self.list_version = self.list_version.wrapping_add(1);
        if !self.query.text.trim().is_empty() {
            self.refresh_search();
        }
    }

    fn reload_filter(&mut self) {
        self.list_version = self.list_version.wrapping_add(1);
        let result = match &self.filter {
            Some(Filter::Album(album)) => self.backend.tracks_for_album(album.id),
            Some(Filter::Artist(artist)) => self.backend.tracks_for_artist(artist.id),
            None => {
                self.filtered.clear();
                self.filtered_keys.clear();
                return;
            }
        };
        match result {
            Ok(tracks) => {
                self.filtered_keys = tracks
                    .iter()
                    .map(|track| format!("filtered-{}", track.id.get()))
                    .collect();
                self.filtered = tracks;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn current_list(&self) -> &[Track] {
        if self.filter.is_some() {
            &self.filtered
        } else {
            &self.tracks
        }
    }

    pub fn current_keys(&self) -> &[String] {
        if self.filter.is_some() {
            &self.filtered_keys
        } else {
            &self.track_keys
        }
    }

    pub fn set_view(&mut self, view: View) {
        let previous = self.view;
        self.view = view;
        if previous == View::Jamendo && view != View::Jamendo {
            self.backend.source_cancel("jamendo");
            self.jamendo_loading = false;
            self.jamendo_loading_more = false;
            self.jamendo_request_at = None;
        }
        if matches!(view, View::Tracks | View::Albums | View::Artists) {
            self.filter = None;
            self.filtered.clear();
            self.filtered_keys.clear();
        }
        if view == View::Radio {
            self.refresh_source_favorites("radio");
        }
        if view == View::Podcast {
            self.refresh_podcast_subscriptions();
        }
        if view == View::Jamendo && !self.jamendo_started {
            self.jamendo_started = true;
            self.load_jamendo_page(true, false);
        }
        if view == View::Playlists {
            self.refresh_playlists();
            self.reload_playlist_items();
        }
    }

    pub fn refresh_playlists(&mut self) {
        match self.backend.playlists() {
            Ok(playlists) => {
                self.playlist_keys = playlists
                    .iter()
                    .map(|playlist| format!("playlist-{}", playlist.id.get()))
                    .collect();
                self.playlists = playlists;
                if let Some(id) = self.selected_playlist {
                    match self.playlists.iter().find(|playlist| playlist.id == id) {
                        Some(playlist) => self.selected_playlist_name = playlist.name.clone(),
                        None => {
                            self.selected_playlist = None;
                            self.selected_playlist_name.clear();
                            self.playlist_items.clear();
                            self.playlist_item_keys.clear();
                            self.playlist_selected_item = None;
                        }
                    }
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn reload_playlist_items(&mut self) {
        let Some(id) = self.selected_playlist else {
            self.playlist_items.clear();
            self.playlist_item_keys.clear();
            self.playlist_selected_item = None;
            return;
        };
        match self.backend.playlist_items(id) {
            Ok(items) => {
                self.playlist_item_keys = items
                    .iter()
                    .map(|item| format!("playlist-item-{}", item.id.get()))
                    .collect();
                self.playlist_items = items;
                if let Some(selected) = self.playlist_selected_item
                    && !self.playlist_items.iter().any(|item| item.id == selected)
                {
                    self.playlist_selected_item = None;
                }
            }
            Err(error) => self.error = Some(error),
        }
        self.list_version = self.list_version.wrapping_add(1);
    }

    pub fn open_playlist(&mut self, id: PlaylistId) {
        let Some(playlist) = self.playlists.iter().find(|playlist| playlist.id == id) else {
            return;
        };
        self.selected_playlist = Some(id);
        self.selected_playlist_name = playlist.name.clone();
        self.playlist_name = TextEdit::new(&playlist.name);
        self.playlist_selected_item = None;
        self.playlist_status = None;
        self.reload_playlist_items();
    }

    pub fn create_playlist(&mut self) {
        let name = self.playlist_name.text.trim().to_string();
        if name.is_empty() {
            self.error = Some(i18n::PLAYLIST_NAME_REQUIRED.into());
            return;
        }
        match self.backend.create_playlist(&name) {
            Ok(id) => {
                self.playlist_name.clear();
                self.refresh_playlists();
                self.open_playlist(id);
                self.playlist_status = Some(i18n::PLAYLIST_CREATED.into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn rename_selected_playlist(&mut self) {
        let Some(id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        let name = self.playlist_name.text.trim().to_string();
        if name.is_empty() {
            self.error = Some(i18n::PLAYLIST_NAME_REQUIRED.into());
            return;
        }
        match self.backend.rename_playlist(id, &name) {
            Ok(()) => {
                self.selected_playlist_name = name;
                self.refresh_playlists();
                self.playlist_status = Some(i18n::PLAYLIST_RENAMED.into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn delete_selected_playlist(&mut self) {
        let Some(id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        match self.backend.delete_playlist(id) {
            Ok(()) => {
                self.selected_playlist = None;
                self.selected_playlist_name.clear();
                self.playlist_items.clear();
                self.playlist_item_keys.clear();
                self.playlist_selected_item = None;
                self.refresh_playlists();
                self.playlist_status = Some(i18n::PLAYLIST_DELETED.into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn add_tracks_to_selected_playlist(&mut self, tracks: Vec<Track>) {
        let Some(id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        let tracks: Vec<Track> = tracks
            .into_iter()
            .filter(|track| !matches!(track.source.scheme(), "radio" | "podcast"))
            .collect();
        if tracks.is_empty() {
            self.playlist_status = Some(i18n::PLAYLIST_NO_TRACKS.into());
            return;
        }
        match self.backend.add_tracks_to_playlist(id, &tracks) {
            Ok(count) => {
                self.refresh_playlists();
                self.reload_playlist_items();
                self.playlist_status = Some(i18n::playlist_added(count));
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn add_track_to_selected_playlist(&mut self, track: Track) {
        self.add_tracks_to_selected_playlist(vec![track]);
    }

    pub fn add_current_to_selected_playlist(&mut self) {
        let Some(item) = self.current.clone() else {
            self.playlist_status = Some(i18n::PLAYLIST_NO_CURRENT.into());
            return;
        };
        self.add_track_to_selected_playlist(item.track);
    }

    pub fn add_queue_to_selected_playlist(&mut self) {
        let tracks: Vec<Track> = self.queue.iter().map(|item| item.track.clone()).collect();
        if tracks.is_empty() {
            self.playlist_status = Some(i18n::EMPTY_QUEUE.into());
            return;
        }
        self.add_tracks_to_selected_playlist(tracks);
    }

    pub fn save_queue_as_playlist(&mut self) {
        let name = self.playlist_name.text.trim().to_string();
        if name.is_empty() {
            self.error = Some(i18n::PLAYLIST_NAME_REQUIRED.into());
            return;
        }
        let tracks: Vec<Track> = self
            .queue
            .iter()
            .map(|item| item.track.clone())
            .filter(|track| !matches!(track.source.scheme(), "radio" | "podcast"))
            .collect();
        if tracks.is_empty() {
            self.playlist_status = Some(i18n::PLAYLIST_NO_TRACKS.into());
            return;
        }
        match self.backend.save_queue_as_playlist(&name, &tracks) {
            Ok(id) => {
                self.playlist_name.clear();
                self.refresh_playlists();
                self.open_playlist(id);
                self.playlist_status = Some(i18n::PLAYLIST_SAVED_QUEUE.into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn select_playlist_item(&mut self, id: PlaylistItemId) {
        self.playlist_selected_item = Some(id);
        self.list_version = self.list_version.wrapping_add(1);
    }

    pub fn play_selected_playlist(&mut self) {
        let Some(id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        self.backend.play_playlist(id);
    }

    pub fn enqueue_selected_playlist(&mut self) {
        let Some(id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        self.backend.enqueue_playlist(id);
    }

    pub fn remove_playlist_item(&mut self, item_id: PlaylistItemId) {
        match self.backend.remove_playlist_item(item_id) {
            Ok(removed) => {
                if removed {
                    self.playlist_selected_item = None;
                    self.refresh_playlists();
                    self.reload_playlist_items();
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn move_playlist_item(&mut self, item_id: PlaylistItemId, delta: i32) {
        let Some(id) = self.selected_playlist else {
            return;
        };
        match self.backend.move_playlist_item(id, item_id, delta) {
            Ok(moved) => {
                if moved {
                    self.playlist_selected_item = Some(item_id);
                    self.reload_playlist_items();
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn move_selected_playlist_item(&mut self, delta: i32) {
        if let Some(item_id) = self.playlist_selected_item {
            self.move_playlist_item(item_id, delta);
        }
    }

    pub fn remove_missing_playlist_items(&mut self) {
        let Some(id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        match self.backend.remove_missing_playlist_items(id) {
            Ok(removed) => {
                self.refresh_playlists();
                self.reload_playlist_items();
                self.playlist_status = Some(i18n::playlist_missing_removed(removed));
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn import_playlist(&mut self, path: &Path) {
        match self.backend.import_playlist(path) {
            Ok(id) => {
                self.refresh_playlists();
                self.open_playlist(id);
                self.playlist_status = Some(i18n::PLAYLIST_IMPORTED.into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn export_playlist(&mut self, path: &Path) {
        let Some(id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        match self.backend.export_playlist(id, path) {
            Ok(()) => self.playlist_status = Some(i18n::PLAYLIST_EXPORTED.into()),
            Err(error) => self.error = Some(error),
        }
    }

    pub fn refresh_source_favorites(&mut self, source: &str) {
        match self.backend.source_favorites(source) {
            Ok(items) => self.set_source_items(items),
            Err(error) => self.error = Some(error),
        }
    }

    pub fn search_source(&mut self, source: &str, query: SourceQuery) {
        self.source_loading = true;
        self.source_offline = false;
        self.backend.source_search(source, query);
    }

    pub fn search_radio(&mut self) {
        let name = self.radio_query.name.text.trim().to_string();
        let country = self.radio_query.country.text.trim().to_string();
        let tag = self.radio_query.tag.text.trim().to_string();
        if name.is_empty() && country.is_empty() && tag.is_empty() {
            self.refresh_source_favorites("radio");
            return;
        }
        let mut query = SourceQuery {
            text: name,
            ..SourceQuery::default()
        };
        if !country.is_empty() {
            query.set_filter("country", country);
        }
        if !tag.is_empty() {
            query.set_filter("tag", tag);
        }
        self.search_source("radio", query);
    }

    fn set_source_items(&mut self, items: Vec<SourceItem>) {
        self.source_keys = items
            .iter()
            .map(|item| format!("{}-{}", item.source.scheme(), item.id))
            .collect();
        self.source_items = items;
    }

    pub fn toggle_source_favorite(&mut self, source: &str, id: &str) {
        match self.backend.source_toggle_favorite(source, id) {
            Ok(favorite) => {
                if let Some(item) = self.source_items.iter_mut().find(|item| item.id == id) {
                    item.favorite = favorite;
                }
                self.list_version = self.list_version.wrapping_add(1);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn play_source(&mut self, source: &str, id: &str) {
        self.backend.source_play(source, id);
    }

    pub fn refresh_podcast_subscriptions(&mut self) {
        match self.backend.source_favorites("podcast") {
            Ok(items) => self.set_podcast_items(items),
            Err(error) => self.error = Some(error),
        }
    }

    pub fn search_podcasts(&mut self) {
        let text = self.podcast_query.text.trim().to_string();
        if text.is_empty() {
            self.refresh_podcast_subscriptions();
            return;
        }
        self.podcast_loading = true;
        self.backend.source_search(
            "podcast",
            SourceQuery {
                text,
                ..SourceQuery::default()
            },
        );
    }

    fn set_podcast_items(&mut self, items: Vec<SourceItem>) {
        self.podcast_keys = items
            .iter()
            .map(|item| format!("{}-{}", item.source.scheme(), item.id))
            .collect();
        self.podcast_items = items;
    }

    pub fn toggle_podcast_subscription(&mut self, id: &str) {
        match self.backend.source_toggle_favorite("podcast", id) {
            Ok(subscribed) => {
                if let Some(item) = self.podcast_items.iter_mut().find(|item| item.id == id) {
                    item.favorite = subscribed;
                }
                self.list_version = self.list_version.wrapping_add(1);
                if subscribed {
                    self.refresh_podcast(id);
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn open_podcast(&mut self, id: &str) {
        self.podcast_selected = Some(id.to_string());
        self.podcast_selected_title = self
            .podcast_items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.title.clone())
            .unwrap_or_else(|| "Podcast".to_string());
        self.podcast_entries.clear();
        self.podcast_entries_loading = true;
        match self.backend.source_entries("podcast", id) {
            Ok(entries) => self.podcast_entries = entries,
            Err(error) => self.error = Some(error),
        }
        self.backend.source_refresh("podcast", id);
    }

    pub fn refresh_podcast(&mut self, id: &str) {
        self.podcast_entries_loading = true;
        self.backend.source_refresh("podcast", id);
    }

    pub fn play_podcast_entry(&mut self, id: &str) {
        self.backend.source_play("podcast", id);
    }

    pub fn toggle_podcast_watched(&mut self, id: &str, watched: bool) {
        match self.backend.source_set_watched("podcast", id, watched) {
            Ok(()) => {
                if let Some(entry) = self.podcast_entries.iter_mut().find(|entry| entry.id == id) {
                    entry.watched = watched;
                }
                self.list_version = self.list_version.wrapping_add(1);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn refresh_jamendo_favorites(&mut self) {
        self.backend.source_cancel("jamendo");
        match self.backend.source_favorites("jamendo") {
            Ok(items) => {
                self.jamendo_loading = false;
                self.jamendo_loading_more = false;
                self.jamendo_offline = false;
                self.jamendo_has_more = false;
                self.jamendo_selected = None;
                self.jamendo_selection = None;
                self.jamendo_selection_name.clear();
                self.set_jamendo_items(items);
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn load_jamendo_page(&mut self, reset: bool, refresh: bool) {
        if !self.jamendo_available {
            return;
        }
        if reset {
            self.jamendo_offset = 0;
            self.jamendo_has_more = false;
            self.jamendo_selected = None;
            self.jamendo_items.clear();
            self.jamendo_keys.clear();
            self.jamendo_loading = true;
            self.jamendo_loading_more = false;
            self.list_version = self.list_version.wrapping_add(1);
        } else {
            self.jamendo_offset = self.jamendo_offset.saturating_add(JAMENDO_PAGE_SIZE);
            self.jamendo_loading_more = true;
            self.jamendo_loading = false;
        }
        let mut query = SourceQuery {
            text: self.jamendo_query.text.trim().to_string(),
            ..SourceQuery::default()
        };
        if let Some(selection) = &self.jamendo_selection {
            query.set_filter("selection", selection.clone());
        } else {
            if !self.jamendo_genre.is_empty() {
                query.set_filter("tag", self.jamendo_genre.clone());
                query.set_filter("order", self.jamendo_sort.filter_value());
            }
        }
        query.set_filter("offset", self.jamendo_offset.to_string());
        query.set_filter("limit", JAMENDO_PAGE_SIZE.to_string());
        if refresh {
            query.set_filter("refresh", "1");
        }
        self.jamendo_offline = false;
        self.jamendo_cached = false;
        self.jamendo_request_at = Some(Instant::now());
        self.backend.source_search("jamendo", query);
    }

    pub fn search_jamendo(&mut self) {
        if self.jamendo_selection.is_some() && !self.jamendo_query.text.trim().is_empty() {
            self.jamendo_selection = None;
            self.jamendo_selection_name.clear();
        }
        self.load_jamendo_page(true, true);
    }

    pub fn select_jamendo_genre(&mut self, tag: &str) {
        self.jamendo_genre = tag.to_string();
        self.jamendo_selection = None;
        self.jamendo_selection_name.clear();
        self.load_jamendo_page(true, false);
    }

    pub fn set_jamendo_sort(&mut self, sort: JamendoSort) {
        self.jamendo_sort = sort;
        self.jamendo_selection = None;
        self.jamendo_selection_name.clear();
        self.load_jamendo_page(true, false);
    }

    pub fn load_more_jamendo(&mut self) {
        if self.jamendo_loading || self.jamendo_loading_more || !self.jamendo_has_more {
            return;
        }
        self.load_jamendo_page(false, false);
    }

    pub fn move_jamendo_genre(&mut self, delta: i32) {
        let genres = i18n::JAMENDO_GENRES;
        if genres.is_empty() {
            return;
        }
        let current = genres
            .iter()
            .position(|(tag, _)| *tag == self.jamendo_genre)
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(genres.len() as i32) as usize;
        let tag = genres[next].0;
        self.select_jamendo_genre(tag);
    }

    pub fn request_jamendo_selections(&mut self) {
        if !self.jamendo_available
            || self.jamendo_selections_requested
            || self.jamendo_selections_attempts >= 3
        {
            return;
        }
        self.jamendo_selections_requested = true;
        self.jamendo_selections_attempts = self.jamendo_selections_attempts.saturating_add(1);
        self.jamendo_selections_loading = true;
        self.backend.source_selections("jamendo");
    }

    pub fn refresh_jamendo_selections(&mut self) {
        self.jamendo_selections_requested = false;
        self.jamendo_selections_attempts = 0;
        self.request_jamendo_selections();
    }

    pub fn open_jamendo_selection(&mut self, id: &str) {
        let name = self
            .jamendo_selections
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.title.clone())
            .unwrap_or_default();
        self.jamendo_selection = Some(id.to_string());
        self.jamendo_selection_name = name;
        self.jamendo_query.clear();
        self.load_jamendo_page(true, false);
    }

    pub fn back_to_jamendo_genres(&mut self) {
        if self.jamendo_selection.is_none() {
            return;
        }
        self.jamendo_selection = None;
        self.jamendo_selection_name.clear();
        self.load_jamendo_page(true, false);
    }

    fn set_jamendo_items(&mut self, items: Vec<SourceItem>) {
        self.jamendo_keys = items
            .iter()
            .map(|item| format!("jamendo-{}", item.id))
            .collect();
        self.jamendo_items = items;
        self.list_version = self.list_version.wrapping_add(1);
    }

    fn append_jamendo_items(&mut self, items: Vec<SourceItem>) {
        for item in items {
            if self
                .jamendo_items
                .iter()
                .any(|existing| existing.id == item.id)
            {
                continue;
            }
            self.jamendo_keys.push(format!("jamendo-{}", item.id));
            self.jamendo_items.push(item);
        }
        self.list_version = self.list_version.wrapping_add(1);
    }

    pub fn toggle_jamendo_favorite(&mut self, id: &str) {
        match self.backend.source_toggle_favorite("jamendo", id) {
            Ok(favorite) => {
                if let Some(item) = self.jamendo_items.iter_mut().find(|item| item.id == id) {
                    item.favorite = favorite;
                }
                self.list_version = self.list_version.wrapping_add(1);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn select_jamendo_row(&mut self, index: usize) {
        if self.jamendo_selected == Some(index) {
            return;
        }
        self.jamendo_selected = Some(index);
        self.list_version = self.list_version.wrapping_add(1);
    }

    pub fn play_jamendo(&mut self, id: &str) {
        self.backend.source_play("jamendo", id);
    }

    pub fn play_jamendo_selected(&mut self) {
        let id = self
            .jamendo_selected
            .and_then(|index| self.jamendo_items.get(index))
            .or_else(|| self.jamendo_items.first())
            .map(|item| item.id.clone());
        if let Some(id) = id {
            self.play_jamendo(&id);
        }
    }

    fn jamendo_ids(&self) -> Vec<String> {
        self.jamendo_items
            .iter()
            .map(|item| item.id.clone())
            .collect()
    }

    pub fn play_jamendo_list(&mut self, shuffle: bool) {
        let ids = self.jamendo_ids();
        if ids.is_empty() {
            self.jamendo_action_status = Some(i18n::JAMENDO_EMPTY.into());
            return;
        }
        match self.backend.source_play_list("jamendo", &ids, shuffle) {
            Ok(count) => {
                self.jamendo_action_status = Some(i18n::jamendo_playing(count));
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn enqueue_jamendo_list(&mut self) {
        let ids = self.jamendo_ids();
        if ids.is_empty() {
            self.jamendo_action_status = Some(i18n::JAMENDO_EMPTY.into());
            return;
        }
        match self.backend.source_enqueue_list("jamendo", &ids) {
            Ok(count) => {
                self.jamendo_action_status = Some(i18n::jamendo_enqueued(count));
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn save_jamendo_playlist(&mut self) {
        let ids = self.jamendo_ids();
        if ids.is_empty() {
            self.jamendo_action_status = Some(i18n::JAMENDO_EMPTY.into());
            return;
        }
        let name = if let Some(selection) = &self.jamendo_selection {
            if self.jamendo_selection_name.trim().is_empty() {
                i18n::jamendo_playlist_name_selection(selection)
            } else {
                i18n::jamendo_playlist_name_selection(&self.jamendo_selection_name)
            }
        } else {
            i18n::jamendo_playlist_name(&self.jamendo_genre, self.jamendo_sort.label())
        };
        match self
            .backend
            .source_save_list_as_playlist("jamendo", &ids, &name)
        {
            Ok(_) => {
                self.refresh_playlists();
                self.jamendo_action_status = Some(i18n::jamendo_playlist_saved(&name));
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn add_jamendo_to_selected_playlist(&mut self, id: &str) {
        let Some(playlist_id) = self.selected_playlist else {
            self.error = Some(i18n::PLAYLIST_SELECT.into());
            return;
        };
        match self
            .backend
            .add_source_to_playlist(playlist_id, "jamendo", id)
        {
            Ok(0) => self.playlist_status = Some(i18n::PLAYLIST_NO_TRACKS.into()),
            Ok(count) => {
                self.refresh_playlists();
                self.reload_playlist_items();
                self.playlist_status = Some(i18n::playlist_added(count));
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn open_album(&mut self, album_id: AlbumId) {
        let Some(album) = self
            .albums
            .iter()
            .find(|album| album.id == album_id)
            .cloned()
        else {
            return;
        };
        self.open_album_value(album);
    }

    pub fn open_album_value(&mut self, album: Album) {
        self.query.clear();
        self.search = None;
        self.filter = Some(Filter::Album(album));
        self.view = View::Tracks;
        self.reload_filter();
    }

    pub fn open_artist(&mut self, artist_id: ArtistId) {
        let Some(artist) = self
            .artists
            .iter()
            .find(|artist| artist.id == artist_id)
            .cloned()
        else {
            return;
        };
        self.open_artist_value(artist);
    }

    pub fn open_artist_value(&mut self, artist: Artist) {
        self.query.clear();
        self.search = None;
        self.filter = Some(Filter::Artist(artist));
        self.view = View::Tracks;
        self.reload_filter();
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn back(&mut self) {
        self.filter = None;
        self.filtered.clear();
        self.filtered_keys.clear();
    }

    pub fn refresh_search(&mut self) {
        let query = self.query.text.trim();
        if query.is_empty() {
            self.search = None;
            return;
        }
        match self.backend.search(query, 100) {
            Ok(results) => self.search = Some(results),
            Err(error) => self.error = Some(error),
        }
    }

    pub fn play_track(&mut self, track_id: TrackId) {
        let track = self
            .current_list()
            .iter()
            .find(|track| track.id == track_id)
            .cloned();
        if let Some(track) = track {
            self.play_track_value(track);
        }
    }

    pub fn play_track_value(&mut self, track: Track) {
        self.backend.send(Command::PlayTrack(Box::new(track)));
    }

    pub fn play_all(&mut self) {
        let tracks = self.current_list().to_vec();
        if tracks.is_empty() {
            return;
        }
        self.play_tracks_value(tracks);
    }

    pub fn play_tracks_value(&mut self, tracks: Vec<Track>) {
        if tracks.is_empty() {
            return;
        }
        self.backend.send(Command::PlayTracks {
            tracks,
            start_index: 0,
        });
    }

    pub fn toggle_play(&mut self) {
        match self.playback {
            PlaybackState::Playing => self.backend.send(Command::Pause),
            PlaybackState::Paused => self.backend.send(Command::Resume),
            PlaybackState::Stopped => {
                if let Some(item) = &self.current {
                    self.backend
                        .send(Command::PlayTrack(Box::new(item.track.clone())));
                } else {
                    self.play_all();
                }
            }
        }
    }

    pub fn next(&self) {
        self.backend.send(Command::Next);
    }

    pub fn previous(&self) {
        self.backend.send(Command::Previous);
    }

    pub fn seek_fraction(&mut self, fraction: f32) {
        if !self.seekable {
            return;
        }
        let duration = self
            .duration
            .or_else(|| self.current.as_ref().map(|item| item.track.duration))
            .unwrap_or_default();
        if duration.is_zero() {
            return;
        }
        let fraction = fraction.clamp(0.0, 1.0);
        self.backend.send(Command::Seek(duration.mul_f32(fraction)));
        self.position = duration.mul_f32(fraction);
    }

    pub fn seek_relative(&mut self, seconds: f64) {
        if !self.seekable {
            return;
        }
        let duration = self
            .duration
            .or_else(|| self.current.as_ref().map(|item| item.track.duration));
        let target = if seconds < 0.0 {
            self.position
                .saturating_sub(Duration::from_secs_f64(-seconds))
        } else {
            self.position + Duration::from_secs_f64(seconds)
        };
        let target = duration.map_or(target, |duration| target.min(duration));
        self.position = target;
        self.backend.send(Command::Seek(target));
    }

    pub fn set_volume(&mut self, volume: u16) {
        self.volume = volume.min(100);
        self.backend.send(Command::SetVolume(self.volume));
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        self.backend.send(Command::SetShuffle(self.shuffle));
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat = match self.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
        self.backend.send(Command::SetRepeat(self.repeat));
    }

    pub fn clear_queue(&mut self) {
        self.backend.send(Command::ClearQueue);
    }

    pub fn remove_queue(&mut self, index: usize) {
        self.backend.send(Command::RemoveQueueItem { index });
    }

    pub fn remove_queue_id(&mut self, id: QueueItemId) {
        if let Some(index) = self.queue.iter().position(|item| item.id == id) {
            self.remove_queue(index);
        }
    }

    pub fn move_queue(&mut self, from: usize, to: usize) {
        self.backend.send(Command::MoveQueueItem { from, to });
    }

    pub fn move_queue_id(&mut self, id: QueueItemId, delta: i32) {
        let Some(index) = self.queue.iter().position(|item| item.id == id) else {
            return;
        };
        let target = if delta < 0 {
            index.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            (index + delta as usize).min(self.queue.len().saturating_sub(1))
        };
        if target != index {
            self.move_queue(index, target);
        }
    }

    pub fn add_folder(&mut self, path: PathBuf) {
        self.backend.add_folder(path);
        self.folders = self.backend.folders();
        self.error = None;
        self.view = View::Settings;
    }

    pub fn remove_folder(&mut self, path: &Path) {
        self.backend.remove_folder(path);
        self.folders = self.backend.folders();
    }

    pub fn rescan(&mut self) {
        self.backend.rescan();
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for _ in 0..128 {
            let Some(event) = self.backend.try_recv_event() else {
                break;
            };
            changed |= self.apply_event(event);
        }
        changed
    }

    fn apply_event(&mut self, event: Event) -> bool {
        match event {
            Event::PlaybackStateChanged(state) => {
                if self.playback == state {
                    return false;
                }
                self.playback = state;
                true
            }
            Event::TrackChanged(Some(item)) => {
                self.duration = (!item.track.duration.is_zero()).then_some(item.track.duration);
                self.position = Duration::ZERO;
                self.stream_title = None;
                self.stream_status = if item.track.is_local() {
                    StreamStatus::Idle
                } else {
                    StreamStatus::Connecting
                };
                self.seekable = item.track.is_local();
                self.current = Some(item);
                true
            }
            Event::TrackChanged(None) => {
                self.current = None;
                self.duration = None;
                self.stream_title = None;
                self.stream_status = StreamStatus::Idle;
                self.seekable = true;
                true
            }
            Event::PositionChanged { position, duration } => {
                if self.position == position && self.duration == duration {
                    return false;
                }
                self.position = position;
                self.duration = duration;
                true
            }
            Event::QueueChanged(queue) => {
                self.queue_keys = queue
                    .iter()
                    .map(|item| format!("queue-{}", item.id.get()))
                    .collect();
                self.queue = queue;
                true
            }
            Event::VolumeChanged(volume) => {
                if self.volume == volume {
                    return false;
                }
                self.volume = volume;
                true
            }
            Event::ShuffleChanged(shuffle) => {
                if self.shuffle == shuffle {
                    return false;
                }
                self.shuffle = shuffle;
                true
            }
            Event::RepeatChanged(repeat) => {
                if self.repeat == repeat {
                    return false;
                }
                self.repeat = repeat;
                true
            }
            Event::StreamStatusChanged(status) => {
                if self.stream_status == status {
                    return false;
                }
                self.stream_status = status;
                true
            }
            Event::StreamTitleChanged(title) => {
                if self.stream_title == title {
                    return false;
                }
                self.stream_title = title;
                true
            }
            Event::SourceResults {
                source,
                request_id: _,
                items,
                offline,
                cached,
                has_more,
            } => {
                if source == "jamendo" {
                    let loading_more = self.jamendo_loading_more;
                    self.jamendo_loading = false;
                    self.jamendo_loading_more = false;
                    self.jamendo_offline = offline;
                    self.jamendo_cached = cached;
                    self.jamendo_latency_cached = cached;
                    if let Some(started) = self.jamendo_request_at.take() {
                        self.jamendo_latency_ms = Some(started.elapsed().as_millis());
                    }
                    if loading_more {
                        self.append_jamendo_items(items);
                    } else {
                        self.set_jamendo_items(items);
                    }
                    self.jamendo_has_more = has_more;
                    if self.jamendo_selections.is_empty() && !self.jamendo_selections_requested {
                        self.request_jamendo_selections();
                    }
                    return true;
                }
                if source == "podcast" {
                    self.podcast_loading = false;
                    self.set_podcast_items(items);
                    self.list_version = self.list_version.wrapping_add(1);
                    return true;
                }
                if source != "radio" {
                    return false;
                }
                self.source_loading = false;
                self.source_offline = offline;
                self.set_source_items(items);
                self.list_version = self.list_version.wrapping_add(1);
                true
            }
            Event::SourceSearchFailed {
                source,
                request_id: _,
                message,
            } => {
                if source == "jamendo" {
                    self.jamendo_loading = false;
                    self.jamendo_loading_more = false;
                    self.jamendo_request_at = None;
                    self.error = Some(message);
                    return true;
                }
                if source == "podcast" {
                    self.podcast_loading = false;
                    self.error = Some(message);
                    return true;
                }
                if source != "radio" {
                    return false;
                }
                self.source_loading = false;
                self.error = Some(message);
                true
            }
            Event::SourceSelections {
                source,
                request_id: _,
                items,
            } => {
                if source != "jamendo" {
                    return false;
                }
                self.jamendo_selections_loading = false;
                if items.is_empty() && self.jamendo_selections_attempts < 3 {
                    // A cancelled or empty cold fetch can retry after the next
                    // result; the attempt counter prevents a request loop.
                    self.jamendo_selections_requested = false;
                }
                self.jamendo_selection_keys = items
                    .iter()
                    .map(|item| format!("jamendo-selection-{}", item.id))
                    .collect();
                self.jamendo_selections = items;
                self.list_version = self.list_version.wrapping_add(1);
                true
            }
            Event::SeekableChanged(seekable) => {
                if self.seekable == seekable {
                    return false;
                }
                self.seekable = seekable;
                true
            }
            Event::SourceEntries { source, id, result } => {
                if source != "podcast" || self.podcast_selected.as_deref() != Some(id.as_str()) {
                    return false;
                }
                self.podcast_entries_loading = false;
                match result {
                    Ok(entries) => self.podcast_entries = entries,
                    Err(message) => self.error = Some(message),
                }
                self.list_version = self.list_version.wrapping_add(1);
                true
            }
            Event::SourceArtwork { source, id, path } => {
                if source == "jamendo" {
                    let Some(item) = self.jamendo_items.iter_mut().find(|item| item.id == id)
                    else {
                        return false;
                    };
                    item.image_path = Some(path);
                    self.list_version = self.list_version.wrapping_add(1);
                    return true;
                }
                if source == "podcast" {
                    let Some(item) = self.podcast_items.iter_mut().find(|item| item.id == id)
                    else {
                        return false;
                    };
                    item.image_path = Some(path);
                    self.list_version = self.list_version.wrapping_add(1);
                    return true;
                }
                if source != "radio" {
                    return false;
                }
                let Some(item) = self.source_items.iter_mut().find(|item| item.id == id) else {
                    return false;
                };
                item.image_path = Some(path);
                self.list_version = self.list_version.wrapping_add(1);
                true
            }
            Event::ScanStarted { .. } => {
                self.scan = Some(ScanState {
                    scanned: 0,
                    total: 0,
                });
                true
            }
            Event::ScanProgress { scanned, total, .. } => {
                self.scan = Some(ScanState { scanned, total });
                true
            }
            Event::ScanFinished {
                added,
                updated,
                removed,
                ..
            } => {
                self.scan = None;
                self.scan_last = Some(i18n::scan_finished(added, updated, removed));
                self.reload();
                true
            }
            Event::LibraryChanged => {
                self.reload();
                true
            }
            Event::Error { context, message } => {
                self.error = Some(format!("{}: {message}", i18n::error_prefix(&context)));
                true
            }
            Event::Shutdown => false,
        }
    }
}

pub fn setup(state: &mut UiState, waker: Waker) {
    state.install_images(waker.clone());
    state.backend.set_waker(waker);
    if state.jamendo_available {
        // One-shot, after the UI is ready. The provider itself refuses a
        // second pass and a user search cancels the remaining warm-up.
        state.backend.source_prefetch("jamendo");
    }
}

pub fn poll(state: &mut UiState, _time: f64, _tx: &mut Tx) -> bool {
    state.poll()
}

pub fn on_key(state: &mut UiState, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.is_command('f') {
        state.query.focused = true;
        tx.focus("search", true);
        return true;
    }
    if event.is_command('r') {
        state.rescan();
        return true;
    }
    if event.is_command('q') {
        state.backend.shutdown();
        tx.close_window();
        return true;
    }
    if state.view == View::Jamendo {
        if event.mods.alt && matches!(event.key, Key::Left) {
            state.move_jamendo_genre(-1);
            return true;
        }
        if event.mods.alt && matches!(event.key, Key::Right) {
            state.move_jamendo_genre(1);
            return true;
        }
        if matches!(event.key, Key::Enter) && event.mods.cmd {
            state.play_jamendo_list(false);
            return true;
        }
        if matches!(event.key, Key::Enter) && event.mods.alt {
            state.play_jamendo_list(true);
            return true;
        }
        if matches!(event.key, Key::Enter) && !event.mods.ctrl && !event.mods.alt {
            state.play_jamendo_selected();
            return true;
        }
    }
    match &event.key {
        Key::Text(text) if text == " " => {
            state.toggle_play();
            true
        }
        Key::Left if event.mods.ctrl => {
            state.previous();
            true
        }
        Key::Up if event.mods.alt && state.view == View::Playlists => {
            if state.playlist_selected_item.is_some() {
                state.move_selected_playlist_item(-1);
                true
            } else {
                false
            }
        }
        Key::Down if event.mods.alt && state.view == View::Playlists => {
            if state.playlist_selected_item.is_some() {
                state.move_selected_playlist_item(1);
                true
            } else {
                false
            }
        }
        Key::Right if event.mods.ctrl => {
            state.next();
            true
        }
        Key::Left => {
            state.seek_relative(-5.0);
            true
        }
        Key::Right => {
            state.seek_relative(5.0);
            true
        }
        Key::Text(text) if text == "+" || text == "=" => {
            state.set_volume(state.volume.saturating_add(5));
            true
        }
        Key::Text(text) if text == "-" => {
            state.set_volume(state.volume.saturating_sub(5));
            true
        }
        Key::Escape => {
            if !state.query.text.is_empty() {
                state.query.clear();
                state.query.focused = false;
                state.search = None;
                true
            } else if state.filter.is_some() {
                state.back();
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

pub type Scene = (&'static str, fn(&mut UiState));

pub fn scenes() -> &'static [Scene] {
    &[
        ("demo", scene_demo),
        ("queue", scene_queue),
        ("settings", scene_settings),
        ("search", scene_search),
        ("albums", scene_albums),
        ("artists", scene_artists),
        ("playlists", scene_playlists),
        ("sources", scene_sources),
        ("sources-jamendo", scene_sources_jamendo),
        ("radio", scene_radio),
        ("podcast", scene_podcast),
        ("jamendo", scene_jamendo),
    ]
}

fn seed_queue(state: &mut UiState, count: usize) {
    state.queue = state
        .tracks
        .iter()
        .take(count)
        .enumerate()
        .map(|(index, track)| QueueItem {
            id: QueueItemId::new(index as u64 + 1),
            track: track.clone(),
        })
        .collect();
    state.queue_keys = state
        .queue
        .iter()
        .map(|item| format!("queue-{}", item.id.get()))
        .collect();
}

fn scene_demo(state: &mut UiState) {
    seed_queue(state, 12);
    state.current = state.queue.first().cloned();
    state.playback = PlaybackState::Playing;
    state.position = Duration::from_secs(42);
    state.duration = state.current.as_ref().map(|item| item.track.duration);
    state.volume = 68;
    state.shuffle = true;
}

fn scene_queue(state: &mut UiState) {
    seed_queue(state, 24);
    state.current = state.queue.get(2).cloned();
    state.playback = PlaybackState::Playing;
    state.position = Duration::from_secs(75);
    state.duration = state.current.as_ref().map(|item| item.track.duration);
    state.view = View::Queue;
}

fn scene_settings(state: &mut UiState) {
    state.view = View::Settings;
}

fn scene_sources(state: &mut UiState) {
    state.view = View::Sources;
}

fn scene_sources_jamendo(state: &mut UiState) {
    state.view = View::Sources;
    state.jamendo_available = true;
}

fn scene_radio(state: &mut UiState) {
    state.view = View::Radio;
    state.source_items = vec![
        musify_core::SourceItem {
            id: "demo-jazz".into(),
            source: musify_core::SourceId::radio("https://stream.example/jazz"),
            title: "Soft Jazz Radio".into(),
            subtitle: "Italy · MP3 128 kbps".into(),
            detail: "jazz, smooth, lounge".into(),
            uri: "https://stream.example/jazz".into(),
            favorite: true,
            image_path: None,
            duration: None,
            license_url: None,
        },
        musify_core::SourceItem {
            id: "demo-electro".into(),
            source: musify_core::SourceId::radio("https://stream.example/electro"),
            title: "Notturno Elettrico".into(),
            subtitle: "Germany · AAC 96 kbps".into(),
            detail: "electronic, house".into(),
            uri: "https://stream.example/electro".into(),
            favorite: false,
            image_path: None,
            duration: None,
            license_url: None,
        },
    ];
    state.source_keys = state
        .source_items
        .iter()
        .map(|item| format!("{}-{}", item.source.scheme(), item.id))
        .collect();
    state.stream_status = StreamStatus::Playing;
    state.current = Some(QueueItem {
        id: QueueItemId::new(1),
        track: musify_radio_track(),
    });
    state.stream_title = Some("Demo Artist - Demo Song".into());
    state.playback = PlaybackState::Playing;
}

fn scene_playlists(state: &mut UiState) {
    state.view = View::Playlists;
    let id = PlaylistId::new(1);
    state.playlists = vec![Playlist {
        id,
        name: "Mix serale".into(),
        created_at: 0,
        updated_at: 0,
        item_count: 3,
    }];
    state.playlist_keys = vec!["playlist-1".into()];
    state.selected_playlist = Some(id);
    state.selected_playlist_name = "Mix serale".into();
    state.playlist_name = TextEdit::new("Mix serale");
    state.playlist_items = vec![
        demo_playlist_item(1, id, 0, "Città", 200, true),
        demo_playlist_item(2, id, 1, "Notturno", 245, true),
        demo_playlist_item(3, id, 2, "Brano rimosso", 180, false),
    ];
    state.playlist_item_keys = state
        .playlist_items
        .iter()
        .map(|item| format!("playlist-item-{}", item.id.get()))
        .collect();
}

fn demo_playlist_item(
    item_id: i64,
    playlist_id: PlaylistId,
    position: u32,
    title: &str,
    seconds: u64,
    available: bool,
) -> PlaylistItem {
    let path = PathBuf::from(format!("C:/Music/{title}.mp3"));
    PlaylistItem {
        id: PlaylistItemId::new(item_id),
        playlist_id,
        position,
        source: musify_core::SourceId::local(&path),
        item_id: path.to_string_lossy().into_owned(),
        title: title.into(),
        artist: "Artist".into(),
        album: "Album".into(),
        duration: Duration::from_secs(seconds),
        cover_path: None,
        available,
    }
}

fn scene_jamendo(state: &mut UiState) {
    state.view = View::Jamendo;
    state.jamendo_available = true;
    state.jamendo_started = true;
    state.jamendo_genre = "jazz".into();
    state.jamendo_sort = JamendoSort::PopularWeek;
    state.jamendo_cached = true;
    state.jamendo_latency_ms = Some(38);
    state.jamendo_latency_cached = true;
    state.jamendo_has_more = true;
    state.jamendo_selections_requested = true;
    state.jamendo_selections = vec![
        demo_jamendo_selection("sel-1", "Indie Night", "Jamendo Curator"),
        demo_jamendo_selection("sel-2", "Electro Focus", "Jamendo Curator"),
        demo_jamendo_selection("sel-3", "Acoustic Morning", "Jamendo Curator"),
    ];
    state.jamendo_selection_keys = state
        .jamendo_selections
        .iter()
        .map(|item| format!("jamendo-selection-{}", item.id))
        .collect();
    state.jamendo_items = vec![
        demo_jamendo_item("42", "Creative Track", "Demo Artist", "Demo Album", 215),
        demo_jamendo_item("43", "Late Night Groove", "Nova", "After Hours", 188),
        demo_jamendo_item("44", "Marea", "Kanso", "Onde", 242),
        demo_jamendo_item("45", "Vetro e sale", "Marta & i Vetri", "Sale", 201),
        demo_jamendo_item("46", "Luce blu", "Lucia Ferri", "Notturno", 176),
        demo_jamendo_item("47", "Distanze", "Sole Alto", "Orizzonte", 229),
    ];
    state.jamendo_keys = state
        .jamendo_items
        .iter()
        .map(|item| format!("jamendo-{}", item.id))
        .collect();
    state.current = Some(QueueItem {
        id: QueueItemId::new(1),
        track: musify_jamendo_track(),
    });
    state.playback = PlaybackState::Playing;
    state.duration = Some(Duration::from_secs(215));
    state.position = Duration::from_secs(42);
}

fn demo_jamendo_item(
    id: &str,
    title: &str,
    artist: &str,
    album: &str,
    seconds: u64,
) -> musify_core::SourceItem {
    musify_core::SourceItem {
        id: id.into(),
        source: musify_core::SourceId::jamendo(format!("https://audio.example/{id}.mp3")),
        title: title.into(),
        subtitle: format!("{artist} · {album}"),
        detail: "CC BY-SA".into(),
        uri: format!("https://audio.example/{id}.mp3"),
        favorite: false,
        image_path: None,
        duration: Some(Duration::from_secs(seconds)),
        license_url: Some("https://creativecommons.org/licenses/by-sa/3.0/".into()),
    }
}

fn demo_jamendo_selection(id: &str, title: &str, curator: &str) -> musify_core::SourceItem {
    musify_core::SourceItem {
        id: id.into(),
        source: musify_core::SourceId::jamendo(""),
        title: title.into(),
        subtitle: curator.into(),
        detail: String::new(),
        uri: String::new(),
        favorite: false,
        image_path: None,
        duration: None,
        license_url: None,
    }
}

fn musify_jamendo_track() -> Track {
    Track {
        id: TrackId::new(4242),
        source: musify_core::SourceId::jamendo("https://audio.example/42.mp3"),
        path: PathBuf::new(),
        title: "Creative Track".into(),
        artist: "Demo Artist".into(),
        album: "Demo Album · CC BY-SA · Jamendo".into(),
        album_artist: None,
        track_number: None,
        disc_number: None,
        duration: Duration::from_secs(215),
        year: None,
        genre: Some("CC BY-SA".into()),
        cover_path: None,
        added_at: 0,
    }
}

fn scene_podcast(state: &mut UiState) {
    state.view = View::Podcast;
    state.podcast_items = vec![musify_core::SourceItem {
        id: "demo-podcast".into(),
        source: musify_core::SourceId::podcast("https://example.com/feed.xml"),
        title: "Demo Podcast".into(),
        subtitle: "Demo Author".into(),
        detail: "Technology".into(),
        uri: "https://example.com/feed.xml".into(),
        favorite: true,
        image_path: None,
        duration: None,
        license_url: None,
    }];
    state.podcast_keys = state
        .podcast_items
        .iter()
        .map(|item| format!("{}-{}", item.source.scheme(), item.id))
        .collect();
    state.podcast_selected = Some("demo-podcast".into());
    state.podcast_selected_title = "Demo Podcast".into();
    state.podcast_entries = vec![musify_core::SourceEntry {
        id: "demo-episode".into(),
        source: musify_core::SourceId::podcast("https://example.com/one.mp3"),
        title: "Episodio di prova".into(),
        subtitle: "2026-11-10 · 42 min".into(),
        detail: "Una descrizione breve dell’episodio.".into(),
        uri: "https://example.com/one.mp3".into(),
        duration: Some(Duration::from_secs(42 * 60)),
        published_at: Some(1_762_771_200),
        watched: false,
        position: Some(Duration::from_secs(120)),
        image_path: None,
    }];
}

fn musify_radio_track() -> Track {
    Track {
        id: TrackId::new(999),
        source: musify_core::SourceId::radio("https://stream.example/jazz"),
        path: PathBuf::new(),
        title: "Soft Jazz Radio".into(),
        artist: "Soft Jazz Radio".into(),
        album: "Radio".into(),
        album_artist: None,
        track_number: None,
        disc_number: None,
        duration: Duration::ZERO,
        year: None,
        genre: None,
        cover_path: None,
        added_at: 0,
    }
}

fn scene_search(state: &mut UiState) {
    state.query = TextEdit::new("Città");
    state.query.focused = true;
    state.refresh_search();
}

fn scene_albums(state: &mut UiState) {
    state.view = View::Albums;
}

fn scene_artists(state: &mut UiState) {
    state.view = View::Artists;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use musify_core::{
        Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, RepeatMode, SearchResults,
        SourceItem, Track, TrackSort,
    };

    use super::*;

    #[derive(Default)]
    struct RecordingBackend {
        queries: Mutex<Vec<(String, SourceQuery)>>,
        actions: Mutex<Vec<String>>,
        selection_requests: AtomicUsize,
        cancel_requests: AtomicUsize,
        playlist_name: Mutex<Option<String>>,
    }

    impl RecordingBackend {
        fn last_query(&self) -> (String, SourceQuery) {
            self.queries
                .lock()
                .expect("queries")
                .last()
                .cloned()
                .expect("a source query")
        }

        fn actions(&self) -> Vec<String> {
            self.actions.lock().expect("actions").clone()
        }
    }

    impl Backend for RecordingBackend {
        fn send(&self, _command: Command) {}

        fn try_recv_event(&self) -> Option<Event> {
            None
        }

        fn set_waker(&self, _waker: Arc<dyn Fn() + Send + Sync>) {}

        fn tracks(
            &self,
            _sort: TrackSort,
            _limit: usize,
            _offset: usize,
        ) -> Result<Vec<Track>, String> {
            Ok(Vec::new())
        }

        fn tracks_for_album(&self, _album_id: AlbumId) -> Result<Vec<Track>, String> {
            Ok(Vec::new())
        }

        fn tracks_for_artist(&self, _artist_id: ArtistId) -> Result<Vec<Track>, String> {
            Ok(Vec::new())
        }

        fn albums(&self) -> Result<Vec<Album>, String> {
            Ok(Vec::new())
        }

        fn artists(&self) -> Result<Vec<Artist>, String> {
            Ok(Vec::new())
        }

        fn search(&self, _query: &str, _limit: usize) -> Result<SearchResults, String> {
            Ok(SearchResults::default())
        }

        fn stats(&self) -> Result<LibraryStats, String> {
            Ok(LibraryStats::default())
        }

        fn folders(&self) -> Vec<PathBuf> {
            Vec::new()
        }

        fn add_folder(&self, _path: PathBuf) {}

        fn remove_folder(&self, _path: &Path) {}

        fn rescan(&self) {}

        fn source_search(&self, source: &str, query: SourceQuery) {
            self.queries
                .lock()
                .expect("queries")
                .push((source.to_string(), query));
        }

        fn source_selections(&self, source: &str) {
            if source == "jamendo" {
                self.selection_requests.fetch_add(1, Ordering::AcqRel);
            }
        }

        fn source_cancel(&self, source: &str) {
            if source == "jamendo" {
                self.cancel_requests.fetch_add(1, Ordering::AcqRel);
            }
        }

        fn source_play_list(
            &self,
            source: &str,
            ids: &[String],
            shuffle: bool,
        ) -> Result<usize, String> {
            self.actions
                .lock()
                .expect("actions")
                .push(format!("play:{source}:{}:{shuffle}", ids.len()));
            Ok(ids.len())
        }

        fn source_enqueue_list(&self, source: &str, ids: &[String]) -> Result<usize, String> {
            self.actions
                .lock()
                .expect("actions")
                .push(format!("enqueue:{source}:{}", ids.len()));
            Ok(ids.len())
        }

        fn source_save_list_as_playlist(
            &self,
            source: &str,
            ids: &[String],
            name: &str,
        ) -> Result<PlaylistId, String> {
            self.actions
                .lock()
                .expect("actions")
                .push(format!("save:{source}:{}", ids.len()));
            *self.playlist_name.lock().expect("playlist name") = Some(name.to_string());
            Ok(PlaylistId::new(7))
        }

        fn source_available(&self, source: &str) -> bool {
            source == "jamendo"
        }

        fn playback_settings(&self) -> (u16, bool, RepeatMode) {
            (80, false, RepeatMode::Off)
        }

        fn output_info(&self) -> (u32, u16) {
            (48_000, 2)
        }

        fn shutdown(&self) {}
    }

    fn backend() -> Arc<RecordingBackend> {
        Arc::new(RecordingBackend::default())
    }

    fn item(id: &str) -> SourceItem {
        SourceItem {
            id: id.into(),
            source: musify_core::SourceId::jamendo(format!("https://audio.example/{id}.mp3")),
            title: format!("Track {id}"),
            subtitle: "Artist · Album".into(),
            detail: "CC BY-SA".into(),
            uri: format!("https://audio.example/{id}.mp3"),
            favorite: false,
            image_path: None,
            duration: Some(Duration::from_secs(180)),
            license_url: Some("https://creativecommons.org/licenses/by-sa/3.0/".into()),
        }
    }

    #[test]
    fn opening_jamendo_loads_the_first_genre_with_the_default_sort() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        state.set_view(View::Jamendo);
        let (source, query) = backend.last_query();
        assert_eq!(source, "jamendo");
        assert_eq!(query.filter("tag"), Some("jazz"));
        assert_eq!(query.filter("order"), Some("popular_week"));
        assert_eq!(query.filter("offset"), Some("0"));
        assert_eq!(query.filter("limit"), Some("60"));
        assert!(state.jamendo_loading);
    }

    #[test]
    fn search_button_requests_a_fresh_response() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        state.set_view(View::Jamendo);
        state.search_jamendo();
        let (_, query) = backend.last_query();
        assert_eq!(query.filter("refresh"), Some("1"));
        assert_eq!(query.filter("tag"), Some("jazz"));
    }

    #[test]
    fn genre_sort_and_pagination_produce_the_expected_queries() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        state.set_view(View::Jamendo);
        state.select_jamendo_genre("rock");
        let (_, query) = backend.last_query();
        assert_eq!(query.filter("tag"), Some("rock"));
        assert_eq!(query.filter("order"), Some("popular_week"));

        state.set_jamendo_sort(JamendoSort::Newest);
        let (_, query) = backend.last_query();
        assert_eq!(query.filter("tag"), Some("rock"));
        assert_eq!(query.filter("order"), Some("newest"));

        state.apply_event(Event::SourceResults {
            source: "jamendo".into(),
            request_id: 1,
            items: vec![item("1"), item("2")],
            offline: false,
            cached: true,
            has_more: true,
        });
        assert!(state.jamendo_has_more);
        assert!(state.jamendo_cached);
        assert!(state.jamendo_latency_ms.is_some());
        state.load_more_jamendo();
        let (_, query) = backend.last_query();
        assert_eq!(query.filter("offset"), Some("60"));
        assert!(state.jamendo_loading_more);
    }

    #[test]
    fn opening_a_selection_uses_its_id_and_returns_to_genres() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        state.set_view(View::Jamendo);
        state.open_jamendo_selection("sel-1");
        let (_, query) = backend.last_query();
        assert_eq!(query.filter("selection"), Some("sel-1"));
        assert_eq!(query.filter("tag"), None);
        state.back_to_jamendo_genres();
        let (_, query) = backend.last_query();
        assert_eq!(query.filter("selection"), None);
        assert_eq!(query.filter("tag"), Some("jazz"));
    }

    #[test]
    fn list_actions_send_all_visible_ids_and_auto_name_the_playlist() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        state.jamendo_items = vec![item("1"), item("2"), item("3")];
        state.jamendo_keys = vec!["jamendo-1".into(), "jamendo-2".into(), "jamendo-3".into()];
        state.play_jamendo_list(true);
        state.enqueue_jamendo_list();
        state.save_jamendo_playlist();
        assert_eq!(
            backend.actions(),
            vec![
                "play:jamendo:3:true".to_string(),
                "enqueue:jamendo:3".to_string(),
                "save:jamendo:3".to_string(),
            ]
        );
        let name = backend
            .playlist_name
            .lock()
            .expect("playlist name")
            .clone()
            .expect("saved name");
        assert!(name.contains("Jamendo"));
        assert!(name.contains("Jazz"));
    }

    #[test]
    fn source_results_request_selections_only_once() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        let event = Event::SourceResults {
            source: "jamendo".into(),
            request_id: 1,
            items: vec![item("1")],
            offline: false,
            cached: false,
            has_more: false,
        };
        state.apply_event(event.clone());
        state.apply_event(event);
        assert_eq!(backend.selection_requests.load(Ordering::Acquire), 1);
        assert_eq!(state.jamendo_items.len(), 1);
    }

    #[test]
    fn applying_sixty_results_is_fast() {
        let backend = backend();
        let mut state = UiState::new(backend);
        state.jamendo_selections_requested = true;
        let items: Vec<SourceItem> = (0..60).map(|index| item(&index.to_string())).collect();
        let started = Instant::now();
        state.apply_event(Event::SourceResults {
            source: "jamendo".into(),
            request_id: 1,
            items,
            offline: false,
            cached: true,
            has_more: true,
        });
        let elapsed = started.elapsed();
        println!("ui apply 60 items: {elapsed:?}");
        assert_eq!(state.jamendo_items.len(), 60);
        assert!(elapsed < Duration::from_millis(100));
    }

    #[test]
    fn source_selections_event_fills_the_section() {
        let backend = backend();
        let mut state = UiState::new(backend);
        assert!(state.apply_event(Event::SourceSelections {
            source: "jamendo".into(),
            request_id: 0,
            items: vec![item("sel-1")],
        }));
        assert_eq!(state.jamendo_selections.len(), 1);
        assert_eq!(
            state.jamendo_selection_keys,
            vec!["jamendo-selection-sel-1"]
        );
    }

    #[test]
    fn leaving_jamendo_cancels_the_pending_request() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        state.set_view(View::Jamendo);
        state.set_view(View::Tracks);
        assert_eq!(backend.cancel_requests.load(Ordering::Acquire), 1);
        assert!(!state.jamendo_loading);
    }

    #[test]
    fn an_empty_selection_response_can_retry_after_the_next_result() {
        let backend = backend();
        let mut state = UiState::new(backend.clone());
        state.request_jamendo_selections();
        assert_eq!(backend.selection_requests.load(Ordering::Acquire), 1);
        state.apply_event(Event::SourceSelections {
            source: "jamendo".into(),
            request_id: 0,
            items: Vec::new(),
        });
        assert!(!state.jamendo_selections_requested);
        state.apply_event(Event::SourceResults {
            source: "jamendo".into(),
            request_id: 1,
            items: vec![item("1")],
            offline: false,
            cached: false,
            has_more: false,
        });
        assert_eq!(backend.selection_requests.load(Ordering::Acquire), 2);
        assert_eq!(state.jamendo_selections_attempts, 2);
    }
}
