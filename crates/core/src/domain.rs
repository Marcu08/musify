use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{AlbumId, ArtistId, PlaylistId, PlaylistItemId, QueueItemId, TrackId};

/// Identifies where a queue item comes from.
///
/// `local` items are files opened by path; any other scheme is an opaque URI
/// that a source can open (for example `radio://https://stream.example/mp3`).
/// The core only carries the identifier: it never interprets the URI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId {
    scheme: String,
    uri: String,
}

impl SourceId {
    #[must_use]
    pub fn new(scheme: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            uri: uri.into(),
        }
    }

    #[must_use]
    pub fn local(path: impl AsRef<Path>) -> Self {
        Self::new("local", path.as_ref().to_string_lossy())
    }

    #[must_use]
    pub fn radio(url: impl Into<String>) -> Self {
        Self::new("radio", url)
    }

    #[must_use]
    pub fn podcast(url: impl Into<String>) -> Self {
        Self::new("podcast", url)
    }

    #[must_use]
    pub fn jamendo(url: impl Into<String>) -> Self {
        Self::new("jamendo", url)
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    #[must_use]
    pub fn is_local(&self) -> bool {
        self.scheme == "local"
    }
}

impl Default for SourceId {
    /// Local with an empty URI, so an old serialized track without a source
    /// still deserializes. New local tracks always set the real path.
    fn default() -> Self {
        Self::new("local", "")
    }
}

/// A playable track. `path` is the canonical absolute path on disk for local
/// items; streamed items keep their URL in `source.uri()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    #[serde(default)]
    pub source: SourceId,
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

impl Track {
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.source.is_local()
    }

    /// Finite HTTP resources can seek and must not reconnect as live streams.
    #[must_use]
    pub fn is_on_demand(&self) -> bool {
        matches!(self.source.scheme(), "podcast" | "jamendo")
    }
}

/// A result from an online source (radio, a remote library, ...) shown by the
/// UI before it becomes a queue item. The core does not interpret `uri`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceItem {
    pub id: String,
    pub source: SourceId,
    pub title: String,
    pub subtitle: String,
    pub detail: String,
    pub uri: String,
    pub favorite: bool,
    pub image_path: Option<PathBuf>,
    #[serde(default)]
    pub duration: Option<Duration>,
    #[serde(default)]
    pub license_url: Option<String>,
}

/// One entry (for example a podcast episode) inside a source item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEntry {
    pub id: String,
    pub source: SourceId,
    pub title: String,
    pub subtitle: String,
    pub detail: String,
    pub uri: String,
    pub duration: Option<Duration>,
    /// Unix timestamp (seconds) of publication, when known.
    pub published_at: Option<i64>,
    pub watched: bool,
    /// Saved resume position for this entry.
    pub position: Option<Duration>,
    pub image_path: Option<PathBuf>,
}

impl SourceEntry {
    #[must_use]
    pub fn progress(&self) -> Option<Duration> {
        self.position.filter(|position| !position.is_zero())
    }
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
    pub item_count: u32,
}

/// One source-agnostic entry in a playlist. The snapshot keeps the entry
/// visible when the original source is unavailable; `available` is computed
/// when the list is read and is not persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: PlaylistItemId,
    pub playlist_id: PlaylistId,
    pub position: u32,
    pub source: SourceId,
    pub item_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Duration,
    pub cover_path: Option<PathBuf>,
    pub available: bool,
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
            source: SourceId::local(r"C:\Music\artist\album\01 - song.flac"),
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
    fn source_ids_distinguish_local_files_from_streams() {
        let local = SourceId::local(r"C:\Music\song.flac");
        assert!(local.is_local());
        assert_eq!(local.uri(), r"C:\Music\song.flac");
        let radio = SourceId::radio("https://stream.example/live.mp3");
        assert!(!radio.is_local());
        assert_eq!(radio.scheme(), "radio");
        assert_eq!(radio.uri(), "https://stream.example/live.mp3");
    }

    #[test]
    fn jamendo_tracks_are_on_demand_streams() {
        let mut item = track();
        item.source = SourceId::jamendo("https://audio.example/track.mp3");
        assert!(!item.is_local());
        assert!(item.is_on_demand());
        assert_eq!(item.source.scheme(), "jamendo");
        assert_eq!(item.source.uri(), "https://audio.example/track.mp3");
    }

    #[test]
    fn old_track_json_without_a_source_is_local() {
        let json = r#"{
            "id": 1,
            "path": "C:\\Music\\song.flac",
            "title": "Song",
            "artist": "Artist",
            "album": "Album",
            "album_artist": null,
            "track_number": null,
            "disc_number": null,
            "duration": {"secs": 1, "nanos": 0},
            "year": null,
            "genre": null,
            "cover_path": null,
            "added_at": 0
        }"#;
        let track: Track = serde_json::from_str(json).expect("deserialize old track");
        assert!(track.is_local());
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
