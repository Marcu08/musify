//! Domain types, commands and events shared by every Musify crate.
//!
//! This crate has no dependency on the UI, the audio backend or the database.

mod command;
mod domain;
mod event;
mod ids;
mod traits;

pub use command::Command;
pub use domain::{
    Album, Artist, LibraryStats, PlaybackState, Playlist, QueueItem, RepeatMode, SearchResults,
    SortDirection, Track, TrackSort,
};
pub use event::Event;
pub use ids::{AlbumId, ArtistId, PlaylistId, QueueItemId, TrackId};
pub use traits::{LibraryError, LibraryProvider, PlaybackError, PlaybackSource};
