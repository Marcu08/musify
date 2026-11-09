use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler as RubatoResampler, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};

/// Streaming stereo resampler.
///
/// Equal source/target rates take the exact linear passthrough path. Otherwise a
/// windowed-sinc resampler (rubato) runs in chunks; the output delay is trimmed so
/// the stream starts at the first real sample. If rubato cannot be created or a
/// chunk fails, the module falls back to linear interpolation instead of breaking
/// playback.
pub(crate) struct Resampler {
    inner: Inner,
}

enum Inner {
    Linear(LinearResampler),
    Sinc(Box<SincResampler>),
}

impl Resampler {
    pub fn new(source_rate: u32, target_rate: u32) -> Self {
        if source_rate == target_rate
            || std::env::var("MUSIFY_LINEAR_RESAMPLER").as_deref() == Ok("1")
        {
            return Self {
                inner: Inner::Linear(LinearResampler::new(source_rate, target_rate)),
            };
        }
        match SincResampler::new(source_rate, target_rate) {
            Ok(sinc) => Self {
                inner: Inner::Sinc(Box::new(sinc)),
            },
            Err(error) => {
                tracing::warn!(%error, "sinc resampler unavailable, using linear interpolation");
                Self {
                    inner: Inner::Linear(LinearResampler::new(source_rate, target_rate)),
                }
            }
        }
    }

    pub fn reset(&mut self) {
        match &mut self.inner {
            Inner::Linear(linear) => linear.reset(),
            Inner::Sinc(sinc) => sinc.reset(),
        }
    }

    pub fn process_frame(&mut self, left: f32, right: f32, out: &mut Vec<f32>) {
        match &mut self.inner {
            Inner::Linear(linear) => linear.process_frame(left, right, out),
            Inner::Sinc(sinc) => sinc.process_frame(left, right, out),
        }
    }

    /// Flush the remaining partial chunk at end of track. No-op for linear.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        if let Inner::Sinc(sinc) = &mut self.inner {
            sinc.flush(out);
        }
    }
}

#[derive(Debug)]
struct LinearResampler {
    step: f64,
    position: f64,
    input_frame: u64,
    previous: [f32; 2],
    started: bool,
}

impl LinearResampler {
    fn new(source_rate: u32, target_rate: u32) -> Self {
        let source = f64::from(source_rate.max(1));
        let target = f64::from(target_rate.max(1));
        Self {
            step: source / target,
            position: 0.0,
            input_frame: 0,
            previous: [0.0, 0.0],
            started: false,
        }
    }

    fn reset(&mut self) {
        self.position = 0.0;
        self.input_frame = 0;
        self.previous = [0.0, 0.0];
        self.started = false;
    }

    fn process_frame(&mut self, left: f32, right: f32, out: &mut Vec<f32>) {
        if !self.started {
            self.previous = [left, right];
            self.started = true;
        }
        let current = self.input_frame as f64;
        while self.position <= current {
            let t = if self.input_frame == 0 {
                0.0
            } else {
                (self.position - (current - 1.0)).clamp(0.0, 1.0) as f32
            };
            out.push(self.previous[0] + (left - self.previous[0]) * t);
            out.push(self.previous[1] + (right - self.previous[1]) * t);
            self.position += self.step;
        }
        self.previous = [left, right];
        self.input_frame += 1;
    }
}

struct SincResampler {
    resampler: Async<f32>,
    input: Vec<f32>,
    output: Vec<f32>,
    output_capacity: usize,
    skip_delay: usize,
    flushed: bool,
    ratio: f64,
    total_input_frames: u64,
    emitted_frames: u64,
}

impl SincResampler {
    fn new(source_rate: u32, target_rate: u32) -> Result<Self, String> {
        let source = f64::from(source_rate.max(1));
        let target = f64::from(target_rate.max(1));
        let ratio = target / source;
        let params = SincInterpolationParameters::new(64, WindowFunction::Blackman2)
            .oversampling_factor(128)
            .interpolation(SincInterpolationType::Quadratic);
        let resampler = Async::<f32>::new_sinc(ratio, 1.1, &params, 512, 2, FixedAsync::Input)
            .map_err(|error| error.to_string())?;
        let output_capacity = resampler.output_frames_max();
        let skip_delay = resampler.output_delay();
        Ok(Self {
            resampler,
            input: Vec::with_capacity(2 * 1024),
            output: vec![0.0; 2 * output_capacity],
            output_capacity,
            skip_delay,
            flushed: false,
            ratio,
            total_input_frames: 0,
            emitted_frames: 0,
        })
    }

    fn reset(&mut self) {
        self.resampler.reset();
        self.input.clear();
        self.skip_delay = self.resampler.output_delay();
        self.flushed = false;
        self.total_input_frames = 0;
        self.emitted_frames = 0;
    }

    fn process_frame(&mut self, left: f32, right: f32, out: &mut Vec<f32>) {
        if self.flushed {
            return;
        }
        self.input.push(left);
        self.input.push(right);
        self.total_input_frames += 1;
        while !self.flushed && self.input.len() / 2 >= self.resampler.input_frames_next() {
            self.process_chunk(out, false);
        }
    }

    fn flush(&mut self, out: &mut Vec<f32>) {
        if self.flushed {
            return;
        }
        if !self.input.is_empty() {
            self.process_chunk(out, true);
        }
        self.flushed = true;
    }

    fn process_chunk(&mut self, out: &mut Vec<f32>, partial: bool) {
        let frames = self.input.len() / 2;
        if frames == 0 {
            return;
        }
        let Ok(input) = InterleavedSlice::new(&self.input, 2, frames) else {
            self.input.clear();
            return;
        };
        self.output.resize(2 * self.output_capacity, 0.0);
        let Ok(mut output) = InterleavedSlice::new_mut(&mut self.output, 2, self.output_capacity)
        else {
            self.input.clear();
            return;
        };
        let indexing = partial.then(|| Indexing {
            partial_len: Some(frames),
            ..Indexing::default()
        });
        match self
            .resampler
            .process_into_buffer(&input, &mut output, indexing.as_ref())
        {
            Ok((_consumed, produced)) => {
                let mut start = 0;
                if self.skip_delay > 0 {
                    let skip = self.skip_delay.min(produced);
                    self.skip_delay -= skip;
                    start = skip;
                }
                let expected_total = (self.total_input_frames as f64 * self.ratio).round() as u64;
                let remaining = expected_total.saturating_sub(self.emitted_frames) as usize;
                let append = (produced.saturating_sub(start)).min(remaining);
                if append > 0 {
                    out.extend_from_slice(&self.output[start * 2..(start + append) * 2]);
                    self.emitted_frames += append as u64;
                }
                self.input.clear();
            }
            Err(error) => {
                tracing::warn!(%error, "sinc resampling chunk failed, dropping it");
                self.input.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rate_is_sample_exact() {
        let mut resampler = Resampler::new(48_000, 48_000);
        let mut out = Vec::new();
        for i in 0..128 {
            let sample = i as f32;
            resampler.process_frame(sample, -sample, &mut out);
        }
        assert_eq!(out.len(), 256);
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], 0.0);
        assert_eq!(out[2 * 127], 127.0);
        assert_eq!(out[2 * 127 + 1], -127.0);
    }

    #[test]
    fn sinc_output_length_follows_the_rate_ratio() {
        let source_rate = 44_100;
        let target_rate = 48_000;
        let frames = source_rate as usize;
        let mut resampler = Resampler::new(source_rate, target_rate);
        let mut out = Vec::new();
        for i in 0..frames {
            let sample = (i as f32 * 0.01).sin();
            resampler.process_frame(sample, sample, &mut out);
        }
        resampler.flush(&mut out);
        let produced = out.len() / 2;
        assert!(
            produced.abs_diff(target_rate as usize) <= 32,
            "produced {produced} frames, expected about {target_rate}"
        );
    }

    #[test]
    fn sinc_preserves_a_sine_without_obvious_clicks() {
        let source_rate = 44_100;
        let target_rate = 48_000;
        let frequency = 1_000.0_f32;
        let mut resampler = Resampler::new(source_rate, target_rate);
        let mut out = Vec::new();
        for i in 0..source_rate {
            let phase = std::f32::consts::TAU * frequency * i as f32 / source_rate as f32;
            let sample = phase.sin() * 0.5;
            resampler.process_frame(sample, sample, &mut out);
        }
        resampler.flush(&mut out);
        let samples: Vec<f32> = out
            .as_chunks::<2>()
            .0
            .iter()
            .map(|frame| frame[0])
            .collect();
        assert!(samples.iter().all(|sample| sample.is_finite()));
        let peak = samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        assert!((0.35..=0.65).contains(&peak), "unexpected peak {peak}");
        let max_step = samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(max_step < 0.15, "sinc produced a discontinuity: {max_step}");
    }

    #[test]
    fn reset_starts_a_new_stream() {
        let mut resampler = Resampler::new(44_100, 48_000);
        let mut out = Vec::new();
        for _ in 0..600 {
            resampler.process_frame(1.0, 1.0, &mut out);
        }
        resampler.flush(&mut out);
        resampler.reset();
        out.clear();
        for _ in 0..600 {
            resampler.process_frame(0.5, 0.5, &mut out);
        }
        resampler.flush(&mut out);
        let samples: Vec<f32> = out
            .as_chunks::<2>()
            .0
            .iter()
            .map(|frame| frame[0])
            .collect();
        assert!(samples.iter().all(|sample| sample.is_finite()));
        assert!(samples.last().is_some_and(|sample| *sample > 0.3));
    }
}
