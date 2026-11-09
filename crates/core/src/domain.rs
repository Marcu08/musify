use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{AlbumId, ArtistId, PlaylistId, QueueItemId, TrackId};

/// A playable track. `path` is the canonical absolute path on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub duration: Duration,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_path: Option<PathBuf>,
    /// Unix timestamp (seconds) of the first scan that saw this file.
    pub added_at: i64,
}

/// An album row for the album grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Album {
    pub id: AlbumId,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub cover_path: Option<PathBuf>,
    pub track_count: u32,
}

/// An artist row for the artist list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub album_count: u32,
    pub track_count: u32,
}

/// A local playlist. Tracks are stored separately in the library database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One slot in the playback queue. The id stays stable when the queue is reordered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: QueueItemId,
    pub track: Track,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrackSort {
    #[default]
    Title,
    Artist,
    Album,
    AddedAt,
    Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
}

/// Aggregate row counts shown by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LibraryStats {
    pub tracks: u64,
    pub albums: u64,
    pub artists: u64,
    pub total_duration: Duration,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn track() -> Track {
        Track {
            id: TrackId::new(1),
            path: PathBuf::from(r"C:\Music\artist\album\01 - song.flac"),
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            album_artist: Some("Album Artist".into()),
            track_number: Some(1),
            disc_number: Some(1),
            duration: Duration::from_secs(215),
            year: Some(2024),
            genre: Some("Electronic".into()),
            cover_path: None,
            added_at: 1_700_000_000,
        }
    }

    #[test]
    fn track_round_trips_through_json() {
        let original = track();
        let json = serde_json::to_string(&original).expect("serialize track");
        let decoded: Track = serde_json::from_str(&json).expect("deserialize track");
        assert_eq!(original, decoded);
    }

    #[test]
    fn queue_item_keeps_its_identity() {
        let item = QueueItem {
            id: QueueItemId::new(9),
            track: track(),
        };
        assert_eq!(item.id.get(), 9);
        assert_eq!(item.track.title, "Song");
    }
}
