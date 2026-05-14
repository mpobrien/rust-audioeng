use std::{fs::File, io};

const FREQ_44_1KHZ: u64 = 44100;

#[derive(Clone, Copy)]
pub enum OscillatorShape {
    Sine,
    Square,
    Sawtooth,
    Triangle,
}

pub struct Oscillator {
    shape: OscillatorShape,
    frequency: f64,
    sample_rate: u32,

    phase: u32,
    phase_increment: u32,
}

impl Oscillator {
    pub fn new(shape: OscillatorShape, frequency: f64, sample_rate: u32) -> Self {
        let phase_increment = (frequency / sample_rate as f64 * (u32::MAX as f64 + 1.0)) as u32;
        Self {
            shape,
            sample_rate,
            frequency,
            phase: 0,
            phase_increment,
        }
    }
}

impl SampleSource for Oscillator {
    fn next_samples(&mut self, buf: &mut [f64]) {
        for sample in buf.iter_mut() {
            let t = self.phase as f64 / u32::MAX as f64;
            *sample = match self.shape {
                OscillatorShape::Sine => (t * 2.0 * std::f64::consts::PI).sin(),
                OscillatorShape::Square => if self.phase < u32::MAX / 2 { 1.0 } else { -1.0 },
                OscillatorShape::Sawtooth => t * 2.0 - 1.0,
                OscillatorShape::Triangle => if t < 0.5 { t * 4.0 - 1.0 } else { 3.0 - t * 4.0 },
            };
            self.phase = self.phase.wrapping_add(self.phase_increment);
        }
    }
}

pub trait SampleSource: Send {
    fn next_samples(&mut self, buf: &mut [f64]);
    fn is_done(&self) -> bool { false }
}
