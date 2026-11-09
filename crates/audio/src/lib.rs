//! Playback engine (M1).
//!
//! The public surface is added milestone by milestone. The module is intentionally
//! independent from Rune and from any windowing code.

/// Crate marker used by the M0 wiring check.
pub const CRATE_NAME: &str = "musify-audio";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired() {
        assert_eq!(CRATE_NAME, "musify-audio");
    }
}
