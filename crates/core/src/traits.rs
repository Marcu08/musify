use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

use crate::{Album, Artist, SearchResults, SourceEntry, SourceItem, Track, TrackSort};

/// Search terms for an online source. `text` is the free-text query; filters
/// are source-specific keys (`country`, `tag`, ...). The core never interprets
/// the keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceQuery {
    pub text: String,
    pub filters: Vec<(String, String)>,
}

impl SourceQuery {
    #[must_use]
    pub fn filter(&self, key: &str) -> Option<&str> {
        self.filters
            .iter()
            .find(|(candidate, value)| candidate == key && !value.trim().is_empty())
            .map(|(_, value)| value.as_str())
    }

    pub fn set_filter(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some((_, current)) = self
            .filters
            .iter_mut()
            .find(|(candidate, _)| candidate == key)
        {
            *current = value;
        } else {
            self.filters.push((key.to_string(), value));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSearch {
    pub items: Vec<SourceItem>,
    pub offline: bool,
}

/// How the engine must treat a resolved source item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePlayback {
    /// Endless stream: no duration, no seek, reconnect on loss.
    Live,
    /// Finite resource: duration and whether the transport supports seeking.
    OnDemand { duration: Duration, seekable: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSource {
    pub track: Track,
    pub playback: SourcePlayback,
}

/// One online source (radio, podcast, a remote library, ...).
///
/// The provider only discovers and resolves items. Playback stays in the audio
/// engine, and the core only transports `SourceId`/`Track`.
pub trait SourceProvider: Send + Sync {
    fn scheme(&self) -> &'static str;
    fn search(&self, query: &SourceQuery) -> Result<SourceSearch, SourceError>;
    fn favorites(&self) -> Result<Vec<SourceItem>, SourceError>;
    fn toggle_favorite(&self, id: &str) -> Result<bool, SourceError>;
    fn resolve(&self, id: &str) -> Result<ResolvedSource, SourceError>;
    /// Optional cover/artwork. Called from a background thread; may download.
    fn artwork(&self, _id: &str) -> Result<Option<PathBuf>, SourceError> {
        Ok(None)
    }

    /// Entries inside a source item (for example podcast episodes).
    fn entries(&self, _id: &str) -> Result<Vec<SourceEntry>, SourceError> {
        Err(SourceError::Unsupported(
            "this source has no entry list".into(),
        ))
    }

    /// Fetch and store the latest entries. Called on a background thread only
    /// after an explicit user action; sources must not poll in the background.
    fn refresh(&self, id: &str) -> Result<Vec<SourceEntry>, SourceError> {
        self.entries(id)
    }

    /// Resume position for an entry, when the source stores it.
    fn resume_position(&self, _id: &str) -> Result<Option<Duration>, SourceError> {
        Ok(None)
    }

    /// Persist playback progress and optionally the watched flag.
    fn save_progress(
        &self,
        _id: &str,
        _position: Duration,
        _watched: bool,
    ) -> Result<(), SourceError> {
        Ok(())
    }

    fn set_watched(&self, _id: &str, _watched: bool) -> Result<(), SourceError> {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("source network error: {0}")]
    Network(String),
    #[error("source storage error: {0}")]
    Storage(String),
    #[error("source item not found: {0}")]
    NotFound(String),
    #[error("source does not support this operation: {0}")]
    Unsupported(String),
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
    fn source_errors_have_ui_ready_messages() {
        let error = SourceError::NotFound("station".into());
        assert_eq!(error.to_string(), "source item not found: station");
        let mut query = SourceQuery::default();
        query.set_filter("country", "Italy");
        assert_eq!(query.filter("country"), Some("Italy"));
    }
}
