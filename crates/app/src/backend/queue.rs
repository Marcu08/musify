use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use musify_audio::AudioEngine;
use musify_core::Command;
use musify_library::Library;
use serde::{Deserialize, Serialize};

use super::smtc::Smtc;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedState {
    queue: Vec<String>,
    current_index: usize,
    position_ms: u64,
}

pub(crate) struct QueueStore {
    path: PathBuf,
    last_save: Mutex<Instant>,
}

impl QueueStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_save: Mutex::new(Instant::now() - Duration::from_secs(10)),
        }
    }

    /// Rebuilds the saved local queue. Returns the restore command the audio
    /// engine must receive; the track is opened but stays paused.
    pub(crate) fn restore(&self, library: &Library, smtc: &Smtc) -> Option<Command> {
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return None;
        };
        let saved: SavedState = match serde_json::from_str(&text) {
            Ok(saved) => saved,
            Err(error) => {
                tracing::warn!(%error, "cannot parse state.json");
                return None;
            }
        };
        let mut tracks = Vec::new();
        for path in &saved.queue {
            match library.track_by_path(Path::new(path)) {
                Ok(Some(track)) => tracks.push(track),
                Ok(None) => tracing::debug!(path, "saved queue track is no longer in the library"),
                Err(error) => tracing::warn!(%error, path, "cannot load saved queue track"),
            }
        }
        if tracks.is_empty() {
            return None;
        }
        let start = saved.current_index.min(tracks.len() - 1);
        let position = Duration::from_millis(saved.position_ms);
        smtc.restore(&tracks[start], position);
        Some(Command::RestoreQueue {
            tracks,
            start_index: start,
            position,
        })
    }

    pub(crate) fn save(&self, audio: &Mutex<Option<AudioEngine>>, force: bool) {
        if !force {
            let Ok(last) = self.last_save.lock() else {
                return;
            };
            if last.elapsed() < Duration::from_secs(2) {
                return;
            }
        }
        let Ok(audio) = audio.lock() else {
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
            let temporary = self.path.with_extension("json.tmp");
            std::fs::write(&temporary, json)?;
            std::fs::rename(temporary, &self.path)?;
            Ok(())
        };
        if let Err(error) = write() {
            tracing::warn!(%error, "cannot save state.json");
        } else if let Ok(mut last) = self.last_save.lock() {
            *last = Instant::now();
        }
    }
}
