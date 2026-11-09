//! Small command-line harness for the audio engine (M1).
//!
//! ```text
//! cargo run -p musify-audio --bin audio-cli -- track1.flac track2.mp3
//! ```
//!
//! Commands on stdin: `pause`, `resume`, `toggle`, `next`, `prev`, `seek 12.5`,
//! `vol 70`, `status`, `quit`.

use std::io::{self, BufRead};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use musify_audio::{AudioEngine, PlaybackSnapshot};
use musify_core::{Command, Event, PlaybackState, Track, TrackId};

fn main() {
    if let Err(error) = run() {
        eprintln!("audio-cli: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.files.is_empty() {
        print_help();
        return Ok(());
    }
    let engine = AudioEngine::start(args.volume).map_err(|error| error.to_string())?;
    let info = engine.output_info();
    println!(
        "output: {} Hz, {} canali · {} brani · volume {}",
        info.sample_rate,
        info.channels,
        args.files.len(),
        args.volume
    );

    let tracks = args
        .files
        .iter()
        .enumerate()
        .map(|(index, path)| track(index as i64 + 1, path.clone()))
        .collect::<Vec<_>>();
    engine
        .send(Command::PlayTracks {
            tracks,
            start_index: args.start_index,
        })
        .map_err(|error| error.to_string())?;
    if let Some(seconds) = args.seek {
        engine
            .send(Command::Seek(Duration::from_secs_f64(seconds)))
            .map_err(|error| error.to_string())?;
    }

    let (command_tx, command_rx) = crossbeam_channel::unbounded::<String>();
    thread::Builder::new()
        .name("audio-cli-stdin".into())
        .spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines().map_while(Result::ok) {
                if command_tx.send(line).is_err() {
                    break;
                }
            }
        })
        .map_err(|error| error.to_string())?;

    let started = Instant::now();
    let mut last_position_print = Instant::now();

    loop {
        if let Some(limit) = args.seconds
            && started.elapsed() >= Duration::from_secs_f64(limit)
        {
            engine
                .send(Command::Shutdown)
                .map_err(|error| error.to_string())?;
            break;
        }

        while let Ok(line) = command_rx.try_recv() {
            if !handle_command(&engine, line.trim()) {
                engine
                    .send(Command::Shutdown)
                    .map_err(|error| error.to_string())?;
                print_snapshot(&engine.snapshot());
                return Ok(());
            }
        }

        if let Some(event) = engine.recv_event_timeout(Duration::from_millis(50)) {
            match event {
                Event::PlaybackStateChanged(state) => match state {
                    PlaybackState::Stopped => println!("stato: fermo"),
                    PlaybackState::Playing => println!("stato: in riproduzione"),
                    PlaybackState::Paused => println!("stato: in pausa"),
                },
                Event::TrackChanged(Some(item)) => {
                    println!(
                        "brano: {} — {} ({})",
                        item.track.title,
                        item.track.artist,
                        format_duration(item.track.duration)
                    );
                }
                Event::TrackChanged(None) => println!("brano: nessuno"),
                Event::PositionChanged { position, duration } => {
                    if last_position_print.elapsed() >= Duration::from_secs(1) {
                        last_position_print = Instant::now();
                        let total = duration.map_or_else(|| "--:--".into(), format_duration);
                        println!("posizione: {} / {}", format_duration(position), total);
                    }
                }
                Event::QueueChanged(queue) => {
                    println!("coda: {} brani", queue.len());
                }
                Event::VolumeChanged(volume) => println!("volume: {volume}"),
                Event::ShuffleChanged(shuffle) => {
                    println!("shuffle: {}", if shuffle { "on" } else { "off" });
                }
                Event::RepeatChanged(repeat) => println!("repeat: {repeat:?}"),
                Event::Error { context, message } => {
                    eprintln!("errore [{context}]: {message}");
                }
                Event::Shutdown => break,
                _ => {}
            }
        }
    }

    let snapshot = engine.snapshot();
    print_snapshot(&snapshot);
    Ok(())
}

fn handle_command(engine: &AudioEngine, line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(command) = parts.next() else {
        return true;
    };
    let command = command.to_lowercase();
    match command.as_str() {
        "quit" | "q" | "exit" => return false,
        "pause" | "p" => send(engine, Command::Pause),
        "resume" | "r" | "play" => send(engine, Command::Resume),
        "toggle" | "space" => send(engine, Command::TogglePlayPause),
        "next" | "n" => send(engine, Command::Next),
        "prev" | "previous" | "b" => send(engine, Command::Previous),
        "stop" | "s" => send(engine, Command::Stop),
        "seek" => match parts.next().and_then(|value| value.parse::<f64>().ok()) {
            Some(seconds) if seconds >= 0.0 => {
                send(engine, Command::Seek(Duration::from_secs_f64(seconds)));
            }
            _ => eprintln!("uso: seek <secondi>"),
        },
        "vol" | "volume" => match parts.next().and_then(|value| value.parse::<u16>().ok()) {
            Some(volume) => send(engine, Command::SetVolume(volume)),
            None => eprintln!("uso: vol <0-100>"),
        },
        "status" => print_snapshot(&engine.snapshot()),
        "help" | "?" => print_commands(),
        other => eprintln!("comando sconosciuto: {other}"),
    }
    true
}

fn send(engine: &AudioEngine, command: Command) {
    if let Err(error) = engine.send(command) {
        eprintln!("invio comando fallito: {error}");
    }
}

fn print_snapshot(snapshot: &PlaybackSnapshot) {
    println!(
        "snapshot: {:?} · posizione {} · volume {} · coda {} · shuffle {} · repeat {:?} · underrun {}",
        snapshot.state,
        format_duration(snapshot.position),
        snapshot.volume,
        snapshot.queue.len(),
        snapshot.shuffle,
        snapshot.repeat,
        snapshot.underruns
    );
    if let Some(item) = &snapshot.current {
        println!("corrente: {} — {}", item.track.title, item.track.artist);
    }
}

fn track(id: i64, path: PathBuf) -> Track {
    let title = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Sconosciuto")
        .to_string();
    Track {
        id: TrackId::new(id),
        source: musify_core::SourceId::local(&path),
        path,
        title,
        artist: String::new(),
        album: String::new(),
        album_artist: None,
        track_number: None,
        disc_number: None,
        duration: Duration::ZERO,
        year: None,
        genre: None,
        cover_path: None,
        added_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs() as i64),
    }
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    format!("{:02}:{:02}", total / 60, total % 60)
}

struct Args {
    files: Vec<PathBuf>,
    start_index: usize,
    volume: u16,
    seconds: Option<f64>,
    seek: Option<f64>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        files: Vec::new(),
        start_index: 0,
        volume: 80,
        seconds: None,
        seek: None,
    };
    let mut iterator = std::env::args().skip(1);
    while let Some(argument) = iterator.next() {
        match argument.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--seconds" | "--duration" => {
                args.seconds = Some(parse_number(iterator.next(), "--seconds")?);
            }
            "--volume" | "--vol" => {
                let volume = parse_number(iterator.next(), "--volume")?;
                args.volume = volume.clamp(0.0, 100.0) as u16;
            }
            "--seek" => {
                args.seek = Some(parse_number(iterator.next(), "--seek")?);
            }
            "--start" | "--index" => {
                let index = parse_number(iterator.next(), "--start")?;
                args.start_index = index.max(0.0) as usize;
            }
            other if other.starts_with('-') => {
                return Err(format!("opzione sconosciuta: {other}"));
            }
            file => args.files.push(PathBuf::from(file)),
        }
    }
    Ok(args)
}

fn parse_number(value: Option<String>, option: &str) -> Result<f64, String> {
    value
        .ok_or_else(|| format!("{option} richiede un valore"))?
        .parse::<f64>()
        .map_err(|error| format!("valore non valido per {option}: {error}"))
}

fn print_help() {
    println!(
        "Musify audio-cli\n\
         \n\
         Uso: audio-cli [--volume 0-100] [--seconds N] [--seek SECONDI] [--start INDICE] FILE...\n\
         \n\
         Comandi da stdin: pause, resume, toggle, next, prev, seek <s>, vol <0-100>, status, quit"
    );
}

fn print_commands() {
    println!(
        "pause | resume | toggle | next | prev | seek <secondi> | vol <0-100> | status | quit"
    );
}
