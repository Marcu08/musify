//! Rune presentation layer (M3).
//!
//! Until the Rune smoke test passes this crate stays free of UI dependencies so the
//! rest of the workspace can be built on any machine.

/// Crate marker used by the M0 wiring check.
pub const CRATE_NAME: &str = "musify-ui";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired() {
        assert_eq!(CRATE_NAME, "musify-ui");
    }
}
