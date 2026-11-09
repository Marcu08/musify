use std::time::Duration;

use thiserror::Error;

use crate::{Album, Artist, PlaybackState, SearchResults, Track, TrackSort};

/// A source that can play one track at a time: the local audio engine or Spotify.
///
/// The engine owns the queue; a source only knows how to render the current track.
pub trait PlaybackSource: Send {
    fn play(&mut self, track: &Track) -> Result<(), PlaybackError>;
    fn pause(&mut self) -> Result<(), PlaybackError>;
    fn resume(&mut self) -> Result<(), PlaybackError>;
    fn stop(&mut self) -> Result<(), PlaybackError>;
    fn seek(&mut self, position: Duration) -> Result<(), PlaybackError>;
    /// Volume is expressed as 0..=100 so it survives the command channel unchanged.
    fn set_volume(&mut self, volume: u16) -> Result<(), PlaybackError>;
    fn state(&self) -> PlaybackState;
    fn position(&self) -> Duration;
    fn duration(&self) -> Option<Duration>;
}

/// Read-only queries the UI can make against the local library.
pub trait LibraryProvider: Send + Sync {
    fn tracks(
        &self,
        sort: TrackSort,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Track>, LibraryError>;
    fn albums(&self, limit: usize, offset: usize) -> Result<Vec<Album>, LibraryError>;
    fn artists(&self, limit: usize, offset: usize) -> Result<Vec<Artist>, LibraryError>;
    fn search(&self, query: &str, limit: usize) -> Result<SearchResults, LibraryError>;
}

#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error("the audio source is not available")]
    Unavailable,
    #[error("the requested seek position is outside the track")]
    InvalidSeek,
    #[error("decode error: {0}")]
    Decode(String),
    #[error("audio output error: {0}")]
    Output(String),
    #[error("the source does not support this operation: {0}")]
    Unsupported(&'static str),
}

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("library database error: {0}")]
    Database(String),
    #[error("library filesystem error: {0}")]
    Io(String),
    #[error("invalid library query: {0}")]
    Query(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_errors_have_ui_ready_messages() {
        let error = PlaybackError::InvalidSeek;
        assert_eq!(
            error.to_string(),
            "the requested seek position is outside the track"
        );
    }
}
