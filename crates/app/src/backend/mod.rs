mod persistence;
mod queue;
mod smtc;
mod sources;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use musify_audio::AudioEngine;
use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, PlaybackState, RepeatMode,
    SearchResults, SourceEntry, SourceId, SourceItem, SourcePlayback, SourceQuery, Track, TrackId,
    TrackSort,
};
use musify_library::{Library, LibraryWatcher};
use musify_podcast::{PodcastSource, PodcastStore};
use musify_radio::{RadioBrowser, RadioStore};
use musify_ui::Backend;

use persistence::{Config, app_data_dir, env_folders, load_config, same_path, save_config};
use queue::QueueStore;
use smtc::Smtc;
use sources::SourceRegistry;

type Waker = Arc<dyn Fn() + Send + Sync>;

pub struct AppBackend {
    data_dir: PathBuf,
    config_path: PathBuf,
    config: Arc<Mutex<Config>>,
    library: Arc<Library>,
    sources: SourceRegistry,
    audio: Mutex<Option<AudioEngine>>,
    events: Mutex<Receiver<Event>>,
    events_tx: Sender<Event>,
    waker: Arc<Mutex<Option<Waker>>>,
    scan: ScanCoordinator,
    watcher: Mutex<Option<LibraryWatcher>>,
    smtc: Smtc,
    queue: QueueStore,
    current_source: Mutex<Option<CurrentSource>>,
}

#[derive(Debug, Clone)]
struct CurrentSource {
    source: String,
    entry_id: String,
    track_uri: String,
    duration: Option<Duration>,
    position: Duration,
    watched: bool,
    last_save: Instant,
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
        let podcast_store = Arc::new(PodcastStore::open(&data_dir.join("podcasts.sqlite3"))?);
        let podcast = Arc::new(PodcastSource::new(
            podcast_store,
            data_dir.join("podcast-covers"),
        ));
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
        let mut sources = SourceRegistry::new(event_tx.clone(), Arc::clone(&waker));
        sources.register(radio);
        sources.register(podcast);

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
            config,
            library,
            sources,
            audio: Mutex::new(audio),
            events: Mutex::new(event_rx),
            events_tx: event_tx,
            waker,
            scan,
            watcher: Mutex::new(None),
            smtc: Smtc::new(),
            queue: QueueStore::new(state_path),
            current_source: Mutex::new(None),
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
        if let Ok(url) = std::env::var("MUSIFY_AUTOPLAY_PODCAST")
            && !url.trim().is_empty()
        {
            backend.send_audio(Command::PlayTrack(Box::new(Track {
                id: TrackId::new(1),
                source: SourceId::podcast(url),
                path: PathBuf::new(),
                title: "Podcast test".into(),
                artist: "Podcast".into(),
                album: "Podcast".into(),
                album_artist: None,
                track_number: None,
                disc_number: None,
                duration: Duration::ZERO,
                year: None,
                genre: None,
                cover_path: None,
                added_at: 0,
            })));
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
        if let Some(command) = self.queue.restore(&self.library, &self.smtc) {
            self.send_audio(command);
        }
    }

    fn observe_event(&self, event: &Event) {
        self.track_source_progress(event);
        let update = self.smtc.observe(event);
        if matches!(
            event,
            Event::TrackChanged(_)
                | Event::QueueChanged(_)
                | Event::PlaybackStateChanged(_)
                | Event::PositionChanged { .. }
        ) {
            self.queue.save(&self.audio, false);
        }
        if update.sync || matches!(event, Event::PositionChanged { .. }) {
            self.smtc.update(update.metadata);
        }
    }

    fn track_source_progress(&self, event: &Event) {
        match event {
            Event::TrackChanged(Some(item)) => {
                let mut slot = self
                    .current_source
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if slot
                    .as_ref()
                    .is_some_and(|current| current.track_uri != item.track.source.uri())
                    && let Some(current) = slot.take()
                {
                    drop(slot);
                    self.save_source_progress(&current);
                }
            }
            Event::TrackChanged(None) => {
                let current = self
                    .current_source
                    .lock()
                    .ok()
                    .and_then(|mut slot| slot.take());
                if let Some(current) = current {
                    self.save_source_progress(&current);
                }
            }
            Event::PositionChanged { position, duration } => {
                let pending = {
                    let mut slot = self
                        .current_source
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let Some(current) = slot.as_mut() else {
                        return;
                    };
                    current.position = *position;
                    if duration.is_some() {
                        current.duration = *duration;
                    }
                    current.watched |= current.duration.is_some_and(|total| {
                        !total.is_zero() && *position + Duration::from_secs(5) >= total
                    });
                    if current.watched || current.last_save.elapsed() >= Duration::from_secs(2) {
                        current.last_save = Instant::now();
                        Some(current.clone())
                    } else {
                        None
                    }
                };
                if let Some(current) = pending {
                    self.save_source_progress(&current);
                }
            }
            Event::PlaybackStateChanged(PlaybackState::Paused | PlaybackState::Stopped) => {
                let current = self
                    .current_source
                    .lock()
                    .ok()
                    .and_then(|slot| slot.clone());
                if let Some(current) = current {
                    self.save_source_progress(&current);
                }
            }
            _ => {}
        }
    }

    fn save_source_progress(&self, current: &CurrentSource) {
        if let Err(error) = self.sources.save_progress(
            &current.source,
            &current.entry_id,
            current.position,
            current.watched,
        ) {
            tracing::warn!(
                %error,
                source = %current.source,
                entry = %current.entry_id,
                "cannot save source progress"
            );
        }
    }
}

impl Drop for AppBackend {
    fn drop(&mut self) {
        let current = self
            .current_source
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        if let Some(current) = current {
            self.save_source_progress(&current);
        }
        self.queue.save(&self.audio, true);
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
            let command_tx = self
                .audio
                .lock()
                .ok()
                .and_then(|audio| audio.as_ref().map(AudioEngine::command_sender));
            self.smtc.start(hwnd, command_tx);
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

    fn source_search(&self, source: &str, query: SourceQuery) {
        self.sources.search(source, query);
    }

    fn source_favorites(&self, source: &str) -> Result<Vec<SourceItem>, String> {
        self.sources.favorites(source)
    }

    fn source_toggle_favorite(&self, source: &str, id: &str) -> Result<bool, String> {
        self.sources.toggle_favorite(source, id)
    }

    fn source_play(&self, source: &str, id: &str) {
        let resolved = match self.sources.resolve(source, id) {
            Ok(resolved) => resolved,
            Err(message) => {
                let _ = self.events_tx.send(Event::Error {
                    context: source.to_string(),
                    message,
                });
                wake(&self.waker);
                return;
            }
        };
        let resume = match resolved.playback {
            SourcePlayback::OnDemand { .. } => {
                self.sources.resume_position(source, id).ok().flatten()
            }
            SourcePlayback::Live => None,
        };
        let previous = self
            .current_source
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        if let Some(previous) = previous {
            self.save_source_progress(&previous);
        }
        if let SourcePlayback::OnDemand { duration, .. } = resolved.playback
            && let Ok(mut slot) = self.current_source.lock()
        {
            *slot = Some(CurrentSource {
                source: source.to_string(),
                entry_id: id.to_string(),
                track_uri: resolved.track.source.uri().to_string(),
                duration: Some(duration),
                position: resume.unwrap_or_default(),
                watched: false,
                last_save: Instant::now() - Duration::from_secs(10),
            });
        }
        tracing::debug!(
            source,
            playback = ?resolved.playback,
            "source item resolved"
        );
        if let Some(position) = resume.filter(|position| !position.is_zero()) {
            self.send_audio(Command::PlayTrackAt(Box::new(resolved.track), position));
        } else {
            self.send_audio(Command::PlayTrack(Box::new(resolved.track)));
        }
    }

    fn source_entries(&self, source: &str, id: &str) -> Result<Vec<SourceEntry>, String> {
        self.sources.entries(source, id)
    }

    fn source_refresh(&self, source: &str, id: &str) {
        self.sources.refresh(source, id);
    }

    fn source_set_watched(&self, source: &str, id: &str, watched: bool) -> Result<(), String> {
        self.sources.set_watched(source, id, watched)
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

fn wake(waker: &Arc<Mutex<Option<Waker>>>) {
    let task = waker.lock().ok().and_then(|guard| guard.as_ref().cloned());
    if let Some(task) = task {
        task();
    }
}
