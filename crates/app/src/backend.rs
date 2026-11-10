use std::collections::HashSet;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use musify_audio::AudioEngine;
use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, PlaybackState, RepeatMode,
    SearchResults, SourceItem, StreamStatus, Track, TrackId, TrackSort,
};
use musify_library::{Library, LibraryWatcher};
use musify_radio::{RadioBrowser, RadioStore, SearchQuery};
use musify_ui::{Backend, RadioQuery};
use serde::{Deserialize, Serialize};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};

type Waker = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    music_folders: Vec<PathBuf>,
    volume: u16,
    shuffle: bool,
    repeat: RepeatMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_folders: Vec::new(),
            volume: 80,
            shuffle: false,
            repeat: RepeatMode::Off,
        }
    }
}

pub struct AppBackend {
    data_dir: PathBuf,
    config_path: PathBuf,
    state_path: PathBuf,
    config: Arc<Mutex<Config>>,
    library: Arc<Library>,
    radio: Arc<RadioBrowser>,
    radio_request: Arc<AtomicU64>,
    icon_inflight: Arc<Mutex<HashSet<String>>>,
    audio: Mutex<Option<AudioEngine>>,
    events: Mutex<Receiver<Event>>,
    events_tx: Sender<Event>,
    waker: Arc<Mutex<Option<Waker>>>,
    scan: ScanCoordinator,
    watcher: Mutex<Option<LibraryWatcher>>,
    smtc: Mutex<Option<MediaControls>>,
    smtc_snapshot: Arc<Mutex<SmtcSnapshot>>,
    last_state_save: Mutex<Instant>,
}

#[derive(Debug, Default)]
struct SmtcSnapshot {
    playback: PlaybackState,
    position: Duration,
    duration: Option<Duration>,
    current: Option<musify_core::QueueItem>,
    metadata_track: Option<TrackId>,
    stream_title: Option<String>,
    stream_status: Option<StreamStatus>,
    last_sync: Option<Instant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedState {
    queue: Vec<String>,
    current_index: usize,
    position_ms: u64,
}

impl AppBackend {
    pub fn new() -> anyhow::Result<Self> {
        let started = Instant::now();
        let data_dir = app_data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let config_path = data_dir.join("config.toml");
        let state_path = data_dir.join("state.json");
        let mut config = load_config(&config_path);
        if let Some(folders) = env_folders() {
            config.music_folders = folders;
        }
        let library = Arc::new(Library::open(
            data_dir.join("library.sqlite3"),
            data_dir.join("covers"),
        )?);
        let radio_store = Arc::new(RadioStore::open(&data_dir.join("radio.sqlite3"))?);
        let radio = Arc::new(RadioBrowser::new(radio_store));
        let (event_tx, event_rx) = unbounded();
        let waker = Arc::new(Mutex::new(None::<Waker>));
        let config = Arc::new(Mutex::new(config));
        let scan = ScanCoordinator {
            library: Arc::clone(&library),
            config: Arc::clone(&config),
            events: event_tx.clone(),
            waker: Arc::clone(&waker),
            scanning: Arc::new(AtomicBool::new(false)),
        };

        let audio = if std::env::var("MUSIFY_NO_AUDIO").as_deref() == Ok("1") {
            None
        } else {
            let volume = config.lock().map_or(80, |config| config.volume);
            match AudioEngine::start(volume) {
                Ok(engine) => {
                    let audio_events = engine.event_receiver();
                    let events = event_tx.clone();
                    let bridge_waker = Arc::clone(&waker);
                    if let Err(error) = thread::Builder::new()
                        .name("musify-audio-bridge".into())
                        .spawn(move || {
                            while let Ok(event) = audio_events.recv() {
                                if events.send(event).is_err() {
                                    break;
                                }
                                wake(&bridge_waker);
                            }
                        })
                    {
                        let _ = event_tx.send(Event::Error {
                            context: "audio bridge".into(),
                            message: error.to_string(),
                        });
                    }
                    Some(engine)
                }
                Err(error) => {
                    let _ = event_tx.send(Event::Error {
                        context: "audio".into(),
                        message: error.to_string(),
                    });
                    None
                }
            }
        };

        let backend = Self {
            data_dir,
            config_path,
            state_path,
            config,
            library,
            radio,
            radio_request: Arc::new(AtomicU64::new(0)),
            icon_inflight: Arc::new(Mutex::new(HashSet::new())),
            audio: Mutex::new(audio),
            events: Mutex::new(event_rx),
            events_tx: event_tx,
            waker,
            scan,
            watcher: Mutex::new(None),
            smtc: Mutex::new(None),
            smtc_snapshot: Arc::new(Mutex::new(SmtcSnapshot::default())),
            last_state_save: Mutex::new(Instant::now() - Duration::from_secs(10)),
        };
        if let Ok(config) = backend.config.lock() {
            let shuffle = config.shuffle;
            let repeat = config.repeat;
            drop(config);
            backend.send_audio(Command::SetShuffle(shuffle));
            backend.send_audio(Command::SetRepeat(repeat));
        }
        backend.restore_state();
        if let Ok(url) = std::env::var("MUSIFY_AUTOPLAY_RADIO")
            && !url.trim().is_empty()
        {
            let station = musify_radio::Station {
                id: "autoplay-radio".into(),
                name: "Radio test".into(),
                url_resolved: url,
                ..musify_radio::Station::default()
            };
            backend.send_audio(Command::PlayTrack(Box::new(station.to_track())));
        }
        if std::env::var("MUSIFY_AUTOPLAY").as_deref() == Ok("1")
            && let Ok(tracks) = backend.library.tracks_sorted(
                TrackSort::Title,
                musify_core::SortDirection::Ascending,
                100,
                0,
            )
            && !tracks.is_empty()
        {
            backend.send_audio(Command::PlayTracks {
                tracks,
                start_index: 0,
            });
        }
        backend.restart_watcher();
        backend.scan.start();
        tracing::info!(
            elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "backend pronto"
        );
        Ok(backend)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn folders(&self) -> Vec<PathBuf> {
        self.config
            .lock()
            .map(|config| config.music_folders.clone())
            .unwrap_or_default()
    }

    fn restart_watcher(&self) {
        let folders = self.folders();
        let mut slot = match self.watcher.lock() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        *slot = None;
        if folders.is_empty() {
            return;
        }
        let (changed_tx, changed_rx) = bounded(16);
        match LibraryWatcher::start(&folders, changed_tx) {
            Ok(watcher) => {
                *slot = Some(watcher);
                let scan = self.scan.clone();
                if let Err(error) =
                    thread::Builder::new()
                        .name("musify-watcher".into())
                        .spawn(move || {
                            while changed_rx.recv().is_ok() {
                                scan.start();
                            }
                        })
                {
                    tracing::warn!(%error, "cannot start the watcher consumer thread");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "cannot start the music folder watcher");
            }
        }
    }

    fn update_config(&self, update: impl FnOnce(&mut Config)) {
        let mut config = match self.config.lock() {
            Ok(config) => config,
            Err(poisoned) => poisoned.into_inner(),
        };
        update(&mut config);
        if let Err(error) = save_config(&self.config_path, &config) {
            tracing::warn!(%error, "cannot save config");
        }
    }

    fn send_audio(&self, command: Command) {
        if let Ok(audio) = self.audio.lock()
            && let Some(audio) = audio.as_ref()
            && let Err(error) = audio.send(command)
        {
            tracing::warn!(%error, "audio command failed");
        }
    }

    fn restore_state(&self) {
        let Ok(text) = std::fs::read_to_string(&self.state_path) else {
            return;
        };
        let saved: SavedState = match serde_json::from_str(&text) {
            Ok(saved) => saved,
            Err(error) => {
                tracing::warn!(%error, "cannot parse state.json");
                return;
            }
        };
        let mut tracks = Vec::new();
        for path in &saved.queue {
            match self.library.track_by_path(Path::new(path)) {
                Ok(Some(track)) => tracks.push(track),
                Ok(None) => tracing::debug!(path, "saved queue track is no longer in the library"),
                Err(error) => tracing::warn!(%error, path, "cannot load saved queue track"),
            }
        }
        if tracks.is_empty() {
            return;
        }
        let start = saved.current_index.min(tracks.len() - 1);
        let position = Duration::from_millis(saved.position_ms);
        if let Ok(mut snapshot) = self.smtc_snapshot.lock() {
            snapshot.current = Some(musify_core::QueueItem {
                id: musify_core::QueueItemId::new(1),
                track: tracks[start].clone(),
            });
            snapshot.playback = PlaybackState::Paused;
            snapshot.position = position;
            snapshot.duration = Some(tracks[start].duration);
            snapshot.metadata_track = None;
        }
        self.send_audio(Command::RestoreQueue {
            tracks,
            start_index: start,
            position,
        });
    }

    fn save_state(&self, force: bool) {
        if !force {
            let Ok(last) = self.last_state_save.lock() else {
                return;
            };
            if last.elapsed() < Duration::from_secs(2) {
                return;
            }
        }
        let Ok(audio) = self.audio.lock() else {
            return;
        };
        let Some(audio) = audio.as_ref() else {
            return;
        };
        let snapshot = audio.snapshot();
        // Live streams are deliberately not persisted. Local items keep their
        // order; the current index is remapped to the local-only queue.
        let mut queue = Vec::new();
        let mut current_index = 0;
        for item in &snapshot.queue {
            if !item.track.is_local() {
                continue;
            }
            if snapshot
                .current
                .as_ref()
                .is_some_and(|current| current.id == item.id)
            {
                current_index = queue.len();
            }
            queue.push(item.track.path.to_string_lossy().into_owned());
        }
        let position = if snapshot
            .current
            .as_ref()
            .is_some_and(|current| current.track.is_local())
        {
            snapshot.position
        } else {
            Duration::ZERO
        };
        let saved = SavedState {
            queue,
            current_index,
            position_ms: u64::try_from(position.as_millis()).unwrap_or(u64::MAX),
        };
        let write = || -> anyhow::Result<()> {
            let json = serde_json::to_vec_pretty(&saved)?;
            let temporary = self.state_path.with_extension("json.tmp");
            std::fs::write(&temporary, json)?;
            std::fs::rename(temporary, &self.state_path)?;
            Ok(())
        };
        if let Err(error) = write() {
            tracing::warn!(%error, "cannot save state.json");
        } else if let Ok(mut last) = self.last_state_save.lock() {
            *last = Instant::now();
        }
    }

    fn observe_event(&self, event: &Event) {
        let mut force_metadata = false;
        let mut force_sync = false;
        if let Ok(mut snapshot) = self.smtc_snapshot.lock() {
            match event {
                Event::TrackChanged(Some(item)) => {
                    snapshot.current = Some(item.clone());
                    snapshot.duration =
                        (!item.track.duration.is_zero()).then_some(item.track.duration);
                    snapshot.position = Duration::ZERO;
                    snapshot.stream_title = None;
                    snapshot.stream_status = None;
                    force_metadata = true;
                    force_sync = true;
                }
                Event::TrackChanged(None) => {
                    snapshot.current = None;
                    snapshot.duration = None;
                    snapshot.stream_title = None;
                    snapshot.stream_status = None;
                    force_metadata = true;
                    force_sync = true;
                }
                Event::StreamTitleChanged(title) => {
                    snapshot.stream_title.clone_from(title);
                    force_metadata = true;
                }
                Event::StreamStatusChanged(status) => {
                    snapshot.stream_status = Some(status.clone());
                    force_sync = true;
                }
                Event::PlaybackStateChanged(state) => {
                    snapshot.playback = *state;
                    force_sync = true;
                }
                Event::PositionChanged { position, duration } => {
                    snapshot.position = *position;
                    snapshot.duration = *duration;
                }
                _ => {}
            }
        }
        if matches!(
            event,
            Event::TrackChanged(_)
                | Event::QueueChanged(_)
                | Event::PlaybackStateChanged(_)
                | Event::PositionChanged { .. }
        ) {
            self.save_state(false);
        }
        if force_sync || matches!(event, Event::PositionChanged { .. }) {
            self.update_smtc(force_metadata);
        }
    }

    fn update_smtc(&self, force_metadata: bool) {
        let Ok(mut controls) = self.smtc.lock() else {
            return;
        };
        let Some(controls) = controls.as_mut() else {
            return;
        };
        let Ok(mut snapshot) = self.smtc_snapshot.lock() else {
            return;
        };
        if !force_metadata
            && snapshot
                .last_sync
                .is_some_and(|last| last.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        snapshot.last_sync = Some(Instant::now());
        let current_track = snapshot.current.as_ref().map(|item| item.track.id);
        if force_metadata || snapshot.metadata_track != current_track {
            if let Some(item) = &snapshot.current {
                let cover_url = item
                    .track
                    .cover_path
                    .as_ref()
                    .map(|path| format!("file://{}", path.display()));
                let title = snapshot
                    .stream_title
                    .as_deref()
                    .unwrap_or(item.track.title.as_str());
                let metadata = MediaMetadata {
                    title: Some(title),
                    artist: Some(item.track.artist.as_str()),
                    album: Some(item.track.album.as_str()),
                    cover_url: cover_url.as_deref(),
                    duration: Some(item.track.duration),
                };
                if let Err(error) = controls.set_metadata(metadata) {
                    tracing::warn!(%error, "cannot update Windows media metadata");
                }
            }
            snapshot.metadata_track = current_track;
        }
        let playback = match snapshot.playback {
            PlaybackState::Playing => MediaPlayback::Playing {
                progress: Some(MediaPosition(snapshot.position)),
            },
            PlaybackState::Paused => MediaPlayback::Paused {
                progress: Some(MediaPosition(snapshot.position)),
            },
            PlaybackState::Stopped => MediaPlayback::Stopped,
        };
        if let Err(error) = controls.set_playback(playback) {
            tracing::warn!(%error, "cannot update Windows media playback state");
        }
    }

    #[cfg(target_os = "windows")]
    fn start_smtc(&self, hwnd: isize) {
        if self.smtc.lock().is_ok_and(|controls| controls.is_some()) {
            return;
        }
        let config = PlatformConfig {
            display_name: "Musify",
            dbus_name: "musify",
            hwnd: Some(hwnd as *mut c_void),
        };
        match MediaControls::new(config) {
            Ok(mut controls) => {
                let command_tx = self
                    .audio
                    .lock()
                    .ok()
                    .and_then(|audio| audio.as_ref().map(AudioEngine::command_sender));
                let snapshot = Arc::clone(&self.smtc_snapshot);
                let attached = controls.attach(move |event| {
                    tracing::info!(?event, "evento Windows SMTC");
                    let command = match event {
                        MediaControlEvent::Play | MediaControlEvent::Toggle => {
                            Some(Command::TogglePlayPause)
                        }
                        MediaControlEvent::Pause => Some(Command::Pause),
                        MediaControlEvent::Stop => Some(Command::Stop),
                        MediaControlEvent::Next => Some(Command::Next),
                        MediaControlEvent::Previous => Some(Command::Previous),
                        MediaControlEvent::SetPosition(MediaPosition(position)) => {
                            Some(Command::Seek(position))
                        }
                        MediaControlEvent::Seek(direction) => {
                            let position = snapshot
                                .lock()
                                .map_or(Duration::ZERO, |snapshot| snapshot.position);
                            let target = if direction == SeekDirection::Forward {
                                position + Duration::from_secs(10)
                            } else {
                                position.saturating_sub(Duration::from_secs(10))
                            };
                            Some(Command::Seek(target))
                        }
                        _ => None,
                    };
                    if let (Some(command_tx), Some(command)) = (&command_tx, command) {
                        let _ = command_tx.send(command);
                    }
                });
                if let Err(error) = attached {
                    tracing::warn!(%error, "cannot attach Windows media controls");
                    return;
                }
                if let Ok(mut slot) = self.smtc.lock() {
                    *slot = Some(controls);
                }
                self.update_smtc(true);
                tracing::info!("Windows SMTC collegato");
            }
            Err(error) => {
                tracing::warn!(%error, "cannot create Windows media controls");
            }
        }
    }
}

impl Drop for AppBackend {
    fn drop(&mut self) {
        self.save_state(true);
        if let Ok(audio) = self.audio.lock()
            && let Some(audio) = audio.as_ref()
        {
            let _ = audio.send(Command::Shutdown);
        }
    }
}

impl Backend for AppBackend {
    fn send(&self, command: Command) {
        match &command {
            Command::SetVolume(volume) => {
                self.update_config(|config| config.volume = (*volume).min(100));
            }
            Command::SetShuffle(shuffle) => {
                self.update_config(|config| config.shuffle = *shuffle);
            }
            Command::SetRepeat(repeat) => {
                self.update_config(|config| config.repeat = *repeat);
            }
            _ => {}
        }
        self.send_audio(command);
    }

    fn try_recv_event(&self) -> Option<Event> {
        let event = {
            let events = match self.events.lock() {
                Ok(events) => events,
                Err(poisoned) => poisoned.into_inner(),
            };
            events.try_recv().ok()
        };
        if let Some(event) = &event {
            self.observe_event(event);
        }
        event
    }

    fn set_waker(&self, waker: Waker) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(waker);
        }
    }

    fn window_ready(&self, hwnd: isize) {
        #[cfg(target_os = "windows")]
        if std::env::var("MUSIFY_NO_SMTC").as_deref() != Ok("1") {
            self.start_smtc(hwnd);
        }
        #[cfg(not(target_os = "windows"))]
        let _ = hwnd;
    }

    fn tracks(&self, sort: TrackSort, limit: usize, offset: usize) -> Result<Vec<Track>, String> {
        self.library
            .tracks_sorted(sort, musify_core::SortDirection::Ascending, limit, offset)
            .map_err(|error| error.to_string())
    }

    fn tracks_for_album(&self, album_id: AlbumId) -> Result<Vec<Track>, String> {
        self.library
            .tracks_for_album(album_id, 100_000, 0)
            .map_err(|error| error.to_string())
    }

    fn tracks_for_artist(&self, artist_id: ArtistId) -> Result<Vec<Track>, String> {
        self.library
            .tracks_for_artist(artist_id, 100_000, 0)
            .map_err(|error| error.to_string())
    }

    fn albums(&self) -> Result<Vec<Album>, String> {
        self.library
            .albums_page(10_000, 0)
            .map_err(|error| error.to_string())
    }

    fn artists(&self) -> Result<Vec<Artist>, String> {
        self.library
            .artists_page(10_000, 0)
            .map_err(|error| error.to_string())
    }

    fn search(&self, query: &str, limit: usize) -> Result<SearchResults, String> {
        self.library
            .search_all(query, limit)
            .map_err(|error| error.to_string())
    }

    fn stats(&self) -> Result<LibraryStats, String> {
        self.library.stats().map_err(|error| error.to_string())
    }

    fn folders(&self) -> Vec<PathBuf> {
        self.folders()
    }

    fn add_folder(&self, path: PathBuf) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        let mut changed = false;
        self.update_config(|config| {
            if !config
                .music_folders
                .iter()
                .any(|folder| same_path(folder, &path))
            {
                config.music_folders.push(path.clone());
                changed = true;
            }
        });
        if changed {
            self.restart_watcher();
            self.scan.start();
        }
    }

    fn remove_folder(&self, path: &Path) {
        let mut changed = false;
        self.update_config(|config| {
            let before = config.music_folders.len();
            config
                .music_folders
                .retain(|folder| !same_path(folder, path));
            changed = config.music_folders.len() != before;
        });
        if changed {
            self.restart_watcher();
            self.scan.start();
        }
    }

    fn rescan(&self) {
        self.scan.start();
    }

    fn radio_search(&self, query: RadioQuery) {
        let request_id = self.radio_request.fetch_add(1, Ordering::AcqRel) + 1;
        let browser = Arc::clone(&self.radio);
        let requests = Arc::clone(&self.radio_request);
        let events = self.events_tx.clone();
        let waker = Arc::clone(&self.waker);
        let icon_inflight = Arc::clone(&self.icon_inflight);
        let _ = thread::Builder::new()
            .name("musify-radio-search".into())
            .spawn(move || {
                let query = SearchQuery {
                    name: query.name,
                    country: query.country,
                    tag: query.tag,
                };
                let (event, icon_ids) = match browser.search(&query, 100) {
                    Ok(outcome) => {
                        let icon_ids = outcome
                            .items
                            .iter()
                            .take(8)
                            .map(|item| item.id.clone())
                            .collect();
                        (
                            Event::SourceResults {
                                source: "radio".into(),
                                request_id,
                                items: outcome.items,
                                offline: outcome.offline,
                            },
                            icon_ids,
                        )
                    }
                    Err(error) => (
                        Event::SourceSearchFailed {
                            source: "radio".into(),
                            request_id,
                            message: error.to_string(),
                        },
                        Vec::new(),
                    ),
                };
                // A newer search supersedes this one; drop the stale result.
                if requests.load(Ordering::Acquire) != request_id {
                    return;
                }
                let _ = events.send(event);
                wake(&waker);
                prefetch_icons(&browser, &events, &waker, &icon_inflight, icon_ids);
            });
    }

    fn radio_favorites(&self) -> Result<Vec<SourceItem>, String> {
        let items = self.radio.favorites().map_err(|error| error.to_string())?;
        let ids: Vec<String> = items.iter().take(8).map(|item| item.id.clone()).collect();
        let browser = Arc::clone(&self.radio);
        let events = self.events_tx.clone();
        let waker = Arc::clone(&self.waker);
        let inflight = Arc::clone(&self.icon_inflight);
        let _ = thread::Builder::new()
            .name("musify-radio-icons".into())
            .spawn(move || prefetch_icons(&browser, &events, &waker, &inflight, ids));
        Ok(items)
    }

    fn radio_toggle_favorite(&self, id: &str) -> Result<bool, String> {
        let station = self
            .radio
            .station(id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "radio station not found in the cache".to_string())?;
        self.radio
            .toggle_favorite(&station)
            .map_err(|error| error.to_string())
    }

    fn radio_play(&self, id: &str) {
        match self.radio.station(id) {
            Ok(Some(station)) => {
                let track = station.to_track();
                self.send_audio(Command::PlayTrack(Box::new(track)));
            }
            Ok(None) => {
                let _ = self.events_tx.send(Event::Error {
                    context: "radio".into(),
                    message: "radio station not found in the cache".into(),
                });
                wake(&self.waker);
            }
            Err(error) => {
                let _ = self.events_tx.send(Event::Error {
                    context: "radio".into(),
                    message: error.to_string(),
                });
                wake(&self.waker);
            }
        }
    }

    fn playback_settings(&self) -> (u16, bool, RepeatMode) {
        self.config
            .lock()
            .map_or((80, false, RepeatMode::Off), |config| {
                (config.volume, config.shuffle, config.repeat)
            })
    }

    fn output_info(&self) -> (u32, u16) {
        let Ok(audio) = self.audio.lock() else {
            return (0, 0);
        };
        audio.as_ref().map_or((0, 0), |audio| {
            let info = audio.output_info();
            (info.sample_rate, info.channels)
        })
    }

    fn shutdown(&self) {
        if let Ok(audio) = self.audio.lock()
            && let Some(audio) = audio.as_ref()
        {
            let _ = audio.send(Command::Shutdown);
        }
    }
}

#[derive(Clone)]
struct ScanCoordinator {
    library: Arc<Library>,
    config: Arc<Mutex<Config>>,
    events: Sender<Event>,
    waker: Arc<Mutex<Option<Waker>>>,
    scanning: Arc<AtomicBool>,
}

impl ScanCoordinator {
    fn start(&self) {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return;
        }
        let folders = self.config.lock().map_or_else(
            |poisoned| poisoned.into_inner().music_folders.clone(),
            |config| config.music_folders.clone(),
        );
        let library = Arc::clone(&self.library);
        let events = self.events.clone();
        let waker = Arc::clone(&self.waker);
        let scanning = Arc::clone(&self.scanning);
        let spawn = thread::Builder::new()
            .name("musify-library-scan".into())
            .spawn(move || {
                send_event(
                    &events,
                    &waker,
                    Event::ScanStarted {
                        folders: folders.len(),
                    },
                );
                let mut last_progress = Instant::now() - Duration::from_millis(250);
                let result = library.scan_with_progress(&folders, |progress| {
                    if last_progress.elapsed() >= Duration::from_millis(250)
                        || progress.scanned == progress.total
                    {
                        last_progress = Instant::now();
                        send_event(
                            &events,
                            &waker,
                            Event::ScanProgress {
                                scanned: progress.scanned,
                                total: progress.total,
                                current: progress.current,
                            },
                        );
                    }
                });
                match result {
                    Ok(report) => {
                        send_event(
                            &events,
                            &waker,
                            Event::ScanFinished {
                                added: report.added,
                                updated: report.updated,
                                removed: report.removed,
                                errors: report.errors,
                                elapsed: report.elapsed,
                            },
                        );
                        send_event(&events, &waker, Event::LibraryChanged);
                    }
                    Err(error) => {
                        send_event(
                            &events,
                            &waker,
                            Event::Error {
                                context: "scansione".into(),
                                message: error.to_string(),
                            },
                        );
                    }
                }
                scanning.store(false, Ordering::SeqCst);
            });
        if let Err(error) = spawn {
            self.scanning.store(false, Ordering::SeqCst);
            send_event(
                &self.events,
                &self.waker,
                Event::Error {
                    context: "scansione".into(),
                    message: error.to_string(),
                },
            );
        }
    }
}

fn send_event(events: &Sender<Event>, waker: &Arc<Mutex<Option<Waker>>>, event: Event) {
    let _ = events.send(event);
    wake(waker);
}

fn prefetch_icons(
    browser: &RadioBrowser,
    events: &Sender<Event>,
    waker: &Arc<Mutex<Option<Waker>>>,
    inflight: &Mutex<HashSet<String>>,
    ids: Vec<String>,
) {
    for id in ids {
        if let Ok(mut inflight) = inflight.lock()
            && !inflight.insert(id.clone())
        {
            continue;
        }
        let cached = browser.station(&id).ok().flatten().and_then(|station| {
            if browser.icon_path(&station).is_some() {
                return None;
            }
            browser.cache_icon(&station).ok().flatten()
        });
        if let Some(path) = cached {
            let _ = events.send(Event::SourceArtwork {
                source: "radio".into(),
                id: id.clone(),
                path,
            });
            wake(waker);
        }
        if let Ok(mut inflight) = inflight.lock() {
            inflight.remove(&id);
        }
    }
}

fn wake(waker: &Arc<Mutex<Option<Waker>>>) {
    let task = waker.lock().ok().and_then(|guard| guard.as_ref().cloned());
    if let Some(task) = task {
        task();
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MUSIFY_DATA_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Musify")
}

fn env_folders() -> Option<Vec<PathBuf>> {
    let value = std::env::var("MUSIFY_MUSIC_DIRS").ok()?;
    let folders: Vec<PathBuf> = value
        .split(';')
        .map(str::trim)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .collect();
    (!folders.is_empty()).then_some(folders)
}

fn load_config(path: &Path) -> Config {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Config::default();
    };
    match toml::from_str(&text) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "invalid config, using defaults");
            Config::default()
        }
    }
}

fn save_config(path: &Path, config: &Config) -> anyhow::Result<()> {
    let text = toml::to_string_pretty(config)?;
    let temporary = path.with_extension("toml.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}
