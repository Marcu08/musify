//! Musify audio engine.
//!
//! Decoding happens on a dedicated worker thread. Decoded audio is converted to
//! interleaved stereo f32 at the output device rate and handed to a wait-free
//! ring buffer; the real-time callback only pops samples and applies gain.
//!
//! The engine has no dependency on the UI, Rune or the library database.

mod clock;
mod decoder;
mod engine;
mod error;
mod output;
mod queue;
mod resample;

pub use engine::{AudioEngine, PlaybackSnapshot};
pub use error::AudioError;
pub use output::OutputInfo;

/// Playback position updates while a track is playing.
pub const POSITION_EVENT_INTERVAL_MS: u64 = 200;
