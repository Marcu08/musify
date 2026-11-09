use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded};
use notify::{EventKind, RecursiveMode, Watcher};

use crate::LibraryError;

const DEBOUNCE: Duration = Duration::from_millis(750);
const POLL: Duration = Duration::from_millis(100);

/// Watches the music folders and emits one `()` on the output channel after a
/// burst of filesystem changes has been quiet for [`DEBOUNCE`].
pub struct LibraryWatcher {
    _watcher: notify::RecommendedWatcher,
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl LibraryWatcher {
    pub fn start(folders: &[PathBuf], changed: Sender<()>) -> Result<Self, LibraryError> {
        let (raw_tx, raw_rx) = bounded(1024);
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let Ok(event) = result else {
                    return;
                };
                if !matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    return;
                }
                if event.paths.iter().any(|path| is_audio_file(path)) {
                    let _ = raw_tx.try_send(());
                }
            })
            .map_err(|error| LibraryError::Watcher(error.to_string()))?;

        for folder in folders {
            watcher
                .watch(folder, RecursiveMode::Recursive)
                .map_err(|error| LibraryError::Watcher(error.to_string()))?;
        }

        let (stop_tx, stop_rx) = bounded(1);
        let thread = thread::Builder::new()
            .name("musify-library-watcher".into())
            .spawn(move || debounce(raw_rx, stop_rx, changed))
            .map_err(|error| LibraryError::Watcher(error.to_string()))?;

        Ok(Self {
            _watcher: watcher,
            stop: stop_tx,
            thread: Some(thread),
        })
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn debounce(raw: Receiver<()>, stop: Receiver<()>, changed: Sender<()>) {
    let mut pending = false;
    let mut last_event = Instant::now();
    loop {
        if stop.try_recv().is_ok() {
            break;
        }
        match raw.recv_timeout(POLL) {
            Ok(()) => {
                pending = true;
                last_event = Instant::now();
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if pending && last_event.elapsed() >= DEBOUNCE {
                    pending = false;
                    if changed.send(()).is_err() {
                        break;
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_lowercase().as_str(),
                "aac" | "flac" | "m4a" | "mp3" | "mp4" | "oga" | "ogg" | "wav"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn recognises_audio_extensions() {
        assert!(is_audio_file(Path::new("track.MP3")));
        assert!(is_audio_file(Path::new("track.flac")));
        assert!(!is_audio_file(Path::new("cover.jpg")));
    }

    #[test]
    fn watcher_emits_after_a_file_change() {
        let dir = tempfile::tempdir().expect("temp");
        let music = dir.path().join("music");
        std::fs::create_dir_all(&music).expect("music dir");
        let (tx, rx) = crossbeam_channel::bounded(4);
        let _watcher =
            LibraryWatcher::start(std::slice::from_ref(&music), tx).expect("start watcher");
        std::fs::write(music.join("new-track.mp3"), b"not really audio").expect("write file");
        let event = rx.recv_timeout(std::time::Duration::from_secs(5));
        assert!(event.is_ok(), "watcher did not emit a change");
    }
}
