//! Spotify playback source (M5, optional).
//!
//! This crate is compiled only when the `spotify` feature is enabled. It will never
//! store credentials in the repository; tokens belong in the Windows Credential Manager.

/// Crate marker used by the M0 wiring check.
pub const CRATE_NAME: &str = "musify-spotify";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired() {
        assert_eq!(CRATE_NAME, "musify-spotify");
    }
}
