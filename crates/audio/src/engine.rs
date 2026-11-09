use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use musify_core::{Command, Event, PlaybackState, QueueItem, QueueItemId, RepeatMode, Track};
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
                self.queue.set_shuffle(shuffle);
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
            self.set_state(PlaybackState::Playing);
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
                self.decoder = Some(decoder);
                self.emit(Event::TrackChanged(Some(item)));
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
        let decoder = TrackDecoder::open(&item.track.path, self.info.sample_rate)?;
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

    fn move_item(&mut self, from: usize, to: usize) {
        self.queue.move_item(from, to);
        for boundary in &mut self.boundaries {
            boundary.queue_index = remap_index(boundary.queue_index, from, to);
        }
        if let Some(index) = self.current_index {
            self.current_index = Some(remap_index(index, from, to));
        }
        self.emit_queue();
    }

    fn remove_item(&mut self, index: usize) {
        self.boundaries
            .retain(|boundary| boundary.queue_index != index);
        for boundary in &mut self.boundaries {
            if boundary.queue_index > index {
                boundary.queue_index -= 1;
            }
        }
        self.current_index = self.current_index.map(|current| {
            if current == index {
                current
            } else if current > index {
                current - 1
            } else {
                current
            }
        });
        let outcome = self.queue.remove(index);
        if outcome.current_removed {
            self.current_index = self.queue.current_index();
        }
        if outcome.current_removed && self.queue.is_empty() {
            self.stop_playback();
        } else {
            self.emit_queue();
        }
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
