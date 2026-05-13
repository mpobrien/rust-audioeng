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

    /// Triggers the "key up" event, i.e. transition to the "release" phase
    pub fn note_off(&mut self) {
        self.phase = Phase::Release;
    }

    pub fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    // Apply the envelope to samples in the given buffer.
    pub fn apply(&mut self, buf: &mut [f64]) {
        for sample in buf.iter_mut() {
            *sample *= self.amplitude as f64;
            self.tick();
        }
    }

    // Advances the envelope by one sample, transitioning between phases according to the ADSR params.
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
                if self.amplitude <= 0.0 {
                    self.amplitude = 0.0;
                    self.phase = Phase::Done;
                }
            }
            Phase::Done => {}
        }
    }
}
