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
const SEEK_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamMode {
    /// Endless stream: reconnect from the beginning, no seek.
    Live,
    /// Finite resource: detect Range support and seek by byte offset.
    OnDemand,
}

/// An HTTP stream with optional byte-range support.
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
    seek_signal: tokio::sync::Notify,
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
    finished: bool,
    generation: u64,
    connections: u32,
    mode: StreamMode,
    content_length: Option<u64>,
    seekable: bool,
    /// Byte offset of the next byte that will be handed to the decoder.
    delivered: u64,
    seek_request: Option<u64>,
    seek_result: Option<Result<u64, String>>,
}

impl HttpStream {
    pub fn start(url: String, mode: StreamMode) -> Self {
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
                finished: false,
                generation: 1,
                connections: 0,
                mode,
                content_length: None,
                seekable: false,
                delivered: 0,
                seek_request: None,
                seek_result: None,
            }),
            ready: Condvar::new(),
            updates: Mutex::new(VecDeque::new()),
            seek_signal: tokio::sync::Notify::new(),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let thread_shared = Arc::clone(&shared);
        let thread_stop = Arc::clone(&stop);
        if let Err(error) = thread::Builder::new()
            .name("musify-stream".into())
            .spawn(move || run_stream(url, thread_shared, thread_stop, mode))
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
                    "the stream did not send data within 15 seconds".into(),
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
                    "the stream did not send data within 15 seconds".into(),
                ));
            }
        }
    }

    pub fn generation(&self) -> u64 {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .generation
    }

    pub fn content_type(&self) -> Option<String> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .content_type
            .clone()
    }

    pub fn is_seekable(&self) -> bool {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .seekable
    }

    #[cfg(test)]
    pub fn content_length(&self) -> Option<u64> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .content_length
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
                state.delivered = state.delivered.saturating_add(count as u64);
                drop(state);
                self.shared.ready.notify_all();
                return Ok(count);
            }
            if let Some(error) = &state.first_error
                && !state.ready
            {
                return Err(io::Error::other(error.clone()));
            }
            if state.finished || state.stopped || self.stop.load(Ordering::Acquire) {
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
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.mode != StreamMode::OnDemand || !state.seekable {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this HTTP stream is not seekable",
            ));
        }
        let current = state.delivered;
        let target = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(delta) => current.saturating_add_signed(delta),
            SeekFrom::End(delta) => {
                let length = state.content_length.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::Unsupported, "unknown content length")
                })?;
                length.saturating_add_signed(delta)
            }
        };
        if state.content_length.is_some_and(|length| target > length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek position is outside the stream",
            ));
        }
        if target == current {
            return Ok(current);
        }
        state.seek_request = Some(target);
        state.seek_result = None;
        state.first_error = None;
        state.finished = false;
        state.buffer.clear();
        drop(state);
        self.shared.ready.notify_all();
        self.shared.seek_signal.notify_one();

        let deadline = Instant::now() + SEEK_TIMEOUT;
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(result) = state.seek_result.take() {
                return match result {
                    Ok(offset) => Ok(offset),
                    Err(error) => Err(io::Error::other(error)),
                };
            }
            if state.stopped || self.stop.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "the stream stopped during the seek",
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the HTTP range request timed out",
                ));
            }
            let (next, _) = self
                .shared
                .ready
                .wait_timeout(state, remaining)
                .unwrap_or_else(|p| p.into_inner());
            state = next;
        }
    }
}

impl MediaSource for HttpReader {
    fn is_seekable(&self) -> bool {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .seekable
    }

    fn byte_len(&self) -> Option<u64> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .content_length
    }
}

fn run_stream(url: String, shared: Arc<Shared>, stop: Arc<AtomicBool>, mode: StreamMode) {
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
            let pending_seek = shared
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .seek_request
                .is_some();
            let status = if pending_seek {
                StreamStatus::Buffering
            } else if attempt == 0 {
                StreamStatus::Connecting
            } else {
                StreamStatus::Reconnecting { attempt }
            };
            set_status(&shared, status);
            match read_once(&client, &url, &shared, &stop, mode).await {
                Ok(ReadOutcome::Seek) => {
                    attempt = 0;
                    continue;
                }
                Ok(ReadOutcome::Stopped) => {
                    mark_finished(&shared);
                    return;
                }
                Ok(ReadOutcome::Ended) => {
                    mark_finished(&shared);
                    // Keep the thread alive so a seek after EOF can reopen a
                    // Range request without creating a new stream.
                    loop {
                        if stop.load(Ordering::Acquire) {
                            return;
                        }
                        if shared
                            .state
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .seek_request
                            .is_some()
                        {
                            break;
                        }
                        let _ = tokio::time::timeout(
                            Duration::from_millis(500),
                            shared.seek_signal.notified(),
                        )
                        .await;
                    }
                    if let Ok(mut state) = shared.state.lock() {
                        state.finished = false;
                    }
                    attempt = 0;
                    continue;
                }
                Err(error) => {
                    let (seekable, delivered) = {
                        let state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                        (state.seekable, state.delivered)
                    };
                    if mode == StreamMode::OnDemand && !seekable && delivered > 0 {
                        // Without Range support a resumed byte offset cannot be
                        // satisfied; stop instead of silently replaying from 0.
                        set_status(&shared, StreamStatus::Error(error));
                        mark_finished(&shared);
                        return;
                    }
                    set_status(&shared, StreamStatus::Error(error));
                }
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

enum ReadOutcome {
    Seek,
    Stopped,
    Ended,
}

async fn read_once(
    client: &reqwest::Client,
    url: &str,
    shared: &Arc<Shared>,
    stop: &Arc<AtomicBool>,
    mode: StreamMode,
) -> Result<ReadOutcome, String> {
    let (start_offset, from_seek) = {
        let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        let from_seek = state.seek_request.is_some();
        let offset = if let Some(target) = state.seek_request.take() {
            target
        } else if mode == StreamMode::OnDemand && state.seekable && state.connections > 0 {
            state.delivered
        } else {
            0
        };
        (offset, from_seek)
    };

    let mut request = client.get(url).header(reqwest::header::ACCEPT, "audio/*");
    if mode == StreamMode::Live {
        request = request.header("Icy-MetaData", "1");
    }
    if start_offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={start_offset}-"));
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            if from_seek {
                let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                state.seek_request = Some(start_offset);
            }
            return Err(format!("cannot connect: {error}"));
        }
    };
    let status = response.status();
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
    let accepts_ranges = response
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("bytes"));
    let content_range = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_range);
    let mut content_length = response.content_length();
    let seekable;

    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        seekable = mode == StreamMode::OnDemand;
        if let Some((range_start, total)) = content_range {
            if start_offset > 0 && range_start != start_offset {
                if from_seek {
                    set_seek_result(
                        shared,
                        Err(format!(
                            "the server resumed at byte {range_start}, expected {start_offset}"
                        )),
                    );
                }
                return Ok(ReadOutcome::Stopped);
            }
            if let Some(total) = total {
                content_length = Some(total);
            }
        }
    } else if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        if from_seek {
            set_seek_result(shared, Err("the server rejected the range request".into()));
        }
        return Ok(ReadOutcome::Stopped);
    } else if status.is_success() {
        let mut effective_start = start_offset;
        if start_offset > 0 {
            // The server ignored Range: fall back to a full response only when
            // this was a reconnect; a user seek must fail explicitly.
            if from_seek {
                if let Ok(mut state) = shared.state.lock() {
                    state.seekable = false;
                }
                set_seek_result(
                    shared,
                    Err("the server does not support HTTP range requests".into()),
                );
                return Ok(ReadOutcome::Stopped);
            }
            effective_start = 0;
        }
        seekable = mode == StreamMode::OnDemand && accepts_ranges && content_length.is_some();
        if effective_start != start_offset {
            return read_body(
                response,
                shared,
                stop,
                mode,
                effective_start,
                content_length,
                seekable,
                content_type,
                metaint,
                from_seek,
            )
            .await;
        }
    } else {
        return Err(format!("the stream answered HTTP {status}"));
    }

    read_body(
        response,
        shared,
        stop,
        mode,
        start_offset,
        content_length,
        seekable,
        content_type,
        metaint,
        from_seek,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn read_body(
    response: reqwest::Response,
    shared: &Arc<Shared>,
    stop: &Arc<AtomicBool>,
    mode: StreamMode,
    start_offset: u64,
    content_length: Option<u64>,
    seekable: bool,
    content_type: Option<String>,
    metaint: Option<usize>,
    from_seek: bool,
) -> Result<ReadOutcome, String> {
    {
        let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.content_type.clone_from(&content_type);
        state.content_length = content_length;
        state.seekable = seekable;
        state.finished = false;
        state.connections = state.connections.saturating_add(1);
        state.delivered = start_offset;
        if from_seek {
            state.seek_request = None;
            state.seek_result = Some(Ok(start_offset));
            state.buffer.clear();
        } else if state.connections > 1 {
            // A new connection restarts the transport. The decoder must not
            // parse the new stream as a continuation of the old one; clearing
            // the buffer and bumping the generation makes it reopen.
            state.buffer.clear();
            state.generation = state.generation.wrapping_add(1);
        }
    }
    shared.ready.notify_all();
    set_status(shared, StreamStatus::Buffering);

    let mut parser = IcyParser::new(if mode == StreamMode::Live {
        metaint
    } else {
        None
    });
    let mut stream = response.bytes_stream();
    let mut received = 0u64;
    loop {
        if stop.load(Ordering::Acquire) {
            return Ok(ReadOutcome::Stopped);
        }
        if shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .seek_request
            .is_some()
        {
            return Ok(ReadOutcome::Seek);
        }
        let chunk = match tokio::time::timeout(Duration::from_millis(500), stream.next()).await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => {
                if mode == StreamMode::OnDemand
                    && content_length.is_some_and(|length| start_offset + received >= length)
                {
                    return Ok(ReadOutcome::Ended);
                }
                return Err("the server closed the stream".into());
            }
            Err(_) => continue,
        };
        if stop.load(Ordering::Acquire) {
            return Ok(ReadOutcome::Stopped);
        }
        let chunk = chunk.map_err(|error| format!("network read failed: {error}"))?;
        received = received.saturating_add(chunk.len() as u64);
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
        if shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .seek_request
            .is_some()
        {
            return Ok(ReadOutcome::Seek);
        }
        if mode == StreamMode::OnDemand
            && content_length.is_some_and(|length| start_offset + received >= length)
        {
            return Ok(ReadOutcome::Ended);
        }
    }
}

fn push_audio(shared: &Arc<Shared>, stop: &Arc<AtomicBool>, bytes: &[u8]) -> Result<(), String> {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    let mut offset = 0;
    while offset < bytes.len() {
        if stop.load(Ordering::Acquire) || state.stopped || state.seek_request.is_some() {
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

fn mark_finished(shared: &Arc<Shared>) {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    state.finished = true;
    shared.ready.notify_all();
}

fn parse_content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let value = value.trim();
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, _end) = range.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let total = total.trim().parse().ok();
    Some((start, total))
}

fn set_seek_result(shared: &Arc<Shared>, result: Result<u64, String>) {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    state.seek_request = None;
    state.seek_result = Some(result);
    shared.ready.notify_all();
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

        let stream = HttpStream::start(url, StreamMode::Live);
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
        assert!(
            stream.generation() >= 2,
            "the stream should have reconnected"
        );
        server.join().expect("server thread");
    }

    struct RangeServer {
        address: std::net::SocketAddr,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl RangeServer {
        fn start(body: Vec<u8>, ranges: bool, close_after: Option<usize>) -> Self {
            use std::net::TcpListener;
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind range server");
            listener
                .set_nonblocking(true)
                .expect("nonblocking listener");
            let address = listener.local_addr().expect("addr");
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let thread = thread::spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    let (mut socket, _) = match listener.accept() {
                        Ok(connection) => connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(_) => return,
                    };
                    let _ = socket.set_nonblocking(false);
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while !request.ends_with(b"\r\n\r\n") {
                        use std::io::Read;
                        if socket.read(&mut byte).unwrap_or(0) == 0 {
                            break;
                        }
                        request.push(byte[0]);
                    }
                    let text = String::from_utf8_lossy(&request);
                    let start = text
                        .lines()
                        .find_map(|line| {
                            let line = line.to_ascii_lowercase();
                            line.strip_prefix("range: bytes=")
                                .and_then(|value| value.split('-').next())
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    use std::io::Write;
                    if ranges && start > 0 {
                        if start >= body.len() {
                            socket
                                .write_all(
                                    b"HTTP/1.1 416 Range Not Satisfiable\r\n\
                                      Content-Range: bytes */0\r\n\
                                      Content-Length: 0\r\n\r\n",
                                )
                                .expect("416");
                            continue;
                        }
                        let header = format!(
                            "HTTP/1.1 206 Partial Content\r\n\
                             Content-Type: audio/mpeg\r\n\
                             Accept-Ranges: bytes\r\n\
                             Content-Range: bytes {start}-{}/{}\r\n\
                             Content-Length: {}\r\n\r\n",
                            body.len() - 1,
                            body.len(),
                            body.len() - start
                        );
                        socket.write_all(header.as_bytes()).expect("headers");
                        let chunk = &body[start..];
                        let count = close_after.unwrap_or(chunk.len()).min(chunk.len());
                        socket.write_all(&chunk[..count]).expect("body");
                    } else {
                        let header = format!(
                            "HTTP/1.1 200 OK\r\n\
                             Content-Type: audio/mpeg\r\n\
                             Accept-Ranges: {}\r\n\
                             Content-Length: {}\r\n\r\n",
                            if ranges { "bytes" } else { "none" },
                            body.len()
                        );
                        socket.write_all(header.as_bytes()).expect("headers");
                        let count = close_after.unwrap_or(body.len()).min(body.len());
                        socket.write_all(&body[..count]).expect("body");
                    }
                    socket.flush().expect("flush");
                    drop(socket);
                }
            });
            Self {
                address,
                stop,
                thread: Some(thread),
            }
        }

        fn url(&self) -> String {
            format!("http://{}/episode.mp3", self.address)
        }

        fn join(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn read_exact_len(reader: &mut HttpReader, len: usize, timeout: Duration) -> Vec<u8> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        while out.len() < len && Instant::now() < deadline {
            let mut buffer = [0u8; 64];
            let remaining = (len - out.len()).min(buffer.len());
            match reader.read(&mut buffer[..remaining]) {
                Ok(0) => break,
                Ok(count) => out.extend_from_slice(&buffer[..count]),
                Err(error) => panic!("read failed: {error}"),
            }
        }
        out
    }

    #[test]
    fn range_requests_resume_from_the_byte_offset() {
        let body: Vec<u8> = (0..1024).map(|value| (value % 251) as u8).collect();
        let mut server = RangeServer::start(body.clone(), true, None);
        let stream = HttpStream::start(server.url(), StreamMode::OnDemand);
        stream.wait_ready().expect("ready");
        assert!(stream.is_seekable());
        assert_eq!(stream.content_length(), Some(body.len() as u64));

        let mut reader = stream.reader();
        let first = read_exact_len(&mut reader, 64, Duration::from_secs(5));
        assert_eq!(first, body[..64]);
        let offset = reader.seek(SeekFrom::Start(400)).expect("seek");
        assert_eq!(offset, 400);
        let rest = read_exact_len(&mut reader, 64, Duration::from_secs(5));
        assert_eq!(rest, body[400..464]);
        drop(stream);
        server.join();
    }

    #[test]
    fn a_server_without_range_support_stays_playable_but_not_seekable() {
        let body: Vec<u8> = (0..512).map(|value| (value % 97) as u8).collect();
        let mut server = RangeServer::start(body.clone(), false, None);
        let stream = HttpStream::start(server.url(), StreamMode::OnDemand);
        stream.wait_ready().expect("ready");
        assert!(!stream.is_seekable());
        let mut reader = stream.reader();
        let samples = read_exact_len(&mut reader, 64, Duration::from_secs(5));
        assert_eq!(samples, body[..64]);
        let error = reader
            .seek(SeekFrom::Start(100))
            .expect_err("non-seekable stream");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        drop(stream);
        server.join();
    }

    #[test]
    fn a_range_request_rejected_with_416_fails_the_seek() {
        let body: Vec<u8> = (0..256).map(|value| value as u8).collect();
        let mut server = RangeServer::start(body.clone(), true, None);
        let stream = HttpStream::start(server.url(), StreamMode::OnDemand);
        stream.wait_ready().expect("ready");
        let mut reader = stream.reader();
        let _ = read_exact_len(&mut reader, 16, Duration::from_secs(5));
        let error = reader
            .seek(SeekFrom::Start(body.len() as u64))
            .expect_err("out of range seek");
        assert!(error.to_string().contains("range"));
        drop(stream);
        server.join();
    }

    #[test]
    fn a_dropped_range_stream_reconnects_from_the_delivered_offset() {
        let body: Vec<u8> = (0..1024).map(|value| (value % 233) as u8).collect();
        let mut server = RangeServer::start(body.clone(), true, Some(200));
        let stream = HttpStream::start(server.url(), StreamMode::OnDemand);
        stream.wait_ready().expect("ready");
        let mut reader = stream.reader();
        let first = read_exact_len(&mut reader, 100, Duration::from_secs(5));
        assert_eq!(first, body[..100]);
        let rest = read_exact_len(&mut reader, 300, Duration::from_secs(10));
        assert_eq!(rest, body[100..400]);
        assert!(stream.generation() >= 2, "range reconnect expected");
        drop(stream);
        server.join();
    }
}
