use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, PlaybackState, QueueItem,
    QueueItemId, RepeatMode, SearchResults, SourceItem, StreamStatus, Track, TrackId, TrackSort,
};
use rune::{Image, ImageCache, Key, KeyEvent, TextEdit, Tx, Waker};

use crate::backend::{Backend, RadioQuery};
use crate::i18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tracks,
    Albums,
    Artists,
    Queue,
    Sources,
    Radio,
    Settings,
}

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
    pub radio_items: Vec<SourceItem>,
    pub radio_keys: Vec<String>,
    pub radio_loading: bool,
    pub radio_offline: bool,
    pub stream_title: Option<String>,
    pub stream_status: StreamStatus,
}

impl UiState {
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        let (volume, shuffle, repeat) = backend.playback_settings();
        let output = backend.output_info();
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
            radio_items: Vec::new(),
            radio_keys: Vec::new(),
            radio_loading: false,
            radio_offline: false,
            stream_title: None,
            stream_status: StreamStatus::Idle,
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
        self.view = view;
        if matches!(view, View::Tracks | View::Albums | View::Artists) {
            self.filter = None;
            self.filtered.clear();
            self.filtered_keys.clear();
        }
        if view == View::Radio {
            self.refresh_radio_favorites();
        }
    }

    pub fn refresh_radio_favorites(&mut self) {
        match self.backend.radio_favorites() {
            Ok(items) => self.set_radio_items(items),
            Err(error) => self.error = Some(error),
        }
    }

    pub fn search_radio(&mut self) {
        let query = RadioQuery {
            name: self.radio_query.name.text.trim().to_string(),
            country: self.radio_query.country.text.trim().to_string(),
            tag: self.radio_query.tag.text.trim().to_string(),
        };
        if query.name.is_empty() && query.country.is_empty() && query.tag.is_empty() {
            self.refresh_radio_favorites();
            return;
        }
        self.radio_loading = true;
        self.radio_offline = false;
        self.backend.radio_search(query);
    }

    fn set_radio_items(&mut self, items: Vec<SourceItem>) {
        self.radio_keys = items
            .iter()
            .map(|item| format!("radio-{}", item.id))
            .collect();
        self.radio_items = items;
    }

    pub fn toggle_radio_favorite(&mut self, id: &str) {
        match self.backend.radio_toggle_favorite(id) {
            Ok(favorite) => {
                if let Some(item) = self.radio_items.iter_mut().find(|item| item.id == id) {
                    item.favorite = favorite;
                }
                self.list_version = self.list_version.wrapping_add(1);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn play_radio(&mut self, id: &str) {
        self.backend.radio_play(id);
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
        if self
            .current
            .as_ref()
            .is_some_and(|item| !item.track.is_local())
        {
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
        if self
            .current
            .as_ref()
            .is_some_and(|item| !item.track.is_local())
        {
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
                self.current = Some(item);
                true
            }
            Event::TrackChanged(None) => {
                self.current = None;
                self.duration = None;
                self.stream_title = None;
                self.stream_status = StreamStatus::Idle;
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
            } => {
                if source != "radio" {
                    return false;
                }
                self.radio_loading = false;
                self.radio_offline = offline;
                self.set_radio_items(items);
                self.list_version = self.list_version.wrapping_add(1);
                true
            }
            Event::SourceSearchFailed {
                source,
                request_id: _,
                message,
            } => {
                if source != "radio" {
                    return false;
                }
                self.radio_loading = false;
                self.error = Some(message);
                true
            }
            Event::SourceArtwork { source, id, path } => {
                if source != "radio" {
                    return false;
                }
                let Some(item) = self.radio_items.iter_mut().find(|item| item.id == id) else {
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
    match &event.key {
        Key::Text(text) if text == " " => {
            state.toggle_play();
            true
        }
        Key::Left if event.mods.ctrl => {
            state.previous();
            true
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
        ("sources", scene_sources),
        ("radio", scene_radio),
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

fn scene_radio(state: &mut UiState) {
    state.view = View::Radio;
    state.radio_items = vec![
        musify_core::SourceItem {
            id: "demo-jazz".into(),
            source: musify_core::SourceId::radio("https://stream.example/jazz"),
            title: "Soft Jazz Radio".into(),
            subtitle: "Italy · MP3 128 kbps".into(),
            detail: "jazz, smooth, lounge".into(),
            uri: "https://stream.example/jazz".into(),
            favorite: true,
            image_path: None,
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
        },
    ];
    state.radio_keys = state
        .radio_items
        .iter()
        .map(|item| format!("radio-{}", item.id))
        .collect();
    state.stream_status = StreamStatus::Playing;
    state.current = Some(QueueItem {
        id: QueueItemId::new(1),
        track: musify_radio_track(),
    });
    state.stream_title = Some("Demo Artist - Demo Song".into());
    state.playback = PlaybackState::Playing;
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
