//! Podcast discovery (iTunes Search), RSS parsing and local subscriptions.
//!
//! Network calls happen only on explicit user actions. The UI sees
//! `musify_core::SourceItem`/`SourceEntry`; no reqwest or SQLite type crosses
//! the crate boundary.

mod client;
mod error;
mod feed;
mod store;

pub use client::PodcastSource;
pub use error::PodcastError;
pub use store::PodcastStore;
