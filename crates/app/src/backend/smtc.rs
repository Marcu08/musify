use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use musify_core::{
    Command, Event, PlaybackState, QueueItem, QueueItemId, StreamStatus, Track, TrackId,
};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};

#[derive(Debug, Default)]
struct SmtcSnapshot {
    playback: PlaybackState,
    position: Duration,
    duration: Option<Duration>,
    current: Option<QueueItem>,
    metadata_track: Option<TrackId>,
    stream_title: Option<String>,
    stream_status: Option<StreamStatus>,
    last_sync: Option<Instant>,
}

/// What an incoming event requires from the media session.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SmtcUpdate {
    pub(crate) metadata: bool,
    pub(crate) sync: bool,
}

pub(crate) struct Smtc {
    controls: Mutex<Option<MediaControls>>,
    snapshot: Arc<Mutex<SmtcSnapshot>>,
}

impl Smtc {
    pub(crate) fn new() -> Self {
        Self {
            controls: Mutex::new(None),
            snapshot: Arc::new(Mutex::new(SmtcSnapshot::default())),
        }
    }

    pub(crate) fn observe(&self, event: &Event) -> SmtcUpdate {
        let mut force_metadata = false;
        let mut force_sync = false;
        if let Ok(mut snapshot) = self.snapshot.lock() {
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
        SmtcUpdate {
            metadata: force_metadata,
            sync: force_sync,
        }
    }

    pub(crate) fn update(&self, force_metadata: bool) {
        let Ok(mut controls) = self.controls.lock() else {
            return;
        };
        let Some(controls) = controls.as_mut() else {
            return;
        };
        let Ok(mut snapshot) = self.snapshot.lock() else {
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

    pub(crate) fn restore(&self, track: &Track, position: Duration) {
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.current = Some(QueueItem {
                id: QueueItemId::new(1),
                track: track.clone(),
            });
            snapshot.playback = PlaybackState::Paused;
            snapshot.position = position;
            snapshot.duration = Some(track.duration);
            snapshot.metadata_track = None;
        }
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn start(&self, hwnd: isize, command_tx: Option<Sender<Command>>) {
        if self
            .controls
            .lock()
            .is_ok_and(|controls| controls.is_some())
        {
            return;
        }
        let config = PlatformConfig {
            display_name: "Musify",
            dbus_name: "musify",
            hwnd: Some(hwnd as *mut c_void),
        };
        match MediaControls::new(config) {
            Ok(mut controls) => {
                let snapshot = Arc::clone(&self.snapshot);
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
                if let Ok(mut slot) = self.controls.lock() {
                    *slot = Some(controls);
                }
                self.update(true);
                tracing::info!("Windows SMTC collegato");
            }
            Err(error) => {
                tracing::warn!(%error, "cannot create Windows media controls");
            }
        }
    }
}
