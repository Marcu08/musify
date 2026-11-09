use std::path::{Path, PathBuf};
use std::sync::Arc;

use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, RepeatMode, SearchResults,
    SourceItem, Track, TrackSort,
};

#[derive(Debug, Clone, Default)]
pub struct RadioQuery {
    pub name: String,
    pub country: String,
    pub tag: String,
}

/// Everything the Rune layer needs from the application. The UI never sees cpal,
/// SQLite, Rune-side backend types or filesystem scanning internals.
pub trait Backend: Send + Sync {
    fn send(&self, command: Command);
    fn try_recv_event(&self) -> Option<Event>;
    fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>);

    fn tracks(&self, sort: TrackSort, limit: usize, offset: usize) -> Result<Vec<Track>, String>;
    fn tracks_for_album(&self, album_id: AlbumId) -> Result<Vec<Track>, String>;
    fn tracks_for_artist(&self, artist_id: ArtistId) -> Result<Vec<Track>, String>;
    fn albums(&self) -> Result<Vec<Album>, String>;
    fn artists(&self) -> Result<Vec<Artist>, String>;
    fn search(&self, query: &str, limit: usize) -> Result<SearchResults, String>;
    fn stats(&self) -> Result<LibraryStats, String>;

    fn folders(&self) -> Vec<PathBuf>;
    fn add_folder(&self, path: PathBuf);
    fn remove_folder(&self, path: &Path);
    fn rescan(&self);

    fn radio_search(&self, _query: RadioQuery) {}
    fn radio_favorites(&self) -> Result<Vec<SourceItem>, String> {
        Ok(Vec::new())
    }
    fn radio_toggle_favorite(&self, _id: &str) -> Result<bool, String> {
        Err("radio is not available".into())
    }
    fn radio_play(&self, _id: &str) {}

    fn playback_settings(&self) -> (u16, bool, RepeatMode);
    fn output_info(&self) -> (u32, u16);
    fn window_ready(&self, _hwnd: isize) {}
    fn shutdown(&self);
}
