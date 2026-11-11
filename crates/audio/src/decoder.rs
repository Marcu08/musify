use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use musify_core::{SourceId, StreamEvent};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

use crate::error::AudioError;
use crate::http_stream::{HttpStream, StreamMode};
use crate::resample::Resampler;

/// One decoded track, converted to interleaved stereo f32 at the output rate.
pub(crate) struct TrackDecoder {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    output_rate: u32,
    resampler: Resampler,
    decoded: Vec<f32>,
    resampled: Vec<f32>,
    skip_frames: u64,
    eof: bool,
    resampler_flushed: bool,
    duration: Option<Duration>,
    stream: Option<Arc<HttpStream>>,
    stream_generation: u64,
    stream_description: String,
}

impl TrackDecoder {
    pub fn open(path: &Path, output_rate: u32) -> Result<Self, AudioError> {
        let file = File::open(path)
            .map_err(|error| AudioError::Io(format!("cannot open {}: {error}", path.display())))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        Self::open_source(
            stream,
            hint,
            output_rate,
            format!("{}", path.display()),
            None,
        )
    }

    pub fn open_stream(
        source: &SourceId,
        output_rate: u32,
        on_demand: bool,
    ) -> Result<Self, AudioError> {
        if source.uri().to_ascii_lowercase().contains(".m3u8") {
            return Err(AudioError::Unsupported(
                "gli stream HLS non sono supportati".into(),
            ));
        }
        let mode = if on_demand {
            StreamMode::OnDemand
        } else {
            StreamMode::Live
        };
        let handle = Arc::new(HttpStream::start(source.uri().to_string(), mode));
        let content_type = handle.wait_ready()?.unwrap_or_default();
        if is_hls_content_type(&content_type) {
            return Err(AudioError::Unsupported(
                "gli stream HLS non sono supportati".into(),
            ));
        }
        let generation = handle.generation();
        let hint = stream_hint(&content_type, source.uri());
        let reader = handle.reader();
        let mut decoder = Self::open_source(
            MediaSourceStream::new(Box::new(reader), Default::default()),
            hint,
            output_rate,
            source.uri().to_string(),
            Some(handle),
        )?;
        decoder.stream_generation = generation;
        Ok(decoder)
    }

    fn reopen_stream(&mut self) -> Result<(), AudioError> {
        let Some(handle) = self.stream.as_ref().cloned() else {
            return Ok(());
        };
        let generation = handle.generation();
        let content_type = handle.content_type().unwrap_or_default();
        let hint = stream_hint(&content_type, &self.stream_description);
        let reader = handle.reader();
        let mut replacement = Self::open_source(
            MediaSourceStream::new(Box::new(reader), Default::default()),
            hint,
            self.output_rate,
            self.stream_description.clone(),
            Some(handle),
        )?;
        replacement.stream_generation = generation;
        *self = replacement;
        Ok(())
    }

    fn open_source(
        stream: MediaSourceStream<'static>,
        hint: Hint,
        output_rate: u32,
        description: String,
        stream_handle: Option<Arc<HttpStream>>,
    ) -> Result<Self, AudioError> {
        let reader = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|error| AudioError::Decode(format!("cannot probe {description}: {error}")))?;

        let (track_id, time_base, duration, source_rate, codec_params) = {
            let track = reader
                .default_track(TrackType::Audio)
                .ok_or_else(|| AudioError::Unsupported("no audio track in the file".into()))?;
            let params = track
                .codec_params
                .as_ref()
                .and_then(|params| params.audio())
                .ok_or_else(|| AudioError::Unsupported("missing audio codec parameters".into()))?;
            let source_rate = params
                .sample_rate
                .ok_or_else(|| AudioError::Unsupported("unknown sample rate".into()))?;
            let duration = track
                .duration
                .and_then(|duration| track.time_base.and_then(|tb| tb.calc_duration(duration)))
                .map(|time| Duration::from_secs_f64(time.as_secs_f64().max(0.0)))
                .or_else(|| {
                    track.num_frames.and_then(|frames| {
                        (source_rate > 0).then(|| {
                            Duration::from_secs_f64(frames as f64 / f64::from(source_rate))
                        })
                    })
                });
            (
                track.id,
                track.time_base,
                duration,
                source_rate,
                params.clone(),
            )
        };

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
            .map_err(|error| AudioError::Unsupported(error.to_string()))?;

        Ok(Self {
            reader,
            decoder,
            track_id,
            time_base,
            output_rate,
            resampler: Resampler::new(source_rate, output_rate),
            decoded: Vec::new(),
            resampled: Vec::new(),
            skip_frames: 0,
            eof: false,
            resampler_flushed: false,
            duration,
            stream: stream_handle,
            stream_generation: 0,
            stream_description: description.clone(),
        })
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn is_seekable(&self) -> bool {
        self.stream
            .as_ref()
            .is_none_or(|stream| stream.is_seekable())
    }

    fn stream_generation_changed(&self) -> bool {
        self.stream
            .as_ref()
            .is_some_and(|stream| stream.generation() != self.stream_generation)
    }

    pub fn take_stream_updates(&self) -> Vec<StreamEvent> {
        self.stream
            .as_ref()
            .map_or_else(Vec::new, |stream| stream.take_updates())
    }

    /// Decode until `target_frames` stereo frames are available or the track ends.
    /// Returns the number of frames appended to `out`.
    pub fn decode_into(
        &mut self,
        out: &mut Vec<f32>,
        target_frames: usize,
    ) -> Result<usize, AudioError> {
        if self.eof {
            if !self.resampler_flushed {
                self.resampler.flush(out);
                self.resampler_flushed = true;
            }
            return Ok(out.len() / 2);
        }
        while out.len() / 2 < target_frames {
            if self.stream_generation_changed() {
                self.reopen_stream()?;
                if out.len() / 2 >= target_frames {
                    break;
                }
            }
            let packet = match self.reader.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    self.eof = true;
                    break;
                }
                Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                    if self.stream_generation_changed() {
                        self.reopen_stream()?;
                        continue;
                    }
                    self.eof = true;
                    break;
                }
                Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::Interrupted => {
                    continue;
                }
                Err(error) => {
                    if self.stream_generation_changed() {
                        self.reopen_stream()?;
                        continue;
                    }
                    return Err(AudioError::Decode(error.to_string()));
                }
            };
            if packet.track_id != self.track_id {
                continue;
            }

            let channels = {
                let audio = match self.decoder.decode(&packet) {
                    Ok(audio) => audio,
                    Err(SymphoniaError::DecodeError(_)) => continue,
                    Err(SymphoniaError::ResetRequired) => {
                        self.decoder.reset();
                        continue;
                    }
                    Err(error) => return Err(AudioError::Decode(error.to_string())),
                };
                let channels = audio.spec().channels().count().max(1);
                self.decoded.resize(audio.samples_interleaved(), 0.0);
                audio.copy_to_slice_interleaved(&mut self.decoded);
                channels
            };

            self.resampled.clear();
            for frame in self.decoded.chunks_exact(channels) {
                let left = frame.first().copied().unwrap_or(0.0);
                let right = if channels > 1 {
                    frame.get(1).copied().unwrap_or(left)
                } else {
                    left
                };
                self.resampler
                    .process_frame(left, right, &mut self.resampled);
            }

            if self.skip_frames > 0 {
                let samples = (self.skip_frames as usize).saturating_mul(2);
                let samples = samples.min(self.resampled.len());
                self.resampled.drain(0..samples);
                self.skip_frames -= (samples / 2) as u64;
            }

            out.extend_from_slice(&self.resampled);
        }
        if self.eof && !self.resampler_flushed {
            self.resampler.flush(out);
            self.resampler_flushed = true;
        }
        Ok(out.len() / 2)
    }

    /// Seek to `position`, returning the position actually reached before packet rounding.
    pub fn seek(&mut self, position: Duration) -> Result<Duration, AudioError> {
        let requested = position.as_secs_f64().max(0.0);
        let time = Time::try_from_secs_f64(requested).ok_or(AudioError::InvalidSeek)?;
        let seeked = self
            .reader
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|error| AudioError::Decode(format!("seek failed: {error}")))?;
        self.decoder.reset();
        self.resampler.reset();
        self.resampler_flushed = false;
        self.decoded.clear();
        self.resampled.clear();
        self.eof = false;

        let actual = self
            .time_base
            .and_then(|time_base| time_base.calc_time(seeked.actual_ts))
            .map_or(requested, |time| time.as_secs_f64().max(0.0));
        let skip_seconds = (requested - actual).max(0.0);
        self.skip_frames = (skip_seconds * f64::from(self.output_rate)).round() as u64;
        Ok(Duration::from_secs_f64(actual))
    }
}

fn is_hls_content_type(content_type: &str) -> bool {
    let value = content_type.to_ascii_lowercase();
    value.contains("mpegurl") || value.contains("m3u8")
}

fn stream_hint(content_type: &str, uri: &str) -> Hint {
    let value = content_type.to_ascii_lowercase();
    let extension = if value.contains("mpeg") {
        Some("mp3")
    } else if value.contains("aac") {
        Some("aac")
    } else if value.contains("ogg") || value.contains("opus") {
        Some("ogg")
    } else if value.contains("flac") {
        Some("flac")
    } else {
        uri.rsplit('/')
            .next()
            .and_then(|name| name.split('?').next())
            .and_then(|name| name.rsplit_once('.').map(|(_, ext)| ext))
    };
    let mut hint = Hint::new();
    if let Some(extension) = extension {
        hint.with_extension(extension);
    }
    hint
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn asset(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join(name)
    }

    fn decode_all(name: &str) {
        let path = asset(name);
        if !path.exists() {
            eprintln!("skipping missing test asset {}", path.display());
            return;
        }
        let mut decoder = TrackDecoder::open(&path, 48_000).expect("open test asset");
        let mut out = Vec::new();
        let mut total = 0usize;
        loop {
            out.clear();
            let frames = decoder.decode_into(&mut out, 4096).expect("decode");
            if frames == 0 {
                break;
            }
            total += frames;
            assert!(out.iter().all(|sample| sample.is_finite()));
        }
        assert!(total > 0, "decoded no frames from {name}");
        assert!(decoder.duration().is_some(), "duration missing for {name}");
    }

    #[test]
    fn decodes_mp3() {
        decode_all("tone.mp3");
    }

    #[test]
    fn hls_urls_fail_with_a_clear_unsupported_error() {
        let source = musify_core::SourceId::radio("https://stream.example/live/index.m3u8");
        let Err(error) = TrackDecoder::open_stream(&source, 48_000, false) else {
            panic!("HLS must not open as a progressive stream");
        };
        assert!(matches!(error, AudioError::Unsupported(_)));
        assert!(error.to_string().contains("HLS"));
    }

    #[test]
    fn decodes_an_http_stream() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let bytes =
            std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/assets/tone.mp3"))
                .expect("read tone");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}/live.mp3", listener.local_addr().expect("addr"));
        let server = thread::spawn(move || {
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
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nContent-Length: ")
                .expect("headers");
            socket
                .write_all(bytes.len().to_string().as_bytes())
                .expect("length");
            socket.write_all(b"\r\n\r\n").expect("end headers");
            socket.write_all(&bytes).expect("body");
            socket.flush().expect("flush");
        });
        let source = musify_core::SourceId::radio(url);
        let mut decoder = TrackDecoder::open_stream(&source, 48_000, false).expect("open stream");
        let mut out = Vec::new();
        let frames = decoder.decode_into(&mut out, 4096).expect("decode");
        assert!(frames > 0, "the HTTP stream should decode audio");
        assert!(out.iter().all(|sample| sample.is_finite()));
        drop(decoder);
        let _ = server.join();
    }

    #[test]
    fn seeks_an_http_on_demand_stream_with_range_requests() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let bytes =
            std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/assets/tone.mp3"))
                .expect("read tone");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let address = listener.local_addr().expect("addr");
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let server = thread::spawn(move || {
            while !server_stop.load(std::sync::atomic::Ordering::Acquire) {
                let (mut socket, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => return,
                };
                let _ = socket.set_nonblocking(false);
                let mut request = Vec::new();
                let mut byte = [0u8; 1];
                while !request.ends_with(b"\r\n\r\n") {
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
                if start > 0 {
                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\n\
                         Content-Type: audio/mpeg\r\n\
                         Accept-Ranges: bytes\r\n\
                         Content-Range: bytes {start}-{}/{}\r\n\
                         Content-Length: {}\r\n\r\n",
                        bytes.len() - 1,
                        bytes.len(),
                        bytes.len() - start
                    );
                    socket.write_all(header.as_bytes()).expect("headers");
                    socket.write_all(&bytes[start..]).expect("body");
                } else {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\n\
                         Content-Type: audio/mpeg\r\n\
                         Accept-Ranges: bytes\r\n\
                         Content-Length: {}\r\n\r\n",
                        bytes.len()
                    );
                    socket.write_all(header.as_bytes()).expect("headers");
                    socket.write_all(&bytes).expect("body");
                }
                socket.flush().expect("flush");
                drop(socket);
            }
        });

        let source = musify_core::SourceId::podcast(format!("http://{address}/episode.mp3"));
        let mut decoder =
            TrackDecoder::open_stream(&source, 48_000, true).expect("open on-demand stream");
        assert!(decoder.is_seekable());
        let mut out = Vec::new();
        let frames = decoder.decode_into(&mut out, 4096).expect("decode");
        assert!(frames > 0);
        let actual = decoder
            .seek(Duration::from_millis(350))
            .expect("range seek");
        assert!(actual <= Duration::from_millis(350));
        out.clear();
        let frames = decoder
            .decode_into(&mut out, 4096)
            .expect("decode after range seek");
        assert!(frames > 0);
        drop(decoder);
        stop.store(true, std::sync::atomic::Ordering::Release);
        let _ = server.join();
    }

    #[test]
    #[ignore = "real network; run manually"]
    fn plays_and_seeks_a_real_podcast_episode() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let feed = runtime
            .block_on(async {
                let client = reqwest::Client::builder()
                    .user_agent("Musify/0.1.0")
                    .build()?;
                client
                    .get("https://feeds.simplecast.com/54nAGcIl")
                    .send()
                    .await?
                    .error_for_status()?
                    .text()
                    .await
            })
            .expect("real podcast feed");
        let enclosure = feed.find("<enclosure").expect("enclosure");
        let tag = &feed[enclosure..enclosure + feed[enclosure..].find('>').expect("tag")];
        let url_start = tag.find("url=\"").expect("url") + "url=\"".len();
        let url_end = tag[url_start..].find('"').expect("url end") + url_start;
        let url = tag[url_start..url_end].replace("&amp;", "&");
        println!("episode: {url}");
        let source = musify_core::SourceId::podcast(url);
        let mut decoder = TrackDecoder::open_stream(&source, 48_000, true).expect("open real");
        println!(
            "duration: {:?}, seekable: {}",
            decoder.duration(),
            decoder.is_seekable()
        );
        assert!(
            decoder.is_seekable(),
            "the real server should support Range"
        );
        let mut out = Vec::new();
        let frames = decoder.decode_into(&mut out, 4096).expect("decode real");
        assert!(frames > 0);
        let actual = decoder.seek(Duration::from_secs(30)).expect("seek real");
        assert!(actual <= Duration::from_secs(30));
        out.clear();
        let frames = decoder
            .decode_into(&mut out, 4096)
            .expect("decode after real seek");
        assert!(frames > 0);
    }

    #[test]
    fn decodes_flac() {
        decode_all("tone.flac");
    }

    #[test]
    fn decodes_ogg() {
        decode_all("tone.ogg");
    }

    #[test]
    fn decodes_wav_and_m4a() {
        decode_all("tone.wav");
        decode_all("tone.m4a");
    }

    #[test]
    fn seek_reaches_the_requested_region() {
        let path = asset("tone.flac");
        if !path.exists() {
            eprintln!("skipping missing test asset {}", path.display());
            return;
        }
        let mut decoder = TrackDecoder::open(&path, 48_000).expect("open test asset");
        let actual = decoder.seek(Duration::from_millis(350)).expect("seek");
        assert!(
            actual <= Duration::from_millis(350),
            "accurate seek returned {actual:?}"
        );
        let mut out = Vec::new();
        let frames = decoder
            .decode_into(&mut out, 1024)
            .expect("decode after seek");
        assert!(frames > 0);
    }
}
