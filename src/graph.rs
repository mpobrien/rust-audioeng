use crate::amp::AmplifiedSource;
use crate::delay::DelayNode;
use crate::envelope::{Adsr, EnvelopedSource};
use crate::filter::{BiquadFilter, CompiledParam, FilterType, FilteredSource, ModulatedFilterSource};
use crate::mixer::MixedSource;
use crate::oscillator::{ConstantSource, Oscillator, OscillatorShape, SampleSource, waveform};

/// A node parameter: either a fixed value or a signal from another node.
pub enum ParamDef {
    /// Fixed value.
    Const(f64),
    /// Value driven by another node. Output each sample: `node * scale + offset`.
    Signal { node: Box<NodeDef>, scale: f64, offset: f64 },
    /// Like `Signal`, but the modulation depth is itself a signal (e.g. an ADSR).
    /// Output each sample: `node * depth * scale + offset`.
    ModSignal { node: Box<NodeDef>, depth: Box<NodeDef>, scale: f64, offset: f64 },
}

/// A node in the audio graph. Compose these into a tree, then call [`compile`].
pub enum NodeDef {
    /// Outputs a fixed constant value every sample — useful as a neutral source.
    Const(f64),

    /// Periodic waveform generator with fixed frequency.
    Oscillator { shape: OscillatorShape, frequency: f64 },

    /// Periodic waveform generator with phase modulation and optional self-feedback.
    /// `frequency` encodes the carrier freq (offset) + modulator signal (node/depth/scale).
    /// `feedback` feeds the previous output sample back into the phase accumulator.
    PmOscillator { shape: OscillatorShape, frequency: ParamDef, feedback: f64 },

    /// Biquad filter. Coefficients are computed once when both params are
    /// `Const`, or recomputed every sample when either is `Signal`.
    Filter { kind: FilterType, cutoff_hz: ParamDef, q: ParamDef, source: Box<NodeDef> },

    /// ADSR envelope. Triggers note-off after `duration_secs`, then releases.
    /// Reports [`SampleSource::is_done`] once the tail has decayed.
    Envelope { adsr: Adsr, duration_secs: f32, source: Box<NodeDef> },

    /// Mixes sources together. Gains should sum to ≤ 1.0 to prevent clipping.
    Mix { sources: Vec<(NodeDef, f64)> },

    /// Multiplies a source by an amplitude signal. `level` should stay in
    /// 0.0..1.0. Use this for tremolo, custom envelope shapes, or any
    /// amplitude modulation that isn't tied to note lifecycle.
    Amplify { level: ParamDef, source: Box<NodeDef> },

    /// Delay line. `delay_secs` can be modulated for chorus and flanger.
    /// `feedback` (0.0..1.0) controls echo tail length.
    /// `mix` blends dry (0.0) and wet (1.0).
    /// `max_delay_secs` sets the buffer size; `delay_secs` must stay below it.
    Delay {
        max_delay_secs: f32,
        delay_secs: ParamDef,
        feedback: f64,
        mix: f64,
        source: Box<NodeDef>,
    },
}

/// Compiles a [`NodeDef`] tree into a runnable [`SampleSource`].
pub fn compile(node: NodeDef, sample_rate: u32) -> Box<dyn SampleSource> {
    match node {
        NodeDef::Const(v) => Box::new(ConstantSource(v)),
        NodeDef::Oscillator { shape, frequency } => {
            Box::new(Oscillator::new(shape, frequency, sample_rate))
        }
        NodeDef::PmOscillator { shape, frequency, feedback } => {
            // Carrier freq lives in `offset`; modulation lives in node/depth/scale.
            // We split them so PmSource can advance phase by carrier freq only and
            // add the modulation (including self-feedback) directly to the phase.
            let (carrier_freq, modulator) = match frequency {
                ParamDef::Const(v) => (v, CompiledParam::Const(0.0)),
                ParamDef::Signal { node, scale, offset } => (
                    offset,
                    CompiledParam::Signal { source: compile(*node, sample_rate), scale, offset: 0.0 },
                ),
                ParamDef::ModSignal { node, depth, scale, offset } => (
                    offset,
                    CompiledParam::ModSignal {
                        source: compile(*node, sample_rate),
                        depth:  compile(*depth, sample_rate),
                        scale,
                        offset: 0.0,
                    },
                ),
            };
            Box::new(PmSource { shape, sample_rate, phase: 0, carrier_freq, modulator, feedback, prev_output: 0.0 })
        }
        NodeDef::Filter { kind, cutoff_hz, q, source } => {
            let init_cutoff = param_initial_value(&cutoff_hz);
            let init_q = param_initial_value(&q);
            let compiled_source = compile(*source, sample_rate);
            let cutoff = compile_param(cutoff_hz, sample_rate);
            let q = compile_param(q, sample_rate);
            let filter = BiquadFilter::new(kind, init_cutoff, init_q, sample_rate);

            match (cutoff, q) {
                (CompiledParam::Const(_), CompiledParam::Const(_)) => {
                    Box::new(FilteredSource::new(compiled_source, filter))
                }
                (cutoff, q) => {
                    Box::new(ModulatedFilterSource::new(compiled_source, filter, cutoff, q))
                }
            }
        }
        NodeDef::Envelope { adsr, duration_secs, source } => {
            Box::new(EnvelopedSource::new(
                compile(*source, sample_rate),
                adsr,
                duration_secs,
                sample_rate,
            ))
        }
        NodeDef::Mix { sources } => {
            let mut mix = MixedSource::new();
            for (node, gain) in sources {
                mix.add_source(compile(node, sample_rate), gain);
            }
            Box::new(mix)
        }
        NodeDef::Amplify { level, source } => {
            Box::new(AmplifiedSource::new(
                compile(*source, sample_rate),
                compile_param(level, sample_rate),
            ))
        }
        NodeDef::Delay { max_delay_secs, delay_secs, feedback, mix, source } => {
            Box::new(DelayNode::new(
                compile(*source, sample_rate),
                max_delay_secs,
                compile_param(delay_secs, sample_rate),
                feedback,
                mix,
                sample_rate,
            ))
        }
    }
}

/// Returns the initial value of a param — used to seed filter coefficients
/// before the first modulator sample is available.
fn param_initial_value(param: &ParamDef) -> f64 {
    match param {
        ParamDef::Const(v) => *v,
        ParamDef::Signal    { offset, .. } => *offset,
        ParamDef::ModSignal { offset, .. } => *offset,
    }
}

/// Compiles a [`ParamDef`] into a [`CompiledParam`], recursively compiling
/// any modulator node.
fn compile_param(param: ParamDef, sample_rate: u32) -> CompiledParam {
    match param {
        ParamDef::Const(v) => CompiledParam::Const(v),
        ParamDef::Signal { node, scale, offset } => CompiledParam::Signal {
            source: compile(*node, sample_rate),
            scale,
            offset,
        },
        ParamDef::ModSignal { node, depth, scale, offset } => CompiledParam::ModSignal {
            source: compile(*node, sample_rate),
            depth:  compile(*depth, sample_rate),
            scale,
            offset,
        },
    }
}

/// Phase-modulation oscillator. The carrier advances at `carrier_freq` Hz;
/// `modulator` (in cycles) is added directly to the phase each sample.
/// `feedback` scales the previous output back into the phase — DX7-style
/// operator self-feedback for waveform distortion and harmonic enrichment.
struct PmSource {
    shape:        OscillatorShape,
    sample_rate:  u32,
    phase:        u32,
    carrier_freq: f64,
    modulator:    CompiledParam,
    feedback:     f64,
    prev_output:  f64,
}

impl SampleSource for PmSource {
    fn next_samples(&mut self, buf: &mut [f64]) {
        for sample in buf.iter_mut() {
            // Phase deviation in cycles from modulator + self-feedback.
            let mod_phase = self.modulator.next_value()
                          + self.feedback * self.prev_output;
            // Carrier phase in 0..1, then offset by modulation.
            let base = self.phase as f64 / (u32::MAX as f64 + 1.0);
            // rem_euclid keeps the result in [0, 1) even for negative mod_phase.
            let t = (base + mod_phase).rem_euclid(1.0);
            *sample = waveform(self.shape, t);
            self.prev_output = *sample;
            let inc = (self.carrier_freq.max(0.0) / self.sample_rate as f64
                       * (u32::MAX as f64 + 1.0)) as u32;
            self.phase = self.phase.wrapping_add(inc);
        }
    }
}
