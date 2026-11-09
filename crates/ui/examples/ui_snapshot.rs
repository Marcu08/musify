//! Repro headless snapshot of the Rune UI without the real audio/library backend.
//!
//! ```sh
//! cargo run -p musify-ui --example ui_snapshot -- --snap ui-demo.png --scene demo
//! cargo run -p musify-ui --example ui_snapshot -- --snap ui-queue.png --scene queue
//! cargo run -p musify-ui --example ui_snapshot -- --snap ui-settings.png --scene settings
//! cargo run -p musify-ui --example ui_snapshot -- --snap ui-search.png --scene search
//! ```

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, RepeatMode, SearchResults,
    Track, TrackId, TrackSort,
};
use musify_ui::Backend;

struct MockBackend {
    tracks: Vec<Track>,
    albums: Vec<Album>,
    artists: Vec<Artist>,
    waker: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl MockBackend {
    fn new() -> Self {
        let names = [
            "Aurora Boreale",
            "Città di notte",
            "Vetro e sale",
            "Notturno elettrico",
            "Una stagione",
            "Marea",
            "Senza gravità",
            "Radio silenzio",
            "Fuori orario",
            "Luce blu",
            "Il rumore del mare",
            "Distanze",
        ];
        let artists = [
            "Lucia Ferri",
            "Nova",
            "Marta & i Vetri",
            "Teatro Notturno",
            "Sole Alto",
            "Kanso",
        ];
        let albums = ["Album uno", "Album due", "Album tre", "Album quattro"];
        let mut tracks = Vec::new();
        for index in 0..48 {
            let artist = artists[index % artists.len()];
            let album = albums[index % albums.len()];
            let title = names[index % names.len()];
            tracks.push(Track {
                id: TrackId::new(index as i64 + 1),
                path: PathBuf::from(format!("C:\\Music\\track-{index}.flac")),
                title: format!("{title} {}", index + 1),
                artist: artist.into(),
                album: album.into(),
                album_artist: Some(artist.into()),
                track_number: Some((index % 12 + 1) as u32),
                disc_number: Some(1),
                duration: Duration::from_secs(180 + index as u64 * 7),
                year: Some(2024),
                genre: Some("Elettronica".into()),
                cover_path: None,
                added_at: 1_700_000_000,
            });
        }
        let albums = albums
            .iter()
            .enumerate()
            .map(|(index, title)| Album {
                id: AlbumId::new(index as i64 + 1),
                title: (*title).into(),
                artist: artists[index % artists.len()].into(),
                year: Some(2024),
                cover_path: None,
                track_count: 12,
            })
            .collect();
        let artists = artists
            .iter()
            .enumerate()
            .map(|(index, name)| Artist {
                id: ArtistId::new(index as i64 + 1),
                name: (*name).into(),
                album_count: 4,
                track_count: 8,
            })
            .collect();
        Self {
            tracks,
            albums,
            artists,
            waker: Mutex::new(None),
        }
    }
}

impl Backend for MockBackend {
    fn send(&self, _command: Command) {}

    fn try_recv_event(&self) -> Option<Event> {
        None
    }

    fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(waker);
        }
    }

    fn tracks(&self, _sort: TrackSort, limit: usize, offset: usize) -> Result<Vec<Track>, String> {
        Ok(self
            .tracks
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect())
    }

    fn tracks_for_album(&self, album_id: AlbumId) -> Result<Vec<Track>, String> {
        Ok(self
            .tracks
            .iter()
            .filter(|track| {
                track.album
                    == self
                        .albums
                        .iter()
                        .find(|a| a.id == album_id)
                        .map_or("", |a| &a.title)
            })
            .cloned()
            .collect())
    }

    fn tracks_for_artist(&self, artist_id: ArtistId) -> Result<Vec<Track>, String> {
        Ok(self
            .tracks
            .iter()
            .filter(|track| {
                track.artist
                    == self
                        .artists
                        .iter()
                        .find(|a| a.id == artist_id)
                        .map_or("", |a| &a.name)
            })
            .cloned()
            .collect())
    }

    fn albums(&self) -> Result<Vec<Album>, String> {
        Ok(self.albums.clone())
    }

    fn artists(&self) -> Result<Vec<Artist>, String> {
        Ok(self.artists.clone())
    }

    fn search(&self, query: &str, limit: usize) -> Result<SearchResults, String> {
        let query = query.to_lowercase();
        let tracks = self
            .tracks
            .iter()
            .filter(|track| {
                track.title.to_lowercase().contains(&query)
                    || track.artist.to_lowercase().contains(&query)
                    || track.album.to_lowercase().contains(&query)
            })
            .take(limit)
            .cloned()
            .collect();
        let albums = self
            .albums
            .iter()
            .filter(|album| album.title.to_lowercase().contains(&query))
            .take(limit)
            .cloned()
            .collect();
        let artists = self
            .artists
            .iter()
            .filter(|artist| artist.name.to_lowercase().contains(&query))
            .take(limit)
            .cloned()
            .collect();
        Ok(SearchResults {
            tracks,
            albums,
            artists,
        })
    }

    fn stats(&self) -> Result<LibraryStats, String> {
        Ok(LibraryStats {
            tracks: self.tracks.len() as u64,
            albums: self.albums.len() as u64,
            artists: self.artists.len() as u64,
            total_duration: self.tracks.iter().map(|track| track.duration).sum(),
        })
    }

    fn folders(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("C:\\Music"), PathBuf::from("D:\\Archivio")]
    }

    fn add_folder(&self, _path: PathBuf) {}

    fn remove_folder(&self, _path: &Path) {}

    fn rescan(&self) {}

    fn playback_settings(&self) -> (u16, bool, RepeatMode) {
        (72, false, RepeatMode::All)
    }

    fn output_info(&self) -> (u32, u16) {
        (48_000, 2)
    }

    fn shutdown(&self) {}
}

fn main() {
    musify_ui::run(Arc::new(MockBackend::new()));
}
