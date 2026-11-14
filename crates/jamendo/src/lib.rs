//! Jamendo source: Creative Commons search, streaming-only playback, local
//! result cache and favorites. The API client id comes only from
//! `MUSIFY_JAMENDO_CLIENT_ID` and is never stored or logged.

mod client;
mod error;
mod model;
mod store;

pub use client::{JamendoOptions, JamendoSource};
pub use error::JamendoError;
pub use model::{JamendoSelection, JamendoTrack, license_label};
pub use store::JamendoStore;
