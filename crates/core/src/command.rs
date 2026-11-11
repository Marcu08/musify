use std::path::PathBuf;
use std::time::Duration;

use crate::{RepeatMode, Track};

/// Commands travel from the UI to the application controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Replace the queue with this track and start playing it.
    PlayTrack(Box<Track>),
    /// Play one finite source item, resuming at `position`.
    PlayTrackAt(Box<Track>, Duration),
    /// Replace the queue with these tracks and start at `start_index`.
    PlayTracks {
        tracks: Vec<Track>,
        start_index: usize,
    },
    /// Restore a saved queue without starting playback, paused at `position`.
    RestoreQueue {
        tracks: Vec<Track>,
        start_index: usize,
        position: Duration,
    },
    Enqueue(Vec<Track>),
    Pause,
    Resume,
    TogglePlayPause,
    Stop,
    Next,
    Previous,
    Seek(Duration),
    SetVolume(u16),
    SetShuffle(bool),
    SetRepeat(RepeatMode),
    MoveQueueItem {
        from: usize,
        to: usize,
    },
    RemoveQueueItem {
        index: usize,
    },
    ClearQueue,
    SetMusicFolders(Vec<PathBuf>),
    ScanLibrary,
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{QueueItemId, TrackId};

    #[test]
    fn commands_are_comparable_for_tests_and_routing() {
        assert_eq!(Command::Pause, Command::Pause);
        assert_eq!(Command::SetVolume(70), Command::SetVolume(70));
        assert_ne!(Command::SetVolume(70), Command::SetVolume(60));

        let track = crate::Track {
            id: TrackId::new(1),
            source: crate::SourceId::local("song.mp3"),
            path: PathBuf::from("song.mp3"),
            title: "Song".into(),
            artist: String::new(),
            album: String::new(),
            album_artist: None,
            track_number: None,
            disc_number: None,
            duration: Duration::ZERO,
            year: None,
            genre: None,
            cover_path: None,
            added_at: 0,
        };
        assert_eq!(
            Command::Enqueue(vec![track]),
            Command::Enqueue(vec![crate::Track {
                id: TrackId::new(1),
                source: crate::SourceId::local("song.mp3"),
                path: PathBuf::from("song.mp3"),
                title: "Song".into(),
                artist: String::new(),
                album: String::new(),
                album_artist: None,
                track_number: None,
                disc_number: None,
                duration: Duration::ZERO,
                year: None,
                genre: None,
                cover_path: None,
                added_at: 0,
            }])
        );
        assert_eq!(QueueItemId::new(1).get(), 1);
    }
}
