use std::path::PathBuf;
use std::time::Duration;

use crate::{PlaybackState, QueueItem, RepeatMode};

/// Events travel from the application controller to the UI.
///
/// Events are deliberately plain data: the UI never receives a backend handle.
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
    ScanStarted {
        folders: usize,
    },
    ScanProgress {
        scanned: u64,
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
