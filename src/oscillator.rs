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

pub fn waveform(shape: OscillatorShape, t: f64) -> f64 {
    match shape {
        OscillatorShape::Sine     => (t * 2.0 * std::f64::consts::PI).sin(),
        OscillatorShape::Square   => if t < 0.5 { 1.0 } else { -1.0 },
        OscillatorShape::Sawtooth => t * 2.0 - 1.0,
        OscillatorShape::Triangle => if t < 0.5 { t * 4.0 - 1.0 } else { 3.0 - t * 4.0 },
    }
}

impl SampleSource for Oscillator {
    fn next_samples(&mut self, buf: &mut [f64]) {
        for sample in buf.iter_mut() {
            let t = self.phase as f64 / u32::MAX as f64;
            *sample = waveform(self.shape, t);
            self.phase = self.phase.wrapping_add(self.phase_increment);
        }
    }
}

pub trait SampleSource: Send {
    fn next_samples(&mut self, buf: &mut [f64]);
    fn is_done(&self) -> bool { false }
}

/// Outputs a fixed constant value every sample.
pub struct ConstantSource(pub f64);

impl SampleSource for ConstantSource {
    fn next_samples(&mut self, buf: &mut [f64]) {
        for s in buf.iter_mut() { *s = self.0; }
    }
}

pub struct FmOscillator {
    shape: OscillatorShape,
    sample_rate: u32,
    phase: u32,
    freq_source: Box<dyn SampleSource>,
    freq_scale: f64,
    freq_offset: f64,
}

impl FmOscillator {
    pub fn new(
        shape: OscillatorShape,
        freq_source: Box<dyn SampleSource>,
        freq_scale: f64,
        freq_offset: f64,
        sample_rate: u32,
    ) -> Self {
        Self { shape, sample_rate, phase: 0, freq_source, freq_scale, freq_offset }
    }
}

impl SampleSource for FmOscillator {
    fn next_samples(&mut self, buf: &mut [f64]) {
        let mut freq_buf = [0.0f64; 1];
        for sample in buf.iter_mut() {
            self.freq_source.next_samples(&mut freq_buf);
            let freq = (freq_buf[0] * self.freq_scale + self.freq_offset).max(0.0);
            let t = self.phase as f64 / u32::MAX as f64;
            *sample = waveform(self.shape, t);
            let inc = (freq / self.sample_rate as f64 * (u32::MAX as f64 + 1.0)) as u32;
            self.phase = self.phase.wrapping_add(inc);
        }
    }
}
