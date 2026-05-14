use crate::amp::AmplifiedSource;
use crate::delay::DelayNode;
use crate::envelope::{Adsr, EnvelopedSource};
use crate::filter::{BiquadFilter, CompiledParam, FilterType, FilteredSource, ModulatedFilterSource};
use crate::mixer::MixedSource;
use crate::oscillator::{Oscillator, OscillatorShape, SampleSource};

/// A node parameter: either a fixed value or a signal from another node.
///
/// For `Signal`, the actual value each sample is `modulator * scale + offset`.
pub enum ParamDef {
    /// Fixed value.
    Const(f64),
    /// Value driven by another node. `offset` is the base, `scale` is the modulation depth.
    Signal { node: Box<NodeDef>, scale: f64, offset: f64 },
}

/// A node in the audio graph. Compose these into a tree, then call [`compile`].
pub enum NodeDef {
    /// Periodic waveform generator.
    Oscillator { shape: OscillatorShape, frequency: f64 },

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
        NodeDef::Oscillator { shape, frequency } => {
            Box::new(Oscillator::new(shape, frequency, sample_rate))
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
        ParamDef::Signal { offset, .. } => *offset,
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
    }
}
