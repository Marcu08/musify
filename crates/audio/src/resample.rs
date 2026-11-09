/// Streaming linear resampler for stereo frames.
///
/// The audio engine keeps a fixed output rate. Linear interpolation is cheap and
/// deterministic; it is not a high-end sinc resampler, which is an explicit M1
/// trade-off documented in `ARCHITECTURE.md`.
#[derive(Debug)]
pub(crate) struct LinearResampler {
    step: f64,
    /// Absolute input position of the next output frame.
    position: f64,
    /// Index of the current input frame.
    input_frame: u64,
    previous: [f32; 2],
    started: bool,
}

impl LinearResampler {
    pub fn new(source_rate: u32, target_rate: u32) -> Self {
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

    pub fn reset(&mut self) {
        self.position = 0.0;
        self.input_frame = 0;
        self.previous = [0.0, 0.0];
        self.started = false;
    }

    /// Push one source frame and append any produced stereo frames to `out`.
    pub fn process_frame(&mut self, left: f32, right: f32, out: &mut Vec<f32>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rate_is_sample_exact() {
        let mut resampler = LinearResampler::new(48_000, 48_000);
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
    fn output_length_follows_the_rate_ratio() {
        let source_rate = 44_100;
        let target_rate = 48_000;
        let frames = source_rate as usize;
        let mut resampler = LinearResampler::new(source_rate, target_rate);
        let mut out = Vec::new();
        for i in 0..frames {
            let sample = (i as f32 * 0.01).sin();
            resampler.process_frame(sample, sample, &mut out);
        }
        let produced = out.len() / 2;
        assert!(
            produced.abs_diff(target_rate as usize) <= 2,
            "produced {produced} frames, expected about {target_rate}"
        );
    }

    #[test]
    fn reset_starts_a_new_stream() {
        let mut resampler = LinearResampler::new(44_100, 48_000);
        let mut out = Vec::new();
        resampler.process_frame(1.0, 1.0, &mut out);
        resampler.reset();
        out.clear();
        resampler.process_frame(0.5, 0.5, &mut out);
        assert_eq!(out.first().copied(), Some(0.5));
    }
}
