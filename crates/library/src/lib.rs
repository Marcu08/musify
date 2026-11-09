//! Local music library (M2): scanner, SQLite index, search and cover cache.

/// Crate marker used by the M0 wiring check.
pub const CRATE_NAME: &str = "musify-library";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired() {
        assert_eq!(CRATE_NAME, "musify-library");
    }
}
