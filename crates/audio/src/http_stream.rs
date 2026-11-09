use std::collections::VecDeque;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use musify_core::{StreamEvent, StreamStatus};
use symphonia::core::io::MediaSource;

use crate::error::AudioError;

const USER_AGENT: &str = "Musify/0.1.0 (+https://github.com/Marcu08/musify)";
const INITIAL_TARGET: usize = 128 * 1024;
const MAX_TARGET: usize = 1024 * 1024;
const START_THRESHOLD: usize = 16 * 1024;
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// A live HTTP stream with auto-reconnect.
///
/// The network thread owns a small Tokio runtime that exists only while the
/// stream exists. Bytes are buffered ahead of the decoder; the buffer target
/// grows when the decoder starves, up to a fixed cap.
pub(crate) struct HttpStream {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
}

struct Shared {
    state: Mutex<State>,
    ready: Condvar,
    updates: Mutex<VecDeque<StreamEvent>>,
}

struct State {
    buffer: VecDeque<u8>,
    target: usize,
    ready: bool,
    content_type: Option<String>,
    first_error: Option<String>,
    status: StreamStatus,
    title: Option<String>,
    stopped: bool,
}

impl HttpStream {
    pub fn start(url: String) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                buffer: VecDeque::with_capacity(INITIAL_TARGET),
                target: INITIAL_TARGET,
                ready: false,
                content_type: None,
                first_error: None,
                status: StreamStatus::Connecting,
                title: None,
                stopped: false,
            }),
            ready: Condvar::new(),
            updates: Mutex::new(VecDeque::new()),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let thread_shared = Arc::clone(&shared);
        let thread_stop = Arc::clone(&stop);
        if let Err(error) = thread::Builder::new()
            .name("musify-radio-stream".into())
            .spawn(move || run_stream(url, thread_shared, thread_stop))
        {
            let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
            state.first_error = Some(format!("cannot start the stream thread: {error}"));
            state.ready = true;
            shared.ready.notify_all();
        }
        Self { shared, stop }
    }

    pub fn reader(&self) -> HttpReader {
        HttpReader {
            shared: Arc::clone(&self.shared),
            stop: Arc::clone(&self.stop),
        }
    }

    /// Wait for the first bytes (or a fatal startup error) and return the
    /// HTTP content type, used as a probe hint.
    pub fn wait_ready(&self) -> Result<Option<String>, AudioError> {
        let deadline = Instant::now() + READY_TIMEOUT;
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if state.ready {
                if let Some(error) = &state.first_error
                    && state.buffer.is_empty()
                {
                    return Err(AudioError::Decode(error.clone()));
                }
                return Ok(state.content_type.clone());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AudioError::Decode(
                    "the radio stream did not send data within 15 seconds".into(),
                ));
            }
            let (next, timeout) = self
                .shared
                .ready
                .wait_timeout(state, remaining)
                .unwrap_or_else(|p| p.into_inner());
            state = next;
            if timeout.timed_out() && !state.ready {
                return Err(AudioError::Decode(
                    "the radio stream did not send data within 15 seconds".into(),
                ));
            }
        }
    }

    pub fn take_updates(&self) -> Vec<StreamEvent> {
        let mut updates = self
            .shared
            .updates
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        updates.drain(..).collect()
    }
}

impl Drop for HttpStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(mut state) = self.shared.state.lock() {
            state.stopped = true;
            state.buffer.clear();
        }
        self.shared.ready.notify_all();
    }
}

pub(crate) struct HttpReader {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
}

impl Read for HttpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if !state.buffer.is_empty() {
                let count = buf.len().min(state.buffer.len());
                for slot in &mut buf[..count] {
                    *slot = state.buffer.pop_front().unwrap_or(0);
                }
                drop(state);
                self.shared.ready.notify_all();
                return Ok(count);
            }
            if let Some(error) = &state.first_error
                && !state.ready
            {
                return Err(io::Error::other(error.clone()));
            }
            if state.stopped || self.stop.load(Ordering::Acquire) {
                return Ok(0);
            }
            // The decoder is starving: ask the producer for a larger lead.
            if state.target < MAX_TARGET {
                state.target = (state.target * 2).min(MAX_TARGET);
            }
            let (next, _) = self
                .shared
                .ready
                .wait_timeout(state, Duration::from_millis(500))
                .unwrap_or_else(|p| p.into_inner());
            state = next;
        }
    }
}

impl Seek for HttpReader {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "a live radio stream is not seekable",
        ))
    }
}

impl MediaSource for HttpReader {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

fn run_stream(url: String, shared: Arc<Shared>, stop: Arc<AtomicBool>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            set_fatal(
                &shared,
                format!("cannot start the network runtime: {error}"),
            );
            return;
        }
    };
    runtime.block_on(async move {
        let client = match reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                set_fatal(&shared, format!("cannot create the HTTP client: {error}"));
                return;
            }
        };
        let mut attempt = 0u32;
        loop {
            if stop.load(Ordering::Acquire) {
                return;
            }
            let status = if attempt == 0 {
                StreamStatus::Connecting
            } else {
                StreamStatus::Reconnecting { attempt }
            };
            set_status(&shared, status);
            match read_once(&client, &url, &shared, &stop).await {
                Ok(()) if stop.load(Ordering::Acquire) => return,
                Ok(()) => set_status(
                    &shared,
                    StreamStatus::Error("the radio stream ended".into()),
                ),
                Err(error) => set_status(&shared, StreamStatus::Error(error)),
            }
            attempt = attempt.saturating_add(1);
            if attempt >= 6 {
                set_status(&shared, StreamStatus::Offline);
            } else {
                set_status(&shared, StreamStatus::Reconnecting { attempt });
            }
            let backoff = Duration::from_secs(1u64 << attempt.min(5)).min(MAX_BACKOFF);
            if !sleep_unless_stopped(backoff, &stop).await {
                return;
            }
            set_status(&shared, StreamStatus::Connecting);
        }
    });
}

async fn read_once(
    client: &reqwest::Client,
    url: &str,
    shared: &Arc<Shared>,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "audio/*")
        .header("Icy-MetaData", "1")
        .send()
        .await
        .map_err(|error| format!("cannot connect: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("the stream answered HTTP {}", response.status()));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let metaint = response
        .headers()
        .get("icy-metaint")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<usize>().ok());
    {
        let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.content_type.clone_from(&content_type);
    }
    set_status(shared, StreamStatus::Buffering);

    let mut parser = IcyParser::new(metaint);
    let mut stream = response.bytes_stream();
    loop {
        let chunk = match tokio::time::timeout(Duration::from_millis(500), stream.next()).await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return Err("the server closed the stream".into()),
            Err(_) => {
                if stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                continue;
            }
        };
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let chunk = chunk.map_err(|error| format!("network read failed: {error}"))?;
        let mut audio = Vec::with_capacity(chunk.len());
        let mut title = None;
        parser.feed(
            &chunk,
            |bytes| audio.extend_from_slice(bytes),
            |value| {
                title = Some(value.to_string());
            },
        );
        if let Some(title) = title {
            set_title(shared, title);
        }
        if !audio.is_empty() {
            push_audio(shared, stop, &audio)?;
        }
    }
}

fn push_audio(shared: &Arc<Shared>, stop: &Arc<AtomicBool>, bytes: &[u8]) -> Result<(), String> {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    let mut offset = 0;
    while offset < bytes.len() {
        if stop.load(Ordering::Acquire) || state.stopped {
            return Ok(());
        }
        let free = state.target.saturating_sub(state.buffer.len());
        if free == 0 {
            let (next, _) = shared
                .ready
                .wait_timeout(state, Duration::from_millis(500))
                .unwrap_or_else(|p| p.into_inner());
            state = next;
            continue;
        }
        let count = free.min(bytes.len() - offset);
        state
            .buffer
            .extend(bytes[offset..offset + count].iter().copied());
        offset += count;
    }
    let buffered = state.buffer.len();
    if !state.ready {
        state.ready = true;
    }
    drop(state);
    shared.ready.notify_all();
    if buffered >= START_THRESHOLD {
        set_status(shared, StreamStatus::Playing);
    }
    Ok(())
}

fn set_status(shared: &Arc<Shared>, status: StreamStatus) {
    let changed = {
        let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.status == status {
            false
        } else {
            state.status = status.clone();
            true
        }
    };
    if changed {
        shared
            .updates
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(StreamEvent::Status(status));
    }
}

fn set_title(shared: &Arc<Shared>, title: String) {
    let title = title.trim().to_string();
    if title.is_empty() {
        return;
    }
    let changed = {
        let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.title.as_deref() == Some(title.as_str()) {
            false
        } else {
            state.title = Some(title.clone());
            true
        }
    };
    if changed {
        shared
            .updates
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(StreamEvent::Title(Some(title)));
    }
}

fn set_fatal(shared: &Arc<Shared>, message: String) {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    state.first_error = Some(message.clone());
    state.ready = true;
    state.status = StreamStatus::Error(message);
    shared.ready.notify_all();
}

async fn sleep_unless_stopped(duration: Duration, stop: &Arc<AtomicBool>) -> bool {
    let slices = (duration.as_millis() / 200).max(1);
    for _ in 0..slices {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    !stop.load(Ordering::Acquire)
}

/// Strips ICY metadata blocks from an MP3/AAC stream and extracts `StreamTitle`.
struct IcyParser {
    metaint: Option<usize>,
    audio_left: usize,
    meta_left: usize,
    meta: Vec<u8>,
    in_meta_length: bool,
}

impl IcyParser {
    fn new(metaint: Option<usize>) -> Self {
        let metaint = metaint.filter(|value| *value > 0);
        Self {
            metaint,
            audio_left: metaint.unwrap_or(0),
            meta_left: 0,
            meta: Vec::new(),
            in_meta_length: metaint.is_some(),
        }
    }

    fn feed(&mut self, input: &[u8], mut audio: impl FnMut(&[u8]), mut title: impl FnMut(&str)) {
        let Some(metaint) = self.metaint else {
            audio(input);
            return;
        };
        let mut index = 0;
        while index < input.len() {
            if self.audio_left > 0 {
                let count = self.audio_left.min(input.len() - index);
                audio(&input[index..index + count]);
                index += count;
                self.audio_left -= count;
                continue;
            }
            if self.in_meta_length {
                let length = usize::from(input[index]) * 16;
                index += 1;
                self.in_meta_length = false;
                self.meta_left = length;
                self.meta.clear();
                if length == 0 {
                    self.audio_left = metaint;
                    self.in_meta_length = true;
                }
                continue;
            }
            let count = self.meta_left.min(input.len() - index);
            self.meta.extend_from_slice(&input[index..index + count]);
            index += count;
            self.meta_left -= count;
            if self.meta_left == 0 {
                if let Ok(text) = std::str::from_utf8(&self.meta)
                    && let Some(value) = parse_stream_title(text)
                {
                    title(&value);
                }
                self.audio_left = metaint;
                self.in_meta_length = true;
            }
        }
    }
}

fn parse_stream_title(text: &str) -> Option<String> {
    let text = text.trim_matches(char::from(0)).trim();
    let start = text.find("StreamTitle='")? + "StreamTitle='".len();
    let rest = &text[start..];
    let end = rest.find("';").or_else(|| rest.find('\''))?;
    let title = rest[..end].trim();
    (!title.is_empty()).then(|| title.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn icy_metadata_is_stripped_and_title_extracted() {
        let audio = [1u8, 2, 3, 4];
        let mut parser = IcyParser::new(Some(4));
        let metadata = b"StreamTitle='Artist - Song';";
        let mut blocks = [0u8; 32];
        blocks[..metadata.len()].copy_from_slice(metadata);
        let mut input = Vec::new();
        input.extend_from_slice(&audio);
        input.push(2); // two 16-byte metadata blocks
        input.extend_from_slice(&blocks);
        input.extend_from_slice(&audio);

        let mut out = Vec::new();
        let mut title = None;
        parser.feed(
            &input,
            |bytes| out.extend_from_slice(bytes),
            |value| title = Some(value.to_string()),
        );
        assert_eq!(out, [1, 2, 3, 4, 1, 2, 3, 4]);
        assert_eq!(title.as_deref(), Some("Artist - Song"));
    }

    #[test]
    fn icy_parser_handles_metadata_split_across_chunks() {
        let mut parser = IcyParser::new(Some(2));
        let metadata = b"StreamTitle='X';";
        let mut blocks = [0u8; 32];
        blocks[..metadata.len()].copy_from_slice(metadata);
        let mut first = vec![9u8, 9];
        first.push(2); // two metadata blocks
        first.extend_from_slice(&blocks[..7]);
        let mut second = blocks[7..].to_vec();
        second.extend_from_slice(&[7u8, 7]);

        let mut out = Vec::new();
        let mut title = None;
        parser.feed(
            &first,
            |bytes| out.extend_from_slice(bytes),
            |value| title = Some(value.to_string()),
        );
        parser.feed(
            &second,
            |bytes| out.extend_from_slice(bytes),
            |value| title = Some(value.to_string()),
        );
        assert_eq!(out, [9, 9, 7, 7]);
        assert_eq!(title.as_deref(), Some("X"));
    }

    #[test]
    fn streams_without_metaint_pass_bytes_through() {
        let mut parser = IcyParser::new(None);
        let mut out = Vec::new();
        parser.feed(b"plain mp3", |bytes| out.extend_from_slice(bytes), |_| {});
        assert_eq!(out, b"plain mp3");
    }

    #[test]
    fn a_dropped_connection_reconnects_and_keeps_metadata() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::mpsc;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let url = format!("http://{}/live.mp3", listener.local_addr().expect("addr"));
        let server = thread::spawn(move || {
            for (audio, title) in [([1u8, 2, 3, 4], "One"), ([5u8, 6, 7, 8], "Two")] {
                let (mut socket, _) = listener.accept().expect("accept");
                let mut request = Vec::new();
                let mut byte = [0u8; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    if socket.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    }
                    request.push(byte[0]);
                }
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nicy-metaint: 4\r\n\r\n",
                    )
                    .expect("headers");
                socket.write_all(&audio).expect("audio");
                let mut meta = [0u8; 32];
                let text = format!("StreamTitle='{title}';");
                meta[..text.len()].copy_from_slice(text.as_bytes());
                socket.write_all(&[2]).expect("length");
                socket.write_all(&meta).expect("metadata");
                socket.flush().expect("flush");
            }
        });

        let stream = HttpStream::start(url);
        let content_type = stream.wait_ready().expect("ready");
        assert_eq!(content_type.as_deref(), Some("audio/mpeg"));
        let mut reader = stream.reader();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut samples = Vec::new();
            while samples.len() < 8 {
                let mut buf = [0u8; 8];
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(count) => samples.extend_from_slice(&buf[..count]),
                    Err(error) => {
                        let _ = tx.send(format!("read error: {error}"));
                        return;
                    }
                }
            }
            let _ = tx.send(format!("samples: {samples:?}"));
        });
        let result = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("stream data within 10 s");
        assert!(result.starts_with("samples:"), "{result}");
        assert_eq!(result, "samples: [1, 2, 3, 4, 5, 6, 7, 8]");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut titles = Vec::new();
        while Instant::now() < deadline {
            for update in stream.take_updates() {
                if let StreamEvent::Title(Some(title)) = update {
                    titles.push(title);
                }
            }
            if titles.iter().any(|title| title == "One")
                && titles.iter().any(|title| title == "Two")
            {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(titles.iter().any(|title| title == "One"), "{titles:?}");
        assert!(titles.iter().any(|title| title == "Two"), "{titles:?}");
        server.join().expect("server thread");
    }
}
