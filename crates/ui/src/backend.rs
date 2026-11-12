use std::path::{Path, PathBuf};
use std::sync::Arc;

use musify_core::{
    Album, AlbumId, Artist, ArtistId, Command, Event, LibraryStats, Playlist, PlaylistId,
    PlaylistItem, PlaylistItemId, RepeatMode, SearchResults, SourceEntry, SourceItem, SourceQuery,
    Track, TrackSort,
};

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

    fn source_search(&self, _source: &str, _query: SourceQuery) {}
    fn source_favorites(&self, _source: &str) -> Result<Vec<SourceItem>, String> {
        Ok(Vec::new())
    }
    fn source_toggle_favorite(&self, _source: &str, _id: &str) -> Result<bool, String> {
        Err("source is not available".into())
    }
    fn source_play(&self, _source: &str, _id: &str) {}
    fn source_entries(&self, _source: &str, _id: &str) -> Result<Vec<SourceEntry>, String> {
        Err("source entries are not available".into())
    }
    fn source_refresh(&self, _source: &str, _id: &str) {}
    fn source_set_watched(&self, _source: &str, _id: &str, _watched: bool) -> Result<(), String> {
        Err("source entries are not available".into())
    }
    fn source_available(&self, _source: &str) -> bool {
        false
    }
    fn add_source_to_playlist(
        &self,
        _playlist_id: PlaylistId,
        _source: &str,
        _id: &str,
    ) -> Result<usize, String> {
        Err("source items cannot be added to playlists".into())
    }

    fn playlists(&self) -> Result<Vec<Playlist>, String> {
        Ok(Vec::new())
    }
    fn playlist_items(&self, _id: PlaylistId) -> Result<Vec<PlaylistItem>, String> {
        Ok(Vec::new())
    }
    fn create_playlist(&self, _name: &str) -> Result<PlaylistId, String> {
        Err("playlists are not available".into())
    }
    fn rename_playlist(&self, _id: PlaylistId, _name: &str) -> Result<(), String> {
        Err("playlists are not available".into())
    }
    fn delete_playlist(&self, _id: PlaylistId) -> Result<(), String> {
        Err("playlists are not available".into())
    }
    fn add_tracks_to_playlist(&self, _id: PlaylistId, _tracks: &[Track]) -> Result<usize, String> {
        Err("playlists are not available".into())
    }
    fn remove_playlist_item(&self, _item_id: PlaylistItemId) -> Result<bool, String> {
        Ok(false)
    }
    fn move_playlist_item(
        &self,
        _playlist_id: PlaylistId,
        _item_id: PlaylistItemId,
        _delta: i32,
    ) -> Result<bool, String> {
        Ok(false)
    }
    fn remove_missing_playlist_items(&self, _id: PlaylistId) -> Result<usize, String> {
        Ok(0)
    }
    fn play_playlist(&self, _id: PlaylistId) {}
    fn enqueue_playlist(&self, _id: PlaylistId) {}
    fn save_queue_as_playlist(&self, _name: &str, _tracks: &[Track]) -> Result<PlaylistId, String> {
        Err("playlists are not available".into())
    }
    fn import_playlist(&self, _path: &Path) -> Result<PlaylistId, String> {
        Err("playlists are not available".into())
    }
    fn export_playlist(&self, _id: PlaylistId, _path: &Path) -> Result<(), String> {
        Err("playlists are not available".into())
    }

    fn playback_settings(&self) -> (u16, bool, RepeatMode);
    fn output_info(&self) -> (u32, u16);
    fn window_ready(&self, _hwnd: isize) {}
    fn shutdown(&self);
}
