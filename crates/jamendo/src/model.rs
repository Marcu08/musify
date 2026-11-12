use std::time::Duration;

use musify_core::TrackId;
use serde::{Deserialize, Serialize};

/// One track returned by the Jamendo tracks API. The API also returns
/// `audiodownload` and `audiodownload_allowed`; they are deliberately not
/// represented here because Musify only streams and never downloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JamendoTrack {
    pub id: String,
    pub name: String,
    pub artist_name: String,
    pub album_name: String,
    pub duration_secs: u64,
    pub audio_url: String,
    pub image_url: String,
    pub license_url: String,
    pub share_url: String,
}

impl JamendoTrack {
    #[must_use]
    pub fn duration(&self) -> Duration {
        Duration::from_secs(self.duration_secs)
    }

    #[must_use]
    pub fn license_label(&self) -> &'static str {
        license_label(&self.license_url)
    }

    /// Album text shown in the UI and in SMTC. It carries the license and the
    /// Jamendo attribution because Windows' media overlay has no extra field.
    #[must_use]
    pub fn display_album(&self) -> String {
        let license = self.license_label();
        let album = self.album_name.trim();
        if album.is_empty() {
            format!("{license} · Jamendo")
        } else {
            format!("{album} · {license} · Jamendo")
        }
    }

    #[must_use]
    pub fn page_url(&self) -> String {
        if self.share_url.trim().is_empty() {
            format!("https://www.jamendo.com/track/{}", self.id)
        } else {
            self.share_url.clone()
        }
    }

    #[must_use]
    pub fn track_id(&self) -> TrackId {
        track_id(&self.id)
    }

    #[must_use]
    pub fn is_playable(&self) -> bool {
        !self.id.trim().is_empty()
            && (self.audio_url.starts_with("https://") || self.audio_url.starts_with("http://"))
    }
}

/// A short, human-readable label for a Creative Commons license URL.
#[must_use]
pub fn license_label(url: &str) -> &'static str {
    let lower = url.trim().to_ascii_lowercase();
    if lower.contains("by-nc-nd") {
        "CC BY-NC-ND"
    } else if lower.contains("by-nc-sa") {
        "CC BY-NC-SA"
    } else if lower.contains("by-nc") {
        "CC BY-NC"
    } else if lower.contains("by-nd") {
        "CC BY-ND"
    } else if lower.contains("by-sa") {
        "CC BY-SA"
    } else if lower.contains("by") {
        "CC BY"
    } else if lower.is_empty() {
        "Licenza Creative Commons"
    } else {
        "Creative Commons"
    }
}

/// Stable positive track id derived from the Jamendo id. It must not collide
/// with the local library ids used by the UI's "now playing" highlight.
#[must_use]
pub fn track_id(id: &str) -> TrackId {
    let hash = blake3::hash(id.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    TrackId::new((i64::from_le_bytes(bytes) & i64::MAX).max(1))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn license_labels_cover_common_creative_commons_urls() {
        assert_eq!(
            license_label("http://creativecommons.org/licenses/by-sa/3.0/"),
            "CC BY-SA"
        );
        assert_eq!(
            license_label("http://creativecommons.org/licenses/by-nc-nd/3.0/"),
            "CC BY-NC-ND"
        );
        assert_eq!(
            license_label("http://creativecommons.org/licenses/by-nc-sa/4.0/"),
            "CC BY-NC-SA"
        );
        assert_eq!(license_label(""), "Licenza Creative Commons");
    }

    #[test]
    fn display_album_keeps_license_and_jamendo_attribution() {
        let track = JamendoTrack {
            id: "42".into(),
            name: "Song".into(),
            artist_name: "Artist".into(),
            album_name: "Album".into(),
            duration_secs: 180,
            audio_url: "https://audio.example/42.mp3".into(),
            image_url: "https://img.example/42.jpg".into(),
            license_url: "http://creativecommons.org/licenses/by-sa/3.0/".into(),
            share_url: "https://www.jamendo.com/track/42".into(),
        };
        assert_eq!(track.display_album(), "Album · CC BY-SA · Jamendo");
        assert_eq!(track.page_url(), "https://www.jamendo.com/track/42");
        assert!(track.is_playable());
        assert_eq!(track.track_id(), track_id("42"));
    }

    #[test]
    fn download_fields_are_not_part_of_the_model() {
        let track = JamendoTrack {
            id: "42".into(),
            name: "Song".into(),
            artist_name: "Artist".into(),
            album_name: String::new(),
            duration_secs: 180,
            audio_url: "https://audio.example/42.mp3".into(),
            image_url: String::new(),
            license_url: String::new(),
            share_url: String::new(),
        };
        let json = serde_json::to_string(&track).expect("serialize");
        assert!(!json.contains("audiodownload"));
        assert_eq!(track.page_url(), "https://www.jamendo.com/track/42");
        assert_eq!(track.display_album(), "Licenza Creative Commons · Jamendo");
    }
}
