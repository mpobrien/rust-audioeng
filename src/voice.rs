use crate::envelope::{Adsr, AdsrEnvelope};
use crate::gate::{Gate, TimedGate, ManualGate};
use crate::graph::{NodeDef, compile};
use crate::oscillator::SampleSource;

/// A note to be played: pitch, velocity, and how long to hold before note-off.
pub struct NoteEvent {
    /// Frequency in Hz.
    pub frequency: f64,
    /// Amplitude scaling, 0.0..1.0.
    pub velocity: f32,
    /// How long to hold the note before releasing. `None` = hold until
    /// `ManualGate::close()` is called (e.g. key-up on a keyboard).
    pub duration_secs: Option<f32>,
}

/// A single synthesizer voice: one note playing through a signal chain.
///
/// The voice owns its source graph, envelope, and gate. It polls the gate
/// each sample and triggers the envelope release when the gate closes.
pub struct Voice {
    source: Box<dyn SampleSource>,
    envelope: AdsrEnvelope,
    gate: Box<dyn Gate>,
    velocity: f32,
    note_off_sent: bool,
}

impl Voice {
    /// Creates a voice from a compiled node graph, ADSR params, and a gate.
    pub fn new(
        node: NodeDef,
        adsr: Adsr,
        gate: Box<dyn Gate>,
        velocity: f32,
        sample_rate: u32,
    ) -> Self {
        Self {
            source: compile(node, sample_rate),
            envelope: AdsrEnvelope::new(adsr, sample_rate),
            gate,
            velocity,
            note_off_sent: false,
        }
    }

    /// Creates a voice from a [`NoteEvent`]. The gate type is chosen based on
    /// whether `duration_secs` is set: timed for sequencer notes, manual for
    /// keyboard-style input.
    pub fn from_event(
        event: NoteEvent,
        node: NodeDef,
        adsr: Adsr,
        sample_rate: u32,
    ) -> Self {
        let gate: Box<dyn Gate> = match event.duration_secs {
            Some(secs) => Box::new(TimedGate::new(secs, sample_rate)),
            None       => Box::new(ManualGate::new()),
        };
        Self::new(node, adsr, gate, event.velocity, sample_rate)
    }

    /// Returns true once the envelope has fully released.
    pub fn is_done(&self) -> bool {
        self.envelope.is_done()
    }

    /// Fills `buf` with the next block of samples. Outputs silence when done.
    pub fn render(&mut self, buf: &mut [f64]) {
        if self.is_done() {
            buf.fill(0.0);
            return;
        }

        self.source.next_samples(buf);

        for sample in buf.iter_mut() {
            if !self.note_off_sent && !self.gate.poll() {
                self.envelope.note_off();
                self.note_off_sent = true;
            }
            *sample = self.envelope.process_sample(*sample) * self.velocity as f64;
        }
    }
}
