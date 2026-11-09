//! Radio Browser client and local cache.
//!
//! The core does not depend on this crate: it only sees `SourceId`, `SourceItem`
//! and events. Tokio is created per request and dropped immediately, so an idle
//! app has no network runtime.

mod client;
mod error;
mod station;
mod store;

pub use client::{RadioBrowser, SearchOutcome, SearchQuery};
pub use error::RadioError;
pub use station::Station;
pub use store::RadioStore;
