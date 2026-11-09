use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use musify_audio::AudioEngine;
use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, RepeatMode, SearchResults,
    Track, TrackSort,
};
use musify_library::{Library, LibraryWatcher};
use musify_ui::Backend;
use serde::{Deserialize, Serialize};

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
    config: Arc<Mutex<Config>>,
    library: Arc<Library>,
    audio: Mutex<Option<AudioEngine>>,
    events: Mutex<Receiver<Event>>,
    waker: Arc<Mutex<Option<Waker>>>,
    scan: ScanCoordinator,
    watcher: Mutex<Option<LibraryWatcher>>,
}

impl AppBackend {
    pub fn new() -> anyhow::Result<Self> {
        let data_dir = app_data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let config_path = data_dir.join("config.toml");
        let mut config = load_config(&config_path);
        if let Some(folders) = env_folders() {
            config.music_folders = folders;
        }
        let library = Arc::new(Library::open(
            data_dir.join("library.sqlite3"),
            data_dir.join("covers"),
        )?);
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
            config,
            library,
            audio: Mutex::new(audio),
            events: Mutex::new(event_rx),
            waker,
            scan,
            watcher: Mutex::new(None),
        };
        backend.restart_watcher();
        backend.scan.start();
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
        if let Ok(audio) = self.audio.lock()
            && let Some(audio) = audio.as_ref()
            && let Err(error) = audio.send(command)
        {
            tracing::warn!(%error, "audio command failed");
        }
    }

    fn try_recv_event(&self) -> Option<Event> {
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(poisoned) => poisoned.into_inner(),
        };
        events.try_recv().ok()
    }

    fn set_waker(&self, waker: Waker) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(waker);
        }
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
