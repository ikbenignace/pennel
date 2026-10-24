pub const TARGET_RATE: u32 = 16_000;

/// Streams interleaved frames of any rate and channel count into 16 kHz mono f32.
/// Downsampling averages each output's input span (a box filter against aliasing);
/// upsampling interpolates linearly.
pub struct MonoResampler {
    channels: usize,
    step: f64,
    position: f64,
    pending: Vec<f32>,
}

impl MonoResampler {
    pub fn new(input_rate: u32, channels: u16) -> Self {
        Self {
            channels: usize::from(channels.max(1)),
            step: f64::from(input_rate.max(1)) / f64::from(TARGET_RATE),
            position: 0.0,
            pending: Vec::new(),
        }
    }

    pub fn push(&mut self, interleaved: &[f32]) -> Vec<f32> {
        self.pending.extend(
            interleaved
                .chunks_exact(self.channels)
                .map(|frame| frame.iter().sum::<f32>() / self.channels as f32),
        );
        let mut out = Vec::with_capacity((self.pending.len() as f64 / self.step) as usize + 1);
        let half = (self.step / 2.0).max(0.5);
        loop {
            let end = self.position + half;
            if end.ceil() as usize >= self.pending.len() {
                break;
            }
            out.push(if self.step > 1.0 {
                let start = (self.position - half).max(0.0).floor() as usize;
                let stop = end.ceil() as usize;
                let span = &self.pending[start..stop];
                span.iter().sum::<f32>() / span.len() as f32
            } else {
                let i = self.position.floor() as usize;
                let frac = (self.position - i as f64) as f32;
                self.pending[i] * (1.0 - frac) + self.pending[i + 1] * frac
            });
            self.position += self.step;
        }
        let consumed = ((self.position - half).floor().max(0.0) as usize).min(self.pending.len());
        self.pending.drain(..consumed);
        self.position -= consumed as f64;
        out
    }
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, seconds: f32, freq: f32, channels: u16) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .flat_map(|i| {
                let v = 0.5 * (i as f32 * freq * std::f32::consts::TAU / rate as f32).sin();
                std::iter::repeat_n(v, usize::from(channels))
            })
            .collect()
    }

    fn convert_in_chunks(rate: u32, channels: u16, input: &[f32], chunk: usize) -> Vec<f32> {
        let mut r = MonoResampler::new(rate, channels);
        input
            .chunks(chunk * usize::from(channels))
            .flat_map(|c| r.push(c))
            .collect()
    }

    #[test]
    fn downsamples_48k_stereo_to_16k_mono() {
        let out = convert_in_chunks(48_000, 2, &sine(48_000, 1.0, 440.0, 2), 480);
        assert!((15_990..=16_000).contains(&out.len()), "{}", out.len());
        assert!(
            (rms(&out) - 0.5 / 2f32.sqrt()).abs() < 0.02,
            "{}",
            rms(&out)
        );
    }

    #[test]
    fn handles_44_1k_in_odd_chunks() {
        let out = convert_in_chunks(44_100, 1, &sine(44_100, 2.0, 300.0, 1), 441);
        assert!((31_980..=32_000).contains(&out.len()), "{}", out.len());
        assert!((rms(&out) - 0.5 / 2f32.sqrt()).abs() < 0.02);
    }

    #[test]
    fn upsamples_8k() {
        let out = convert_in_chunks(8_000, 1, &sine(8_000, 1.0, 200.0, 1), 160);
        assert!((15_990..=16_000).contains(&out.len()), "{}", out.len());
        assert!((rms(&out) - 0.5 / 2f32.sqrt()).abs() < 0.02);
    }

    #[test]
    fn passes_16k_through() {
        let input = sine(16_000, 0.5, 440.0, 1);
        let out = convert_in_chunks(16_000, 1, &input, 100);
        assert!(out.len() >= input.len() - 2);
        assert!((out[100] - input[100]).abs() < 1e-6);
    }

    #[test]
    fn removes_frequencies_above_nyquist_mostly() {
        let out = convert_in_chunks(48_000, 1, &sine(48_000, 1.0, 15_000.0, 1), 480);
        assert!(rms(&out) < 0.2, "{}", rms(&out));
    }

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(rms(&[0.0; 10]), 0.0);
    }
}
