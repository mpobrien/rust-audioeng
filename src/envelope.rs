use crate::oscillator::SampleSource;

pub struct Adsr {
    pub attack_secs: f32,
    pub decay_secs: f32,
    pub sustain_level: f32, // 0.0..1.0
    pub release_secs: f32,
}

enum Phase {
    Attack,
    Decay,
    Sustain,
    Release,
    Done,
}

pub struct AdsrEnvelope {
    params: Adsr,
    sample_rate: u32,
    phase: Phase,
    amplitude: f32,
}

impl AdsrEnvelope {
    pub fn new(params: Adsr, sample_rate: u32) -> Self {
        Self {
            params,
            sample_rate,
            phase: Phase::Attack,
            amplitude: 0.0,
        }
    }

    pub fn note_off(&mut self) {
        self.phase = Phase::Release;
    }

    pub fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    pub fn apply(&mut self, buf: &mut [f64]) {
        for sample in buf.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    pub fn process_sample(&mut self, x: f64) -> f64 {
        let y = x * self.amplitude as f64;
        self.tick();
        y
    }

    fn tick(&mut self) {
        match self.phase {
            Phase::Attack => {
                self.amplitude += 1.0 / (self.params.attack_secs * self.sample_rate as f32);
                if self.amplitude >= 1.0 {
                    self.amplitude = 1.0;
                    self.phase = Phase::Decay;
                }
            }
            Phase::Decay => {
                let target = self.params.sustain_level;
                self.amplitude -=
                    (1.0 - target) / (self.params.decay_secs * self.sample_rate as f32);
                if self.amplitude <= target {
                    self.amplitude = target;
                    self.phase = Phase::Sustain;
                }
            }
            Phase::Sustain => {}
            Phase::Release => {
                self.amplitude -=
                    self.amplitude / (self.params.release_secs * self.sample_rate as f32);
                if self.amplitude < 1e-4 {
                    self.amplitude = 0.0;
                    self.phase = Phase::Done;
                }
            }
            Phase::Done => {}
        }
    }
}

pub struct EnvelopedSource {
    source: Box<dyn SampleSource>,
    envelope: AdsrEnvelope,
    samples_elapsed: u32,
    note_off_sample: u32,
    note_off_triggered: bool,
}

impl EnvelopedSource {
    pub fn new(
        source: Box<dyn SampleSource>,
        params: Adsr,
        duration_secs: f32,
        sample_rate: u32,
    ) -> Self {
        Self {
            source,
            envelope: AdsrEnvelope::new(params, sample_rate),
            samples_elapsed: 0,
            note_off_sample: (duration_secs * sample_rate as f32) as u32,
            note_off_triggered: false,
        }
    }

    pub fn is_done(&self) -> bool {
        self.envelope.is_done()
    }
}

impl SampleSource for EnvelopedSource {
    fn is_done(&self) -> bool {
        self.envelope.is_done()
    }

    fn next_samples(&mut self, buf: &mut [f64]) {
        self.source.next_samples(buf);
        for sample in buf.iter_mut() {
            if !self.note_off_triggered && self.samples_elapsed >= self.note_off_sample {
                self.envelope.note_off();
                self.note_off_triggered = true;
            }
            *sample = self.envelope.process_sample(*sample);
            self.samples_elapsed += 1;
        }
    }
}
