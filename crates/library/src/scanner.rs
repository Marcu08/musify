use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use lofty::file::{AudioFile, TaggedFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use rayon::prelude::*;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use walkdir::WalkDir;

use crate::cover;
use crate::db::{configure_connection, find_track_stamp, init_schema};
use crate::{LibraryError, ScanProgress, ScanReport};

const AUDIO_EXTENSIONS: &[&str] = &["aac", "flac", "m4a", "mp3", "mp4", "oga", "ogg", "wav"];
const BATCH_SIZE: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Added,
    Updated,
}

struct Stamp {
    path: PathBuf,
    mtime_ns: i64,
    file_size: i64,
}

struct ParsedFile {
    path: PathBuf,
    mtime_ns: i64,
    file_size: i64,
    metadata: ExtractedMetadata,
    metadata_error: bool,
}

pub(crate) fn scan(
    db_path: &Path,
    cover_dir: &Path,
    roots: &[PathBuf],
    mut progress: impl FnMut(ScanProgress),
) -> Result<ScanReport, LibraryError> {
    let started = Instant::now();
    let mut errors = 0u64;
    let files = collect_files(roots, &mut errors)?;
    let total = files.len() as u64;
    progress(ScanProgress {
        scanned: 0,
        total,
        current: None,
    });

    let mut connection = Connection::open(db_path)?;
    configure_connection(&connection)?;
    init_schema(&connection)?;

    let mut seen = HashSet::with_capacity(files.len());
    let mut added = 0u64;
    let mut updated = 0u64;
    let mut scanned = 0u64;

    for chunk in files.chunks(BATCH_SIZE) {
        let mut jobs = Vec::new();
        for path in chunk {
            seen.insert(normalize_path(path));
            let metadata = match fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    errors += 1;
                    scanned += 1;
                    tracing::warn!(path = %path.display(), %error, "stat failed during scan");
                    progress(ScanProgress {
                        scanned,
                        total,
                        current: Some(path.clone()),
                    });
                    continue;
                }
            };
            let file_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
            let mtime_ns = modified_ns(&metadata);
            let path_string = path.to_string_lossy().to_string();
            let unchanged = find_track_stamp(&connection, &path_string)
                .ok()
                .flatten()
                .is_some_and(|(old_mtime, old_size)| {
                    old_mtime == mtime_ns && old_size == file_size
                });
            if unchanged {
                scanned += 1;
                progress(ScanProgress {
                    scanned,
                    total,
                    current: Some(path.clone()),
                });
                continue;
            }
            jobs.push(Stamp {
                path: path.clone(),
                mtime_ns,
                file_size,
            });
        }

        let parsed: Vec<ParsedFile> = jobs
            .par_iter()
            .map(|stamp| parse_file(stamp, cover_dir))
            .collect();
        let transaction = connection.transaction()?;
        for parsed in parsed {
            match upsert_file(&transaction, &parsed) {
                Ok(outcome) => {
                    if parsed.metadata_error {
                        errors += 1;
                    }
                    match outcome {
                        Outcome::Added => added += 1,
                        Outcome::Updated => updated += 1,
                    }
                }
                Err(error) => {
                    errors += 1;
                    tracing::warn!(path = %parsed.path.display(), %error, "file skipped during scan");
                }
            }
            scanned += 1;
            progress(ScanProgress {
                scanned,
                total,
                current: Some(parsed.path),
            });
        }
        transaction.commit()?;
    }

    let removed = remove_missing(&mut connection, roots, &seen)?;
    connection.execute_batch(
        "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks);\
         DELETE FROM artists WHERE id NOT IN (SELECT DISTINCT artist_id FROM tracks);",
    )?;

    Ok(ScanReport {
        added,
        updated,
        removed,
        errors,
        elapsed: started.elapsed(),
    })
}

fn parse_file(stamp: &Stamp, cover_dir: &Path) -> ParsedFile {
    let (tagged, metadata_error) = match lofty::read_from_path(&stamp.path) {
        Ok(tagged) => (Some(tagged), false),
        Err(error) => {
            tracing::debug!(path = %stamp.path.display(), %error, "metadata read failed");
            (None, true)
        }
    };
    let metadata = extract_metadata(&stamp.path, tagged.as_ref(), cover_dir);
    ParsedFile {
        path: stamp.path.clone(),
        mtime_ns: stamp.mtime_ns,
        file_size: stamp.file_size,
        metadata,
        metadata_error,
    }
}

fn upsert_file(
    transaction: &Transaction<'_>,
    parsed: &ParsedFile,
) -> Result<Outcome, LibraryError> {
    let path_string = parsed.path.to_string_lossy().to_string();
    let existing: Option<(i64, i64)> = transaction
        .query_row(
            "SELECT id, added_at FROM tracks WHERE path = ?1",
            params![path_string],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let artist_id = upsert_artist(transaction, &parsed.metadata.artist)?;
    let album_id = upsert_album(
        transaction,
        &parsed.metadata.album,
        &parsed.metadata.album_artist,
        parsed.metadata.year,
        parsed.metadata.cover_path.as_deref(),
    )?;
    let now = unix_now();
    let cover_string = parsed
        .metadata
        .cover_path
        .as_ref()
        .map(|cover| cover.to_string_lossy().into_owned());

    let outcome = if let Some((id, _added_at)) = existing {
        transaction.execute(
            "UPDATE tracks SET path = ?1, title = ?2, artist = ?3, album = ?4, \
             album_artist = ?5, artist_id = ?6, album_id = ?7, track_number = ?8, \
             disc_number = ?9, duration_ms = ?10, year = ?11, genre = ?12, cover_path = ?13, \
             mtime_ns = ?14, file_size = ?15, updated_at = ?16 WHERE id = ?17",
            params![
                path_string,
                parsed.metadata.title,
                parsed.metadata.artist,
                parsed.metadata.album,
                parsed.metadata.album_artist,
                artist_id,
                album_id,
                parsed.metadata.track_number,
                parsed.metadata.disc_number,
                parsed.metadata.duration_ms,
                parsed.metadata.year,
                parsed.metadata.genre,
                cover_string,
                parsed.mtime_ns,
                parsed.file_size,
                now,
                id,
            ],
        )?;
        Outcome::Updated
    } else {
        transaction.execute(
            "INSERT INTO tracks(path, title, artist, album, album_artist, artist_id, album_id, \
             track_number, disc_number, duration_ms, year, genre, cover_path, mtime_ns, file_size, \
             added_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)",
            params![
                path_string,
                parsed.metadata.title,
                parsed.metadata.artist,
                parsed.metadata.album,
                parsed.metadata.album_artist,
                artist_id,
                album_id,
                parsed.metadata.track_number,
                parsed.metadata.disc_number,
                parsed.metadata.duration_ms,
                parsed.metadata.year,
                parsed.metadata.genre,
                cover_string,
                parsed.mtime_ns,
                parsed.file_size,
                now,
            ],
        )?;
        Outcome::Added
    };

    Ok(outcome)
}

struct ExtractedMetadata {
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    track_number: Option<i64>,
    disc_number: Option<i64>,
    duration_ms: i64,
    year: Option<i64>,
    genre: Option<String>,
    cover_path: Option<PathBuf>,
}

fn extract_metadata(
    path: &Path,
    tagged: Option<&TaggedFile>,
    cover_dir: &Path,
) -> ExtractedMetadata {
    let tag = tagged.and_then(|file| file.primary_tag().or_else(|| file.first_tag()));
    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let title = tag
        .and_then(|tag| tag.title())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(file_stem);
    let artist = tag
        .and_then(|tag| tag.artist())
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let album = tag
        .and_then(|tag| tag.album())
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let album_artist = tag
        .and_then(|tag| tag.get_string(ItemKey::AlbumArtist))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(|| artist.clone(), |value| value.to_string());
    let track_number = tag.and_then(|tag| tag.track()).map(i64::from);
    let disc_number = tag.and_then(|tag| tag.disk()).map(i64::from);
    let year = tag
        .and_then(|tag| tag.date())
        .map(|date| i64::from(date.year));
    let genre = tag
        .and_then(|tag| tag.genre())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let duration_ms = tagged.map_or(0, |file| {
        i64::try_from(file.properties().duration().as_millis()).unwrap_or(i64::MAX)
    });
    let cover_path = cover::extract(tagged, path, cover_dir);

    ExtractedMetadata {
        title,
        artist,
        album,
        album_artist,
        track_number,
        disc_number,
        duration_ms,
        year,
        genre,
        cover_path,
    }
}

fn upsert_artist(transaction: &Transaction<'_>, name: &str) -> Result<i64, LibraryError> {
    transaction.execute(
        "INSERT INTO artists(name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
        params![name],
    )?;
    Ok(transaction.query_row(
        "SELECT id FROM artists WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?)
}

fn upsert_album(
    transaction: &Transaction<'_>,
    title: &str,
    artist: &str,
    year: Option<i64>,
    cover_path: Option<&Path>,
) -> Result<i64, LibraryError> {
    let cover = cover_path.map(|cover| cover.to_string_lossy().into_owned());
    transaction.execute(
        "INSERT INTO albums(title, artist, year, cover_path) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(title, artist) DO UPDATE SET \
            year = COALESCE(albums.year, excluded.year), \
            cover_path = COALESCE(albums.cover_path, excluded.cover_path)",
        params![title, artist, year, cover],
    )?;
    Ok(transaction.query_row(
        "SELECT id FROM albums WHERE title = ?1 AND artist = ?2",
        params![title, artist],
        |row| row.get(0),
    )?)
}

fn collect_files(roots: &[PathBuf], errors: &mut u64) -> Result<Vec<PathBuf>, LibraryError> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let root = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        if !root.exists() {
            *errors += 1;
            tracing::warn!(path = %root.display(), "music folder does not exist");
            continue;
        }
        for entry in WalkDir::new(&root).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    *errors += 1;
                    tracing::debug!(%error, "walk error");
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_audio_file(entry.path()) {
                continue;
            }
            let key = normalize_path(entry.path());
            if seen.insert(key) {
                files.push(entry.into_path());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn remove_missing(
    connection: &mut Connection,
    roots: &[PathBuf],
    seen: &HashSet<String>,
) -> Result<u64, LibraryError> {
    if roots.is_empty() {
        return Ok(0);
    }
    let normalized_roots: Vec<String> = roots
        .iter()
        .map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.clone()))
        .map(|root| normalize_path(&root))
        .collect();

    let mut statement = connection.prepare("SELECT id, path FROM tracks")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut missing = Vec::new();
    for row in rows {
        let (id, path) = row?;
        let normalized = normalize_path(Path::new(&path));
        let under_scanned_root = normalized_roots
            .iter()
            .any(|root| normalized == *root || normalized.starts_with(&format!("{root}\\")));
        if under_scanned_root && !seen.contains(&normalized) {
            missing.push(id);
        }
    }
    drop(statement);

    let transaction = connection.transaction()?;
    for id in &missing {
        transaction.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
    }
    transaction.commit()?;
    Ok(missing.len() as u64)
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| AUDIO_EXTENSIONS.contains(&extension.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

fn modified_ns(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| {
            i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX)
        })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn audio_extensions_are_case_insensitive() {
        assert!(is_audio_file(Path::new("song.MP3")));
        assert!(is_audio_file(Path::new("song.flac")));
        assert!(!is_audio_file(Path::new("cover.jpg")));
    }

    #[test]
    fn path_normalization_is_stable_on_windows() {
        assert_eq!(
            normalize_path(Path::new(r"C:\Music\Artist\Song.FLAC")),
            r"c:\music\artist\song.flac"
        );
    }

    #[test]
    fn embedded_cover_is_cached_and_resized() {
        let dir = tempfile::tempdir().expect("temp");
        let music = dir.path().join("music");
        std::fs::create_dir_all(&music).expect("music dir");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("assets")
                .join("tone-cover.mp3"),
            music.join("tone-cover.mp3"),
        )
        .expect("copy cover asset");
        let library = crate::Library::open(
            dir.path().join("library.sqlite3"),
            dir.path().join("covers"),
        )
        .expect("library");
        library.scan(std::slice::from_ref(&music)).expect("scan");

        let tracks = library
            .tracks_sorted(
                musify_core::TrackSort::Title,
                musify_core::SortDirection::Ascending,
                10,
                0,
            )
            .expect("tracks");
        assert_eq!(tracks.len(), 1);
        let cover = tracks[0].cover_path.as_ref().expect("track cover");
        assert!(cover.exists(), "cover file missing: {}", cover.display());
        let image = image::open(cover).expect("open cover");
        assert!(image.width() <= 512);
        assert!(image.height() <= 512);

        let albums = library.albums_page(10, 0).expect("albums");
        assert_eq!(albums.len(), 1);
        assert!(albums[0].cover_path.is_some());
    }

    #[test]
    fn scan_indexes_assets_and_rescans_incrementally() {
        let dir = tempfile::tempdir().expect("temp");
        let music = dir.path().join("music");
        std::fs::create_dir_all(&music).expect("music dir");
        for name in ["tone.mp3", "tone.flac", "tone.ogg"] {
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../audio/tests/assets")
                .join(name);
            let destination = music.join(name);
            std::fs::copy(source, destination).expect("copy asset");
        }
        let db = dir.path().join("library.sqlite3");
        let covers = dir.path().join("covers");
        let library = crate::Library::open(&db, &covers).expect("library");
        let first = library
            .scan(std::slice::from_ref(&music))
            .expect("first scan");
        assert_eq!(first.added, 3);
        assert_eq!(first.updated, 0);
        let stats = library.stats().expect("stats");
        assert_eq!(stats.tracks, 3);
        assert_eq!(stats.albums, 1);
        assert_eq!(stats.artists, 1);

        let second = library
            .scan(std::slice::from_ref(&music))
            .expect("second scan");
        assert_eq!(second.added, 0);
        assert_eq!(second.updated, 0);
        assert_eq!(second.removed, 0);

        std::fs::remove_file(music.join("tone.ogg")).expect("remove file");
        let third = library.scan(&[music]).expect("third scan");
        assert_eq!(third.removed, 1);
        let stats = library.stats().expect("stats after removal");
        assert_eq!(stats.tracks, 2);
        assert_eq!(stats.albums, 1);
    }
}
