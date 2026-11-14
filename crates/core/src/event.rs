use std::path::PathBuf;
use std::time::Duration;

use crate::{PlaybackState, QueueItem, RepeatMode};

/// Connection state of the current online stream. It is separate from
/// `PlaybackState`: the player can be "playing" while a stream reconnects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamStatus {
    Idle,
    Connecting,
    Buffering,
    Playing,
    Reconnecting { attempt: u32 },
    Offline,
    Error(String),
}

/// ICY metadata and connection state updates for the current stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Status(StreamStatus),
    Title(Option<String>),
}

/// Events travel from the application controller to the UI.
///
/// Events are deliberately plain data: the UI never receives a backend handle.
/// `TrackChanged` carries the whole queue item; boxing it would only move the
/// allocation to every producer for a channel that is not hot.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    PlaybackStateChanged(PlaybackState),
    TrackChanged(Option<QueueItem>),
    PositionChanged {
        position: Duration,
        duration: Option<Duration>,
    },
    QueueChanged(Vec<QueueItem>),
    VolumeChanged(u16),
    ShuffleChanged(bool),
    RepeatChanged(RepeatMode),
    StreamStatusChanged(StreamStatus),
    StreamTitleChanged(Option<String>),
    SeekableChanged(bool),
    SourceResults {
        source: String,
        request_id: u64,
        items: Vec<crate::SourceItem>,
        offline: bool,
        cached: bool,
        has_more: bool,
    },
    SourceSearchFailed {
        source: String,
        request_id: u64,
        message: String,
    },
    SourceSelections {
        source: String,
        request_id: u64,
        items: Vec<crate::SourceItem>,
    },
    SourceArtwork {
        source: String,
        id: String,
        path: std::path::PathBuf,
    },
    SourceEntries {
        source: String,
        id: String,
        result: Result<Vec<crate::SourceEntry>, String>,
    },
    ScanStarted {
        folders: usize,
    },
    ScanProgress {
        scanned: u64,
        total: u64,
        current: Option<PathBuf>,
    },
    ScanFinished {
        added: u64,
        updated: u64,
        removed: u64,
        errors: u64,
        elapsed: Duration,
    },
    LibraryChanged,
    Error {
        context: String,
        message: String,
    },
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_events_keep_the_ui_facing_message() {
        let event = Event::Error {
            context: "scan".into(),
            message: "permission denied".into(),
        };
        assert!(matches!(event, Event::Error { .. }));
    }

    #[test]
    fn position_events_carry_a_duration_when_known() {
        let event = Event::PositionChanged {
            position: Duration::from_millis(1_500),
            duration: Some(Duration::from_secs(10)),
        };
        assert_eq!(
            event,
            Event::PositionChanged {
                position: Duration::from_millis(1_500),
                duration: Some(Duration::from_secs(10)),
            }
        );
    }
}
