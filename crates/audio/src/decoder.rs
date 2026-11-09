use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

use crate::error::AudioError;
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
        let reader = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|error| {
                AudioError::Decode(format!("cannot probe {}: {error}", path.display()))
            })?;

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
        })
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
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
            let packet = match self.reader.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    self.eof = true;
                    break;
                }
                Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                    self.eof = true;
                    break;
                }
                Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::Interrupted => {
                    continue;
                }
                Err(error) => return Err(AudioError::Decode(error.to_string())),
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
