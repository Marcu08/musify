use std::path::PathBuf;
use std::time::Duration;

use musify_core::{SourceId, SourceItem, Track, TrackId};
use serde::{Deserialize, Serialize};

/// One station from the Radio Browser JSON API.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    #[serde(default, rename = "stationuuid")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub url_resolved: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub favicon: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub country: String,
    #[serde(default, rename = "countrycode")]
    pub country_code: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub bitrate: u32,
    #[serde(default)]
    pub votes: i64,
    #[serde(default, rename = "lastcheckok")]
    pub last_check_ok: u8,
}

impl Station {
    pub fn stream_url(&self) -> &str {
        if self.url_resolved.is_empty() {
            &self.url
        } else {
            &self.url_resolved
        }
    }

    pub fn display_name(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            "Radio".to_string()
        } else {
            name.to_string()
        }
    }

    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if !self.country.is_empty() {
            parts.push(self.country.clone());
        } else if !self.country_code.is_empty() {
            parts.push(self.country_code.clone());
        }
        if !self.codec.is_empty() {
            parts.push(if self.bitrate > 0 {
                format!("{} {} kbps", self.codec, self.bitrate)
            } else {
                self.codec.clone()
            });
        }
        if parts.is_empty() {
            "Radio internet".to_string()
        } else {
            parts.join(" · ")
        }
    }

    pub fn to_track(&self) -> Track {
        Track {
            id: TrackId::new(stable_id(&self.id)),
            source: SourceId::radio(self.stream_url()),
            path: PathBuf::new(),
            title: self.display_name(),
            artist: self.display_name(),
            album: "Radio".to_string(),
            album_artist: None,
            track_number: None,
            disc_number: None,
            duration: Duration::ZERO,
            year: None,
            genre: (!self.tags.is_empty()).then(|| self.tags.clone()),
            cover_path: None,
            added_at: 0,
        }
    }

    pub fn to_source_item(&self, favorite: bool) -> SourceItem {
        SourceItem {
            id: self.id.clone(),
            source: SourceId::radio(self.stream_url()),
            title: self.display_name(),
            subtitle: self.subtitle(),
            detail: self.tags.clone(),
            uri: self.stream_url().to_string(),
            favorite,
            image_path: None,
            duration: None,
            license_url: None,
        }
    }
}

/// Stable positive id for a UUID, used before the station enters the queue.
fn stable_id(value: &str) -> i64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    i64::try_from(hash & 0x7fff_ffff_ffff_ffff)
        .unwrap_or(1)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn station_maps_to_a_radio_track() {
        let station = Station {
            id: "abc".into(),
            name: "Jazz FM".into(),
            url_resolved: "https://stream.example/jazz".into(),
            country: "Italy".into(),
            tags: "jazz,smooth".into(),
            codec: "MP3".into(),
            bitrate: 128,
            ..Station::default()
        };
        let track = station.to_track();
        assert_eq!(track.title, "Jazz FM");
        assert_eq!(track.source.scheme(), "radio");
        assert_eq!(track.source.uri(), "https://stream.example/jazz");
        assert!(track.duration.is_zero());
        assert_eq!(stable_id("abc"), stable_id("abc"));
        assert_ne!(stable_id("abc"), stable_id("def"));
    }
}
