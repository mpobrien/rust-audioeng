use crate::filter::CompiledParam;
use crate::oscillator::SampleSource;

/// Single-tap delay line with linear interpolation, feedback, and dry/wet mix.
///
/// `delay_secs` is evaluated per-sample so an LFO can modulate it continuously
/// without clicks (chorus, flanger). Linear interpolation handles fractional
/// delay positions.
///
/// Mixing is additive: dry passes through at full level, delayed signal is
/// added at `mix` gain. `mix = 0.0` is dry-only; `mix = 1.0` adds echoes at
/// the same level as the dry signal. Keep `mix * feedback < 1.0` to avoid runaway feedback.
pub struct DelayNode {
    source: Box<dyn SampleSource>,
    buffer: Vec<f64>,
    write_pos: usize,
    delay_param: CompiledParam,
    feedback: f64,
    mix: f64,
    sample_rate: u32,
}

impl DelayNode {
    pub fn new(
        source: Box<dyn SampleSource>,
        max_delay_secs: f32,
        delay_param: CompiledParam,
        feedback: f64,
        mix: f64,
        sample_rate: u32,
    ) -> Self {
        // +2 so fractional read never goes out of bounds
        let capacity = (max_delay_secs * sample_rate as f32).ceil() as usize + 2;
        Self {
            source,
            buffer: vec![0.0; capacity],
            write_pos: 0,
            delay_param,
            feedback,
            mix,
            sample_rate,
        }
    }
}

impl SampleSource for DelayNode {
    fn is_done(&self) -> bool {
        self.source.is_done()
    }

    fn next_samples(&mut self, buf: &mut [f64]) {
        self.source.next_samples(buf);
        let len = self.buffer.len();

        for sample in buf.iter_mut() {
            // Clamp to at least 1 sample so read never aliases the current write pos
            let delay_samples = (self.delay_param.next_value() * self.sample_rate as f64)
                .max(1.0)
                .min((len - 2) as f64);

            let floor = delay_samples as usize;
            let frac = delay_samples - floor as f64;

            // Read two adjacent taps and interpolate
            let r0 = (self.write_pos + len - floor) % len;
            let r1 = (self.write_pos + len - floor - 1) % len;
            let delayed = self.buffer[r0] * (1.0 - frac) + self.buffer[r1] * frac;

            self.buffer[self.write_pos] = *sample + self.feedback * delayed;
            self.write_pos = (self.write_pos + 1) % len;

            *sample = *sample + delayed * self.mix;
        }
    }
}
