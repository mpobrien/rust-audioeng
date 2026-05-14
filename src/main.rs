mod amp;
mod delay;
mod envelope;
mod filter;
mod gate;
mod graph;
mod mixer;
mod oscillator;
mod voice;
mod wav;

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use crate::envelope::{Adsr, AdsrEnvelope};
    use crate::filter::{BiquadFilter, FilterType};
    use crate::graph::{NodeDef, ParamDef, compile};
    use crate::voice::{Voice, NoteEvent};
    use crate::mixer::MixedSource;
    use crate::oscillator::{Oscillator, OscillatorShape, SampleSource};
    use crate::wav::write_wav;
    use std::fs::File;
    use std::io::BufWriter;

    const SAMPLE_RATE: u32 = 44100;
    const FREQUENCY: f64 = 261.63;

    const ATTACK_SECS: f32 = 0.3;
    const DECAY_SECS: f32 = 0.1;
    const SUSTAIN_LEVEL: f32 = 0.8;
    const SUSTAIN_HOLD_SECS: f32 = 2.0;
    const RELEASE_SECS: f32 = 0.5;

    fn write_shape(shape: OscillatorShape, filename: &str) {
        let mut osc = Oscillator::new(shape, FREQUENCY, SAMPLE_RATE);
        let mut samples = vec![0.0f64; SAMPLE_RATE as usize];
        osc.next_samples(&mut samples);
        let file = File::create(filename).unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    fn make_adsr() -> Adsr {
        Adsr {
            attack_secs: ATTACK_SECS,
            decay_secs: DECAY_SECS,
            sustain_level: SUSTAIN_LEVEL,
            release_secs: RELEASE_SECS,
        }
    }

    // Applies the envelope to note_on_samples, triggers note_off, then
    // generates release chunks until the envelope reaches Done.
    fn apply_envelope_with_release(
        source: &mut dyn SampleSource,
        env: &mut AdsrEnvelope,
        note_on_samples: usize,
    ) -> Vec<f64> {
        let mut samples = vec![0.0f64; note_on_samples];
        source.next_samples(&mut samples);
        env.apply(&mut samples);
        env.note_off();

        const CHUNK: usize = 256;
        while !env.is_done() {
            let mut chunk = vec![0.0f64; CHUNK];
            source.next_samples(&mut chunk);
            env.apply(&mut chunk);
            samples.extend_from_slice(&chunk);
        }
        samples
    }

    fn write_shape_with_envelope(shape: OscillatorShape, filename: &str) {
        let note_on_samples =
            ((ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS) * SAMPLE_RATE as f32) as usize;

        let mut osc = Oscillator::new(shape, FREQUENCY, SAMPLE_RATE);
        let mut env = AdsrEnvelope::new(make_adsr(), SAMPLE_RATE);
        let samples = apply_envelope_with_release(&mut osc, &mut env, note_on_samples);

        let file = File::create(filename).unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_sine() {
        write_shape(OscillatorShape::Sine, "middle_c_sine.wav");
    }

    #[test]
    fn test_square() {
        write_shape(OscillatorShape::Square, "middle_c_square.wav");
    }

    #[test]
    fn test_sawtooth() {
        write_shape(OscillatorShape::Sawtooth, "middle_c_sawtooth.wav");
    }

    #[test]
    fn test_triangle() {
        write_shape(OscillatorShape::Triangle, "middle_c_triangle.wav");
    }

    #[test]
    fn test_sine_envelope() {
        write_shape_with_envelope(OscillatorShape::Sine, "middle_c_sine_adsr.wav");
    }

    #[test]
    fn test_square_envelope() {
        write_shape_with_envelope(OscillatorShape::Square, "middle_c_square_adsr.wav");
    }

    #[test]
    fn test_sawtooth_envelope() {
        write_shape_with_envelope(OscillatorShape::Sawtooth, "middle_c_sawtooth_adsr.wav");
    }

    #[test]
    fn test_triangle_envelope() {
        write_shape_with_envelope(OscillatorShape::Triangle, "middle_c_triangle_adsr.wav");
    }

    #[test]
    fn test_cmaj7_chord() {
        // C4, E4, G4, B4
        let notes = [261.63, 329.63, 392.00, 493.88];
        let note_on_samples =
            ((ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS) * SAMPLE_RATE as f32) as usize;

        let mut mix = MixedSource::new();
        for freq in notes {
            mix.add_source_equal(Box::new(Oscillator::new(OscillatorShape::Sine, freq, SAMPLE_RATE)));
        }

        let mut env = AdsrEnvelope::new(make_adsr(), SAMPLE_RATE);
        let samples = apply_envelope_with_release(&mut mix, &mut env, note_on_samples);

        let file = File::create("cmaj7.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_graph_cmaj7() {
        let notes = [261.63, 329.63, 392.00, 493.88];
        let duration_secs = ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS;
        let adsr = || Adsr {
            attack_secs: ATTACK_SECS,
            decay_secs: DECAY_SECS,
            sustain_level: SUSTAIN_LEVEL,
            release_secs: RELEASE_SECS,
        };

        let node = NodeDef::Envelope {
            adsr: adsr(),
            duration_secs,
            source: Box::new(NodeDef::Mix {
                sources: notes.map(|freq| (
                    NodeDef::Oscillator { shape: OscillatorShape::Sine, frequency: freq },
                    0.25,
                )).into(),
            }),
        };

        let mut source = compile(node, SAMPLE_RATE);
        let mut samples = Vec::new();
        const CHUNK: usize = 256;
        while !source.is_done() {
            let mut chunk = vec![0.0f64; CHUNK];
            source.next_samples(&mut chunk);
            samples.extend_from_slice(&chunk);
        }

        let file = File::create("graph_cmaj7.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_voice() {
        let event = NoteEvent {
            frequency: FREQUENCY,
            velocity: 1.0,
            duration_secs: Some(ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS),
        };

        let node = NodeDef::Filter {
            kind: FilterType::LowPass,
            cutoff_hz: ParamDef::Const(1000.0),
            q: ParamDef::Const(0.707),
            source: Box::new(NodeDef::Oscillator {
                shape: OscillatorShape::Sine,
                frequency: FREQUENCY,
            }),
        };

        let mut voice = Voice::from_event(event, node, make_adsr(), SAMPLE_RATE);
        let mut samples = Vec::new();
        const CHUNK: usize = 256;
        while !voice.is_done() {
            let mut chunk = vec![0.0f64; CHUNK];
            voice.render(&mut chunk);
            samples.extend_from_slice(&chunk);
        }

        let file = File::create("voice.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    // --- Delay-based effects ---

    // Echo: short punchy note so each repeat lands in clear silence.
    // 400ms delay, 60% feedback → ~4 audible repeats fading out.
    #[test]
    fn test_echo() {
        let echo_adsr = Adsr {
            attack_secs: 0.01,
            decay_secs: 0.08,
            sustain_level: 0.3,
            release_secs: 0.05,
        };
        let note_secs = echo_adsr.attack_secs + echo_adsr.decay_secs + 0.1; // short hold
        let tail_secs = 2.5f32;
        let total_samples = ((note_secs + tail_secs) * SAMPLE_RATE as f32) as usize;

        let node = NodeDef::Delay {
            max_delay_secs: 0.45,
            delay_secs: ParamDef::Const(0.40),
            feedback: 0.60,
            mix: 0.50,
            source: Box::new(NodeDef::Envelope {
                adsr: echo_adsr,
                duration_secs: note_secs,
                source: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sine,
                    frequency: FREQUENCY,
                }),
            }),
        };

        let mut source = compile(node, SAMPLE_RATE);
        let mut samples = Vec::new();
        const CHUNK: usize = 256;
        let mut rendered = 0;
        while rendered < total_samples {
            let n = CHUNK.min(total_samples - rendered);
            let mut chunk = vec![0.0f64; n];
            source.next_samples(&mut chunk);
            samples.extend_from_slice(&chunk);
            rendered += n;
        }

        let file = File::create("echo.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    // Chorus: 15–25ms delay modulated by a 0.8 Hz LFO, no feedback.
    // Thickens the sound by slightly detuning a copy of the signal.
    #[test]
    fn test_chorus() {
        let note_secs = ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS;

        let node = NodeDef::Delay {
            max_delay_secs: 0.05,
            delay_secs: ParamDef::Signal {
                node: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sine,
                    frequency: 0.8,
                }),
                scale:  0.005,  // ±5ms modulation depth
                offset: 0.020,  // 20ms centre delay
            },
            feedback: 0.0,
            mix: 0.6,
            source: Box::new(NodeDef::Envelope {
                adsr: make_adsr(),
                duration_secs: note_secs,
                source: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sine,
                    frequency: FREQUENCY,
                }),
            }),
        };

        let mut source = compile(node, SAMPLE_RATE);
        let mut samples = Vec::new();
        const CHUNK: usize = 256;
        while !source.is_done() {
            let mut chunk = vec![0.0f64; CHUNK];
            source.next_samples(&mut chunk);
            samples.extend_from_slice(&chunk);
        }

        let file = File::create("chorus.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    // Flanger: 1–8ms delay modulated by a 0.3 Hz LFO, high feedback.
    // Creates a sweeping comb-filter effect.
    #[test]
    fn test_flanger() {
        let note_secs = ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS;

        let node = NodeDef::Delay {
            max_delay_secs: 0.02,
            delay_secs: ParamDef::Signal {
                node: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sine,
                    frequency: 0.3,
                }),
                scale:  0.0035, // ±3.5ms sweep
                offset: 0.0045, // 4.5ms centre delay
            },
            feedback: 0.7,
            mix: 0.7,
            source: Box::new(NodeDef::Envelope {
                adsr: make_adsr(),
                duration_secs: note_secs,
                source: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sawtooth,
                    frequency: FREQUENCY,
                }),
            }),
        };

        let mut source = compile(node, SAMPLE_RATE);
        let mut samples = Vec::new();
        const CHUNK: usize = 256;
        while !source.is_done() {
            let mut chunk = vec![0.0f64; CHUNK];
            source.next_samples(&mut chunk);
            samples.extend_from_slice(&chunk);
        }

        let file = File::create("flanger.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_voice_cmaj_scale() {
        // C major scale up then back down
        let scale: &[f64] = &[
            261.63, 293.66, 329.63, 349.23,
            392.00, 440.00, 493.88, 523.25,
            493.88, 440.00, 392.00, 349.23,
            329.63, 293.66, 261.63,
        ];

        let make_note_adsr = || Adsr {
            attack_secs: 0.02,
            decay_secs: 0.05,
            sustain_level: 0.7,
            release_secs: 0.03,
        };
        let note_hold_secs = 0.25f32;
        let gate_secs = make_note_adsr().attack_secs + make_note_adsr().decay_secs + note_hold_secs;

        let mut all_samples: Vec<f64> = Vec::new();

        for &freq in scale {
            let event = NoteEvent {
                frequency: freq,
                velocity: 1.0,
                duration_secs: Some(gate_secs),
            };

            // Sawtooth through an LPF whose cutoff is swept 200–3000 Hz by a 1 Hz LFO
            let node = NodeDef::Filter {
                kind: FilterType::LowPass,
                cutoff_hz: ParamDef::Signal {
                    node: Box::new(NodeDef::Oscillator {
                        shape: OscillatorShape::Sine,
                        frequency: 1.0,
                    }),
                    scale: 1400.0,
                    offset: 1600.0,
                },
                q: ParamDef::Const(1.2),
                source: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sawtooth,
                    frequency: freq,
                }),
            };

            let mut voice = Voice::from_event(event, node, make_note_adsr(), SAMPLE_RATE);
            const CHUNK: usize = 256;
            while !voice.is_done() {
                let mut chunk = vec![0.0f64; CHUNK];
                voice.render(&mut chunk);
                all_samples.extend_from_slice(&chunk);
            }
        }

        let file = File::create("cmaj_scale.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &all_samples).unwrap();
    }

    #[test]
    fn test_sine_lfo_filter_sweep() {
        let low_hz  = 200.0;
        let high_hz = 2000.0;
        let lfo_freq = 0.5; // Hz — one sweep per 2 seconds

        let node = NodeDef::Envelope {
            adsr: make_adsr(),
            duration_secs: ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS,
            source: Box::new(NodeDef::Filter {
                kind: FilterType::LowPass,
                cutoff_hz: ParamDef::Signal {
                    node: Box::new(NodeDef::Oscillator {
                        shape: OscillatorShape::Sine,
                        frequency: lfo_freq,
                    }),
                    scale:  (high_hz - low_hz) / 2.0,
                    offset: (high_hz + low_hz) / 2.0,
                },
                q: ParamDef::Const(2.0),
                source: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sine,
                    frequency: FREQUENCY,
                }),
            }),
        };

        let mut source = compile(node, SAMPLE_RATE);
        let mut samples = Vec::new();
        const CHUNK: usize = 256;
        while !source.is_done() {
            let mut chunk = vec![0.0f64; CHUNK];
            source.next_samples(&mut chunk);
            samples.extend_from_slice(&chunk);
        }

        let file = File::create("sine_lfo_filter_sweep.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_lfo_filter_sweep() {
        let duration_secs = ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS;

        let node = NodeDef::Envelope {
            adsr: make_adsr(),
            duration_secs,
            source: Box::new(NodeDef::Filter {
                kind: FilterType::LowPass,
                // LFO at 0.5 Hz sweeps cutoff between 200 Hz and 2200 Hz
                cutoff_hz: ParamDef::Signal {
                    node: Box::new(NodeDef::Oscillator {
                        shape: OscillatorShape::Sine,
                        frequency: 0.5,
                    }),
                    scale: 1000.0,
                    offset: 1200.0,
                },
                q: ParamDef::Const(2.0),
                source: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sawtooth,
                    frequency: FREQUENCY,
                }),
            }),
        };

        let mut source = compile(node, SAMPLE_RATE);
        let mut samples = Vec::new();
        const CHUNK: usize = 256;
        while !source.is_done() {
            let mut chunk = vec![0.0f64; CHUNK];
            source.next_samples(&mut chunk);
            samples.extend_from_slice(&chunk);
        }

        let file = File::create("lfo_filter_sweep.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_filtered() {
        let shapes = [
            (OscillatorShape::Sine,     "sine"),
            (OscillatorShape::Square,   "square"),
            (OscillatorShape::Sawtooth, "sawtooth"),
            (OscillatorShape::Triangle, "triangle"),
        ];
        let filters: [(FilterType, &str); 4] = [
            (FilterType::LowPass,  "lowpass"),
            (FilterType::HighPass, "highpass"),
            (FilterType::BandPass, "bandpass"),
            (FilterType::Notch,    "notch"),
        ];

        let note_on_samples = ((ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS) * SAMPLE_RATE as f32) as usize;
        let release_samples = (RELEASE_SECS * SAMPLE_RATE as f32) as usize;
        let total_samples = note_on_samples + release_samples;

        for (shape, shape_name) in shapes {
            for (filter_type, filter_name) in &filters {
                let mut osc = Oscillator::new(shape, FREQUENCY, SAMPLE_RATE);
                let mut samples = vec![0.0f64; total_samples];
                osc.next_samples(&mut samples);

                let mut env = AdsrEnvelope::new(Adsr {
                    attack_secs: ATTACK_SECS,
                    decay_secs: DECAY_SECS,
                    sustain_level: SUSTAIN_LEVEL,
                    release_secs: RELEASE_SECS,
                }, SAMPLE_RATE);
                env.apply(&mut samples[..note_on_samples]);
                env.note_off();
                env.apply(&mut samples[note_on_samples..]);

                let mut filt = BiquadFilter::new(*filter_type, 1000.0, 0.707, SAMPLE_RATE);
                filt.process(&mut samples);

                let filename = format!("middle_c_{}_{}.wav", shape_name, filter_name);
                let file = File::create(&filename).unwrap();
                write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
            }
        }
    }
}
