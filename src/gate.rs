/// A gate signal: open (high) = note on, closed (low) = note off.
/// The voice polls the gate each sample and triggers release when it closes.
pub trait Gate {
    fn poll(&mut self) -> bool;
}

/// Gate that closes automatically after a fixed duration.
pub struct TimedGate {
    duration_samples: u32,
    elapsed: u32,
}

impl TimedGate {
    pub fn new(duration_secs: f32, sample_rate: u32) -> Self {
        Self {
            duration_samples: (duration_secs * sample_rate as f32) as u32,
            elapsed: 0,
        }
    }
}

impl Gate for TimedGate {
    fn poll(&mut self) -> bool {
        if self.elapsed < self.duration_samples {
            self.elapsed += 1;
            true
        } else {
            false
        }
    }
}

/// Gate controlled externally — stays open until `close()` is called.
/// Use this for keyboard input or any source where note-off is unpredictable.
pub struct ManualGate {
    open: bool,
}

impl ManualGate {
    pub fn new() -> Self {
        Self { open: true }
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

impl Gate for ManualGate {
    fn poll(&mut self) -> bool {
        self.open
    }
}
