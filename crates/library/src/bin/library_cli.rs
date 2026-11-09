//! Command-line harness for the local library (M2).
//!
//! ```text
//! library-cli scan C:\Music [--db path] [--covers path]
//! library-cli search "città" [--db path]
//! library-cli stats [--db path]
//! library-cli bench C:\Music [--db path] [--covers path]
//! ```

use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use musify_library::{Library, LibraryStats, ScanProgress};

fn main() {
    if let Err(error) = run() {
        eprintln!("library-cli: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().ok_or_else(usage_error)?;
    let mut db = PathBuf::from("testdata/library.sqlite3");
    let mut covers = PathBuf::from("testdata/covers");
    let mut folders = Vec::new();
    let mut search_query = None;
    let mut limit = 100usize;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--db" => {
                db = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--db richiede un percorso".to_string())?,
                );
            }
            "--covers" => {
                covers = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--covers richiede un percorso".to_string())?,
                );
            }
            "--limit" => {
                limit = arguments
                    .next()
                    .ok_or_else(|| "--limit richiede un numero".to_string())?
                    .parse::<usize>()
                    .map_err(|error| format!("--limit non valido: {error}"))?;
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("opzione sconosciuta: {other}").into());
            }
            other => {
                if command == "search" {
                    search_query = Some(other.to_string());
                } else {
                    folders.push(PathBuf::from(other));
                }
            }
        }
    }

    match command.as_str() {
        "scan" => {
            if folders.is_empty() {
                return Err("scan richiede almeno una cartella".into());
            }
            let library = Library::open(&db, &covers)?;
            let report = library.scan_with_progress(&folders, print_progress)?;
            println!(
                "scansione: +{} nuovi, {} aggiornati, {} rimossi, {} errori in {:.2} s",
                report.added,
                report.updated,
                report.removed,
                report.errors,
                report.elapsed.as_secs_f64()
            );
            print_stats(&library.stats()?);
        }
        "search" => {
            let query = search_query
                .as_deref()
                .ok_or_else(|| "search richiede una query".to_string())?;
            let library = Library::open(&db, &covers)?;
            let results = library.search_all(query, limit)?;
            println!("brani: {}", results.tracks.len());
            for track in &results.tracks {
                println!("  {} — {} · {}", track.title, track.artist, track.album);
            }
            println!("album: {}", results.albums.len());
            for album in &results.albums {
                println!(
                    "  {} — {} ({} brani)",
                    album.title, album.artist, album.track_count
                );
            }
            println!("artisti: {}", results.artists.len());
            for artist in &results.artists {
                println!(
                    "  {} ({} album, {} brani)",
                    artist.name, artist.album_count, artist.track_count
                );
            }
        }
        "stats" => {
            let library = Library::open(&db, &covers)?;
            print_stats(&library.stats()?);
        }
        "bench" => {
            if folders.is_empty() {
                return Err("bench richiede una cartella".into());
            }
            remove_if_exists(&db);
            remove_if_exists(&db.with_extension("sqlite3-wal"));
            remove_if_exists(&db.with_extension("sqlite3-shm"));
            let library = Library::open(&db, &covers)?;
            println!("prima scansione (indice freddo)...");
            let first = library.scan_with_progress(&folders, print_progress)?;
            println!(
                "+{} nuovi, {} aggiornati, {} rimossi, {} errori in {:.2} s",
                first.added,
                first.updated,
                first.removed,
                first.errors,
                first.elapsed.as_secs_f64()
            );
            println!("seconda scansione (incrementale)...");
            let second = library.scan_with_progress(&folders, print_progress)?;
            println!(
                "+{} nuovi, {} aggiornati, {} rimossi, {} errori in {:.2} s",
                second.added,
                second.updated,
                second.removed,
                second.errors,
                second.elapsed.as_secs_f64()
            );
            print_stats(&library.stats()?);
        }
        other => return Err(format!("comando sconosciuto: {other}").into()),
    }
    Ok(())
}

fn print_progress(progress: ScanProgress) {
    if progress.scanned == 0
        || progress.scanned == progress.total
        || progress.scanned.is_multiple_of(1000)
    {
        println!(
            "  {}/{} {}",
            progress.scanned,
            progress.total,
            progress
                .current
                .as_ref()
                .map_or_else(String::new, |path| path.display().to_string())
        );
    }
}

fn print_stats(stats: &LibraryStats) {
    println!(
        "libreria: {} brani · {} album · {} artisti · durata {}",
        stats.tracks,
        stats.albums,
        stats.artists,
        format_duration(stats.total_duration)
    );
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!(
        "{}h {:02}m {:02}s",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

fn remove_if_exists(path: &PathBuf) {
    if path.exists()
        && let Err(error) = std::fs::remove_file(path)
    {
        eprintln!("impossibile rimuovere {}: {error}", path.display());
    }
}

fn usage_error() -> Box<dyn Error> {
    print_usage();
    Box::new(std::io::Error::other("comando mancante"))
}

fn print_usage() {
    println!(
        "Musify library-cli\n\
         \n\
         Uso:\n\
           library-cli scan <cartella...> [--db file] [--covers dir]\n\
           library-cli search <query> [--db file] [--limit N]\n\
           library-cli stats [--db file]\n\
           library-cli bench <cartella> [--db file] [--covers dir]"
    );
}
