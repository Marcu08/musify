use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use musify_core::{
    Command, Event, PlaybackState, QueueItem, QueueItemId, RepeatMode, StreamStatus, Track,
};
use rtrb::Producer;

use crate::clock::AudioClock;
use crate::decoder::TrackDecoder;
use crate::error::AudioError;
use crate::output::{self, OutputGuard, OutputInfo};
use crate::queue::PlaybackQueue;

/// A consistent view of the engine for status displays and tests.
#[derive(Debug, Clone, Default)]
pub struct PlaybackSnapshot {
    pub state: PlaybackState,
    pub current: Option<QueueItem>,
    pub queue: Vec<QueueItem>,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: u16,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub output_sample_rate: u32,
    pub output_channels: u16,
    pub underruns: u64,
}

/// Handle to the audio engine. Dropping it stops the worker and the output stream.
pub struct AudioEngine {
    commands: Sender<Command>,
    events: Receiver<Event>,
    clock: Arc<AudioClock>,
    shared: Arc<Mutex<PlaybackSnapshot>>,
    worker: Option<JoinHandle<()>>,
    output: Option<OutputGuard>,
    info: OutputInfo,
}

impl AudioEngine {
    pub fn start(volume: u16) -> Result<Self, AudioError> {
        let volume = volume.min(100);
        let clock = Arc::new(AudioClock::new(volume));
        let (output, producer, info) = output::start(Arc::clone(&clock))?;
        let (command_tx, command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let shared = Arc::new(Mutex::new(PlaybackSnapshot {
            volume,
            output_sample_rate: info.sample_rate,
            output_channels: info.channels,
            ..PlaybackSnapshot::default()
        }));
        let shared_worker = Arc::clone(&shared);
        let clock_worker = Arc::clone(&clock);
        let worker = thread::Builder::new()
            .name("musify-audio-engine".into())
            .spawn(move || {
                EngineThread::new(
                    producer,
                    info,
                    clock_worker,
                    event_tx,
                    shared_worker,
                    volume,
                )
                .run(command_rx);
            })
            .map_err(|error| AudioError::Output(error.to_string()))?;

        Ok(Self {
            commands: command_tx,
            events: event_rx,
            clock,
            shared,
            worker: Some(worker),
            output: Some(output),
            info,
        })
    }

    pub fn send(&self, command: Command) -> Result<(), AudioError> {
        self.commands
            .send(command)
            .map_err(|_| AudioError::EngineStopped)
    }

    /// A clone of the command sender, for OS integrations that must send
    /// media-key commands without owning the engine handle.
    pub fn command_sender(&self) -> Sender<Command> {
        self.commands.clone()
    }

    pub fn try_recv_event(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// A clone of the event receiver, for a bridge thread that forwards engine
    /// events to the UI and wakes it.
    pub fn event_receiver(&self) -> Receiver<Event> {
        self.events.clone()
    }

    pub fn recv_event(&self) -> Option<Event> {
        self.events.recv().ok()
    }

    pub fn recv_event_timeout(&self, timeout: Duration) -> Option<Event> {
        self.events.recv_timeout(timeout).ok()
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        let mut snapshot = match self.shared.lock() {
            Ok(shared) => shared.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        snapshot.underruns = self.clock.underruns();
        snapshot
    }

    pub fn output_info(&self) -> OutputInfo {
        self.info
    }

    pub fn underruns(&self) -> u64 {
        self.clock.underruns()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        drop(self.output.take());
    }
}

#[derive(Debug, Clone)]
struct Boundary {
    at_frame: u64,
    queue_index: usize,
    item: QueueItem,
    seek_offset: Duration,
}

struct EngineThread {
    producer: Producer<f32>,
    info: OutputInfo,
    clock: Arc<AudioClock>,
    events: Sender<Event>,
    shared: Arc<Mutex<PlaybackSnapshot>>,
    queue: PlaybackQueue,
    decoder: Option<TrackDecoder>,
    current: Option<QueueItem>,
    current_index: Option<usize>,
    state: PlaybackState,
    volume: u16,
    next_item_id: u64,
    decoded_frames: u64,
    boundaries: VecDeque<Boundary>,
    current_origin: u64,
    seek_offset: Duration,
    end_frame: Option<u64>,
    decode_buf: Vec<f32>,
    decode_pos: usize,
    last_position_event: Instant,
    queue_dirty: bool,
}

impl EngineThread {
    fn new(
        producer: Producer<f32>,
        info: OutputInfo,
        clock: Arc<AudioClock>,
        events: Sender<Event>,
        shared: Arc<Mutex<PlaybackSnapshot>>,
        volume: u16,
    ) -> Self {
        Self {
            producer,
            info,
            clock,
            events,
            shared,
            queue: PlaybackQueue::new(),
            decoder: None,
            current: None,
            current_index: None,
            state: PlaybackState::Stopped,
            volume,
            next_item_id: 1,
            decoded_frames: 0,
            boundaries: VecDeque::new(),
            current_origin: 0,
            seek_offset: Duration::ZERO,
            end_frame: None,
            decode_buf: Vec::new(),
            decode_pos: 0,
            last_position_event: Instant::now(),
            queue_dirty: true,
        }
    }

    fn run(mut self, commands: Receiver<Command>) {
        loop {
            let command = if self.state == PlaybackState::Playing {
                match commands.recv_timeout(Duration::from_millis(25)) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match commands.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            };

            if let Some(command) = command
                && !self.handle(command)
            {
                break;
            }

            if self.state == PlaybackState::Playing {
                self.pump_audio();
                self.update_boundaries();
                self.check_end();
                self.tick_position();
            }
            self.drain_stream_updates();
            self.publish();
        }
        self.shutdown_engine();
    }

    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::PlayTrack(track) => self.start_tracks(vec![*track], 0),
            Command::PlayTracks {
                tracks,
                start_index,
            } => self.start_tracks(tracks, start_index),
            Command::RestoreQueue {
                tracks,
                start_index,
                position,
            } => self.start_tracks_internal(tracks, start_index, false, position),
            Command::Enqueue(tracks) => {
                let items: Vec<QueueItem> = tracks.into_iter().map(|t| self.item(t)).collect();
                self.queue.enqueue(items);
                self.emit_queue();
            }
            Command::Pause => {
                if self.state == PlaybackState::Playing {
                    self.set_state(PlaybackState::Paused);
                }
            }
            Command::Resume => {
                if self.state == PlaybackState::Paused && self.decoder.is_some() {
                    self.set_state(PlaybackState::Playing);
                }
            }
            Command::TogglePlayPause => match self.state {
                PlaybackState::Playing => self.set_state(PlaybackState::Paused),
                PlaybackState::Paused => {
                    if self.decoder.is_some() {
                        self.set_state(PlaybackState::Playing);
                    }
                }
                PlaybackState::Stopped => {
                    if !self.queue.is_empty() {
                        self.restart_current();
                    }
                }
            },
            Command::Stop => self.stop_playback(),
            Command::Next => self.skip_next(),
            Command::Previous => self.previous(),
            Command::Seek(position) => self.seek(position),
            Command::SetVolume(volume) => self.set_volume(volume),
            Command::SetShuffle(shuffle) => {
                let audible = self.current_index;
                let was_active = self.state != PlaybackState::Stopped && audible.is_some();
                let position = self.position();
                self.queue.set_shuffle(shuffle);
                if let Some(audible) = audible {
                    self.queue.select(audible);
                    if was_active {
                        self.restart_current_at(position);
                    }
                }
                self.emit(Event::ShuffleChanged(shuffle));
                self.emit_queue();
            }
            Command::SetRepeat(repeat) => {
                self.queue.set_repeat(repeat);
                self.emit(Event::RepeatChanged(repeat));
            }
            Command::MoveQueueItem { from, to } => self.move_item(from, to),
            Command::RemoveQueueItem { index } => self.remove_item(index),
            Command::ClearQueue => self.clear_queue(),
            Command::SetMusicFolders(_) | Command::ScanLibrary => {}
            Command::Shutdown => {
                self.shutdown_engine();
                return false;
            }
        }
        true
    }

    fn item(&mut self, track: Track) -> QueueItem {
        let id = QueueItemId::new(self.next_item_id);
        self.next_item_id = self.next_item_id.wrapping_add(1);
        QueueItem { id, track }
    }

    fn start_tracks(&mut self, tracks: Vec<Track>, start_index: usize) {
        self.start_tracks_internal(tracks, start_index, true, Duration::ZERO);
    }

    fn start_tracks_internal(
        &mut self,
        tracks: Vec<Track>,
        start_index: usize,
        autoplay: bool,
        position: Duration,
    ) {
        self.flush_stream(false);
        self.decoder = None;
        self.current = None;
        self.current_index = None;
        self.boundaries.clear();
        self.end_frame = None;
        let items: Vec<QueueItem> = tracks.into_iter().map(|track| self.item(track)).collect();
        let shuffle = self.queue.shuffle();
        self.queue.set_tracks(items, start_index, shuffle);
        self.emit_queue();
        if self.queue.is_empty() {
            self.set_state(PlaybackState::Stopped);
            self.emit(Event::TrackChanged(None));
            self.publish();
            return;
        }
        if self.open_current(0) {
            self.prefill();
            if autoplay {
                self.set_state(PlaybackState::Playing);
            } else {
                self.set_state(PlaybackState::Paused);
                if !position.is_zero() {
                    self.seek(position);
                }
            }
        } else {
            self.set_state(PlaybackState::Stopped);
            self.emit(Event::TrackChanged(None));
        }
        self.publish();
    }

    fn restart_current(&mut self) {
        self.flush_stream(false);
        self.decoder = None;
        self.boundaries.clear();
        self.end_frame = None;
        if self.open_current(0) {
            self.prefill();
            self.set_state(PlaybackState::Playing);
        } else {
            self.set_state(PlaybackState::Stopped);
        }
        self.publish();
    }

    fn open_current(&mut self, attempt: usize) -> bool {
        let Some(item) = self.queue.current().cloned() else {
            return false;
        };
        let Some(index) = self.queue.current_index() else {
            return false;
        };
        match self.open_track(item) {
            Ok((decoder, item)) => {
                self.current = Some(item.clone());
                self.current_index = Some(index);
                self.current_origin = self.decoded_frames;
                self.seek_offset = Duration::ZERO;
                let stream = !item.track.is_local();
                self.decoder = Some(decoder);
                self.emit(Event::TrackChanged(Some(item)));
                self.emit(Event::StreamTitleChanged(None));
                self.emit(Event::StreamStatusChanged(if stream {
                    StreamStatus::Connecting
                } else {
                    StreamStatus::Idle
                }));
                self.emit_position();
                true
            }
            Err(error) => {
                self.emit_error("apertura brano", error.to_string());
                if attempt < self.queue.items().len().saturating_sub(1)
                    && self.queue.advance(true).is_some()
                {
                    return self.open_current(attempt + 1);
                }
                false
            }
        }
    }

    fn open_track(&self, mut item: QueueItem) -> Result<(TrackDecoder, QueueItem), AudioError> {
        let decoder = if item.track.is_local() {
            TrackDecoder::open(&item.track.path, self.info.sample_rate)?
        } else {
            TrackDecoder::open_stream(&item.track.source, self.info.sample_rate)?
        };
        if item.track.duration.is_zero() {
            item.track.duration = decoder.duration().unwrap_or_default();
        }
        Ok((decoder, item))
    }

    fn advance_auto(&mut self) {
        self.decoder = None;
        let repeat_one = self.queue.repeat() == RepeatMode::One;
        let max_attempts = self.queue.items().len().max(1);
        for attempt in 0..max_attempts {
            let index = if repeat_one && attempt == 0 {
                self.queue.current_index()
            } else {
                self.queue.advance(false)
            };
            let Some(index) = index else { break };
            let Some(item) = self.queue.items().get(index).cloned() else {
                continue;
            };
            match self.open_track(item) {
                Ok((decoder, item)) => {
                    self.boundaries.push_back(Boundary {
                        at_frame: self.decoded_frames,
                        queue_index: index,
                        item,
                        seek_offset: Duration::ZERO,
                    });
                    self.decoder = Some(decoder);
                    self.end_frame = None;
                    return;
                }
                Err(error) => {
                    self.emit_error("apertura brano successivo", error.to_string());
                }
            }
        }
        self.end_frame = Some(self.decoded_frames);
    }

    fn pump_audio(&mut self) {
        while self.state == PlaybackState::Playing {
            if self.decode_pos >= self.decode_buf.len() {
                if self.decoder.is_none() {
                    break;
                }
                self.decode_buf.clear();
                self.decode_pos = 0;
                let Some(decoder) = self.decoder.as_mut() else {
                    break;
                };
                match decoder.decode_into(&mut self.decode_buf, 1024) {
                    Ok(0) => {
                        self.advance_auto();
                        continue;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        self.emit_error("decodifica", error.to_string());
                        self.advance_auto();
                        continue;
                    }
                }
            }

            let mut pushed = false;
            while self.decode_pos + 1 < self.decode_buf.len() {
                if self.producer.slots() < 2 {
                    break;
                }
                let left = self.decode_buf[self.decode_pos];
                let right = self.decode_buf[self.decode_pos + 1];
                if self.producer.push(left).is_err() || self.producer.push(right).is_err() {
                    break;
                }
                self.decode_pos += 2;
                self.decoded_frames += 1;
                pushed = true;
            }
            if !pushed || self.decode_pos < self.decode_buf.len() {
                break;
            }
        }
    }

    fn prefill(&mut self) {
        let previous = self.state;
        self.state = PlaybackState::Playing;
        self.pump_audio();
        self.state = previous;
    }

    fn update_boundaries(&mut self) {
        let total = self.clock.total_played_frames();
        while self
            .boundaries
            .front()
            .is_some_and(|boundary| total >= boundary.at_frame)
        {
            let Some(boundary) = self.boundaries.pop_front() else {
                break;
            };
            self.current = Some(boundary.item.clone());
            self.current_index = Some(boundary.queue_index);
            self.current_origin = boundary.at_frame;
            self.seek_offset = boundary.seek_offset;
            self.emit(Event::TrackChanged(Some(boundary.item)));
            self.emit_position();
        }
    }

    fn check_end(&mut self) {
        let Some(end) = self.end_frame else {
            return;
        };
        if self.clock.total_played_frames() >= end && self.state == PlaybackState::Playing {
            self.end_frame = None;
            self.set_state(PlaybackState::Stopped);
            self.tick_position_now();
        }
    }

    fn tick_position(&mut self) {
        if self.state != PlaybackState::Playing {
            return;
        }
        if self.last_position_event.elapsed() < Duration::from_millis(200) {
            return;
        }
        self.last_position_event = Instant::now();
        self.emit_position();
    }

    fn tick_position_now(&mut self) {
        self.last_position_event = Instant::now();
        self.emit_position();
    }

    fn drain_stream_updates(&mut self) {
        let is_stream = self
            .current
            .as_ref()
            .is_some_and(|item| !item.track.is_local());
        if !is_stream {
            return;
        }
        let Some(decoder) = self.decoder.as_ref() else {
            return;
        };
        for update in decoder.take_stream_updates() {
            match update {
                musify_core::StreamEvent::Status(status) => {
                    tracing::info!(?status, "stato stream");
                    self.emit(Event::StreamStatusChanged(status));
                }
                musify_core::StreamEvent::Title(title) => {
                    tracing::info!(?title, "titolo stream");
                    self.emit(Event::StreamTitleChanged(title));
                }
            }
        }
    }

    fn emit_position(&mut self) {
        let position = self.position();
        let duration = self
            .current
            .as_ref()
            .and_then(|item| (!item.track.duration.is_zero()).then_some(item.track.duration));
        self.emit(Event::PositionChanged { position, duration });
    }

    fn position(&self) -> Duration {
        let total = self.clock.total_played_frames();
        let frames = total.saturating_sub(self.current_origin);
        let position = self.seek_offset + frames_to_duration(frames, self.info.sample_rate);
        match self.current.as_ref() {
            Some(item) if !item.track.duration.is_zero() => position.min(item.track.duration),
            _ => position,
        }
    }

    fn seek(&mut self, position: Duration) {
        if self.decoder.is_none() || self.current.is_none() {
            return;
        }
        if self
            .current
            .as_ref()
            .is_some_and(|item| !item.track.is_local())
        {
            // Live streams have no timeline to seek.
            return;
        }
        let duration = self.current.as_ref().map(|item| item.track.duration);
        let position = duration.map_or(position, |duration| position.min(duration));
        let was_playing = self.state == PlaybackState::Playing;
        self.flush_stream(false);
        self.boundaries.clear();
        self.end_frame = None;
        self.current_origin = 0;
        let Some(decoder) = self.decoder.as_mut() else {
            return;
        };
        match decoder.seek(position) {
            Ok(actual) => {
                self.seek_offset = actual;
                self.prefill();
                if was_playing {
                    self.set_state(PlaybackState::Playing);
                } else {
                    self.set_state(PlaybackState::Paused);
                }
                self.emit_position();
            }
            Err(error) => {
                self.emit_error("seek", error.to_string());
                self.seek_offset = Duration::ZERO;
                if was_playing {
                    self.set_state(PlaybackState::Playing);
                }
            }
        }
        self.publish();
    }

    fn skip_next(&mut self) {
        let was_playing = self.state == PlaybackState::Playing;
        self.flush_stream(false);
        self.decoder = None;
        self.boundaries.clear();
        self.end_frame = None;
        self.current = None;
        self.current_index = None;
        if self.queue.advance(true).is_some() && self.open_current(0) {
            self.prefill();
            if was_playing {
                self.set_state(PlaybackState::Playing);
            } else {
                self.set_state(PlaybackState::Paused);
            }
        } else {
            self.set_state(PlaybackState::Stopped);
            self.emit(Event::TrackChanged(None));
        }
        self.publish();
    }

    fn previous(&mut self) {
        if self.position() > Duration::from_secs(3) {
            self.seek(Duration::ZERO);
            return;
        }
        let was_playing = self.state == PlaybackState::Playing;
        self.flush_stream(false);
        self.decoder = None;
        self.boundaries.clear();
        self.end_frame = None;
        self.current = None;
        self.current_index = None;
        if self.queue.previous().is_some() || self.queue.current_index().is_some() {
            if self.open_current(0) {
                self.prefill();
                if was_playing {
                    self.set_state(PlaybackState::Playing);
                } else {
                    self.set_state(PlaybackState::Paused);
                }
            } else {
                self.set_state(PlaybackState::Stopped);
                self.emit(Event::TrackChanged(None));
            }
        } else {
            self.set_state(PlaybackState::Stopped);
            self.emit(Event::TrackChanged(None));
        }
        self.publish();
    }

    fn stop_playback(&mut self) {
        self.flush_stream(false);
        self.decoder = None;
        self.current = None;
        self.current_index = None;
        self.boundaries.clear();
        self.end_frame = None;
        self.queue.clear();
        self.set_state(PlaybackState::Stopped);
        self.emit(Event::TrackChanged(None));
        self.emit_queue();
        self.publish();
    }

    fn clear_queue(&mut self) {
        self.queue.clear();
        self.boundaries.clear();
        self.emit_queue();
        if self.state != PlaybackState::Stopped {
            self.stop_playback();
        } else {
            self.current = None;
            self.current_index = None;
            self.emit(Event::TrackChanged(None));
        }
    }

    fn restart_current_at(&mut self, position: Duration) {
        let was_playing = self.state == PlaybackState::Playing;
        self.flush_stream(false);
        self.decoder = None;
        self.boundaries.clear();
        self.end_frame = None;
        self.current = None;
        self.current_index = None;
        if self.queue.current_index().is_none() {
            self.set_state(PlaybackState::Stopped);
            self.emit(Event::TrackChanged(None));
            return;
        }
        if !self.open_current(0) {
            self.set_state(PlaybackState::Stopped);
            self.emit(Event::TrackChanged(None));
            return;
        }
        if was_playing {
            self.set_state(PlaybackState::Playing);
        } else {
            self.set_state(PlaybackState::Paused);
        }
        if position.is_zero() {
            self.prefill();
        } else {
            self.seek(position);
        }
        self.publish();
    }

    fn move_item(&mut self, from: usize, to: usize) {
        let audible = self.current_index;
        let was_active = self.state != PlaybackState::Stopped && audible.is_some();
        let position = self.position();
        self.queue.move_item(from, to);
        for boundary in &mut self.boundaries {
            boundary.queue_index = remap_index(boundary.queue_index, from, to);
        }
        if let Some(index) = audible {
            let moved = remap_index(index, from, to);
            self.current_index = Some(moved);
            self.queue.select(moved);
            if was_active {
                self.restart_current_at(position);
            }
        }
        self.emit_queue();
    }

    fn remove_item(&mut self, index: usize) {
        let audible = self.current_index;
        let audible_removed = audible == Some(index);
        let was_active = self.state != PlaybackState::Stopped && audible.is_some();
        let position = self.position();
        self.boundaries
            .retain(|boundary| boundary.queue_index != index);
        for boundary in &mut self.boundaries {
            if boundary.queue_index > index {
                boundary.queue_index -= 1;
            }
        }
        self.queue.remove(index);
        if self.queue.is_empty() {
            self.stop_playback();
            return;
        }
        let selected = if audible_removed {
            // The cursor may already point at a prefetched track further ahead.
            // The first boundary is the one that would have played next.
            self.boundaries
                .front()
                .map(|boundary| boundary.queue_index)
                .or_else(|| self.queue.current_index())
        } else {
            audible.map(|current| {
                if current > index {
                    current - 1
                } else {
                    current
                }
            })
        };
        if let Some(selected) = selected {
            self.current_index = Some(selected);
            self.queue.select(selected);
        }
        if was_active {
            self.restart_current_at(if audible_removed {
                Duration::ZERO
            } else {
                position
            });
        } else if audible_removed {
            self.current_index = None;
        }
        self.emit_queue();
    }

    fn set_volume(&mut self, volume: u16) {
        self.volume = volume.min(100);
        self.clock.volume.store(self.volume, Ordering::Relaxed);
        self.emit(Event::VolumeChanged(self.volume));
        self.publish();
    }

    fn set_state(&mut self, state: PlaybackState) {
        let changed = self.state != state;
        self.state = state;
        self.clock
            .playing
            .store(state == PlaybackState::Playing, Ordering::Release);
        if changed {
            self.emit(Event::PlaybackStateChanged(state));
            self.publish();
        }
    }

    fn flush_stream(&mut self, play_after: bool) {
        self.clock.playing.store(false, Ordering::Release);
        let target = self.clock.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let start = Instant::now();
        while self.clock.applied_epoch.load(Ordering::Acquire) != target {
            if start.elapsed() > Duration::from_millis(500) {
                tracing::warn!("audio callback did not acknowledge a flush in 500 ms");
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.decoded_frames = 0;
        self.decode_buf.clear();
        self.decode_pos = 0;
        self.boundaries.clear();
        self.current_origin = 0;
        self.seek_offset = Duration::ZERO;
        self.end_frame = None;
        self.clock.total_played_frames.store(0, Ordering::Release);
        self.clock.playing.store(play_after, Ordering::Release);
    }

    fn shutdown_engine(&mut self) {
        self.clock.playing.store(false, Ordering::Release);
        self.emit(Event::Shutdown);
        self.publish();
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    fn emit_error(&self, context: &str, message: String) {
        self.emit(Event::Error {
            context: context.into(),
            message,
        });
    }

    fn emit_queue(&mut self) {
        self.queue_dirty = true;
        let queue = self.queue.items().to_vec();
        self.emit(Event::QueueChanged(queue));
    }

    fn publish(&mut self) {
        let position = self.position();
        let duration = self
            .current
            .as_ref()
            .and_then(|item| (!item.track.duration.is_zero()).then_some(item.track.duration));
        let mut shared = match self.shared.lock() {
            Ok(shared) => shared,
            Err(poisoned) => poisoned.into_inner(),
        };
        shared.state = self.state;
        shared.current.clone_from(&self.current);
        if self.queue_dirty {
            shared.queue = self.queue.items().to_vec();
            self.queue_dirty = false;
        }
        shared.position = position;
        shared.duration = duration;
        shared.volume = self.volume;
        shared.shuffle = self.queue.shuffle();
        shared.repeat = self.queue.repeat();
        shared.output_sample_rate = self.info.sample_rate;
        shared.output_channels = self.info.channels;
        shared.underruns = self.clock.underruns();
    }
}

fn frames_to_duration(frames: u64, sample_rate: u32) -> Duration {
    if sample_rate == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f64(frames as f64 / f64::from(sample_rate))
}

fn remap_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < index && index <= to {
        index - 1
    } else if to <= index && index < from {
        index + 1
    } else {
        index
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::thread::{self, JoinHandle};

    use crossbeam_channel::unbounded;
    use musify_core::TrackId;
    use rtrb::{Consumer, RingBuffer};

    use super::*;
    use crate::clock::AudioClock;

    const SAMPLE_RATE: u32 = 48_000;
    const RING_FRAMES: usize = 4_096;

    fn write_constant_wav(path: &Path, sample: f32) {
        let channels = 2u16;
        let frames = SAMPLE_RATE;
        let value = (sample * f32::from(i16::MAX)) as i16;
        let data_len = frames * u32::from(channels) * 2;
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        bytes.extend_from_slice(&(SAMPLE_RATE * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for _ in 0..frames * u32::from(channels) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        fs::write(path, bytes).expect("write test wav");
    }

    fn track(id: i64, path: &Path) -> Track {
        Track {
            id: TrackId::new(id),
            source: musify_core::SourceId::local(path),
            path: path.to_path_buf(),
            title: format!("Track {id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            album_artist: None,
            track_number: None,
            disc_number: None,
            duration: Duration::from_secs(1),
            year: None,
            genre: None,
            cover_path: None,
            added_at: 0,
        }
    }

    /// Stands in for the real-time callback: drains the ring when the engine
    /// asks for a flush (epoch change), and drains a requested number of frames
    /// so a test can make room for prefetched audio.
    struct FakeCallback {
        drain_requests: mpsc::Sender<usize>,
        drain_done: mpsc::Receiver<()>,
        stop: Arc<AtomicBool>,
        handle: Option<JoinHandle<Consumer<f32>>>,
    }

    impl FakeCallback {
        fn start(mut consumer: Consumer<f32>, clock: Arc<AudioClock>) -> Self {
            let (drain_requests, request_rx) = mpsc::channel::<usize>();
            let (done_tx, drain_done) = mpsc::channel::<()>();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = Arc::clone(&stop);
            let handle = thread::spawn(move || {
                while !stop_thread.load(Ordering::Relaxed) {
                    while let Ok(frames) = request_rx.try_recv() {
                        for _ in 0..frames * 2 {
                            if consumer.pop().is_err() {
                                break;
                            }
                        }
                        let _ = done_tx.send(());
                    }
                    let epoch = clock.epoch.load(Ordering::Acquire);
                    if clock.applied_epoch.load(Ordering::Acquire) != epoch {
                        while consumer.pop().is_ok() {}
                        clock.applied_epoch.store(epoch, Ordering::Release);
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                consumer
            });
            Self {
                drain_requests,
                drain_done,
                stop,
                handle: Some(handle),
            }
        }

        fn drain(&self, frames: usize) {
            self.drain_requests.send(frames).expect("callback alive");
            self.drain_done.recv().expect("drain acknowledged");
        }

        fn stop(mut self) -> Consumer<f32> {
            self.stop.store(true, Ordering::Relaxed);
            self.handle
                .take()
                .expect("callback thread")
                .join()
                .expect("join")
        }
    }

    struct Harness {
        engine: EngineThread,
        callback: FakeCallback,
    }

    impl Harness {
        fn new() -> Self {
            let (producer, consumer) = RingBuffer::new(RING_FRAMES * 2);
            let clock = Arc::new(AudioClock::new(100));
            let callback = FakeCallback::start(consumer, Arc::clone(&clock));
            let (events, _) = unbounded();
            let shared = Arc::new(Mutex::new(PlaybackSnapshot::default()));
            let engine = EngineThread::new(
                producer,
                OutputInfo {
                    sample_rate: SAMPLE_RATE,
                    channels: 2,
                },
                clock,
                events,
                shared,
                100,
            );
            Self { engine, callback }
        }

        /// Start with `tracks`, then prefetch the next track into the ring.
        fn prefetch_next(&mut self, tracks: Vec<Track>) {
            self.engine
                .start_tracks_internal(tracks, 0, true, Duration::ZERO);
            self.callback.drain(RING_FRAMES / 2);
            self.engine.advance_auto();
            self.engine.pump_audio();
        }

        fn ring_samples(self) -> Vec<f32> {
            let mut consumer = self.callback.stop();
            let mut samples = Vec::new();
            while let Ok(sample) = consumer.pop() {
                samples.push(sample);
            }
            samples
        }
    }

    fn assert_only(samples: &[f32], expected: f32) {
        assert!(!samples.is_empty(), "the ring should contain audio");
        assert!(
            samples
                .iter()
                .all(|sample| (sample - expected).abs() < 0.01),
            "the ring still contains audio from another track"
        );
    }

    fn titles(engine: &EngineThread) -> Vec<&str> {
        engine
            .queue
            .items()
            .iter()
            .map(|item| item.track.title.as_str())
            .collect()
    }

    #[test]
    fn removing_the_prefetched_next_track_flushes_its_audio() {
        let dir = tempfile::tempdir().expect("temp dir");
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        let c = dir.path().join("c.wav");
        write_constant_wav(&a, 0.25);
        write_constant_wav(&b, -0.75);
        write_constant_wav(&c, 0.5);
        let mut harness = Harness::new();
        harness.prefetch_next(vec![track(1, &a), track(2, &b), track(3, &c)]);

        harness.engine.remove_item(1);

        assert_eq!(titles(&harness.engine), vec!["Track 1", "Track 3"]);
        assert!(harness.engine.boundaries.is_empty());
        assert_only(&harness.ring_samples(), 0.25);
    }

    #[test]
    fn moving_the_prefetched_next_track_flushes_its_audio() {
        let dir = tempfile::tempdir().expect("temp dir");
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        let c = dir.path().join("c.wav");
        write_constant_wav(&a, 0.25);
        write_constant_wav(&b, -0.75);
        write_constant_wav(&c, 0.5);
        let mut harness = Harness::new();
        harness.prefetch_next(vec![track(1, &a), track(2, &b), track(3, &c)]);

        harness.engine.move_item(1, 2);

        assert_eq!(
            titles(&harness.engine),
            vec!["Track 1", "Track 3", "Track 2"]
        );
        assert!(harness.engine.boundaries.is_empty());
        assert_only(&harness.ring_samples(), 0.25);
    }

    #[test]
    fn removing_the_audible_track_starts_the_prefetched_next_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        write_constant_wav(&a, 0.25);
        write_constant_wav(&b, -0.75);
        let mut harness = Harness::new();
        harness.prefetch_next(vec![track(1, &a), track(2, &b)]);

        harness.engine.remove_item(0);

        assert_eq!(titles(&harness.engine), vec!["Track 2"]);
        assert!(harness.engine.boundaries.is_empty());
        assert_only(&harness.ring_samples(), -0.75);
    }
}
