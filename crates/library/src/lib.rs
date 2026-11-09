//! Local music library: incremental scanner, SQLite index, metadata, covers and
//! search. The crate is UI-independent and implements `musify_core::LibraryProvider`.

mod cover;
mod db;
mod scanner;
mod watcher;

use std::path::PathBuf;
use std::time::Duration;

pub use db::Library;
pub use musify_core::LibraryStats;
pub use watcher::LibraryWatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanReport {
    pub added: u64,
    pub updated: u64,
    pub removed: u64,
    pub errors: u64,
    pub elapsed: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProgress {
    pub scanned: u64,
    pub total: u64,
    pub current: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("library database error: {0}")]
    Database(String),
    #[error("library filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cover error: {0}")]
    Cover(String),
    #[error("invalid library path: {0}")]
    Path(String),
    #[error("filesystem watcher error: {0}")]
    Watcher(String),
}

impl From<rusqlite::Error> for LibraryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

// `ImageError` and lofty parse errors are handled per file; they do not abort a scan.
