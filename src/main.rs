mod amp;
mod delay;
mod envelope;
mod output;
mod filter;
mod gate;
mod graph;
mod lang;
mod mixer;
mod oscillator;
mod voice;
mod wav;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    let watch = raw.iter().any(|a| a == "--watch");
    let positional: Vec<&str> = raw.iter()
        .filter(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .collect();

    if positional.is_empty() {
        eprintln!("usage: audioengine [--watch] <patch-file> [patch-name]");
        std::process::exit(1);
    }

    let path = positional[0];
    let patch_name: Option<String> = positional.get(1).map(|s| s.to_string());

    let sample_rate = output::device_sample_rate();

    if watch {
        run_watch(path, patch_name.as_deref(), sample_rate);
    } else {
        match load_and_play(path, patch_name.as_deref(), sample_rate) {
            Ok(audio) => audio.wait(),
            Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
        }
    }
}

// ── shared demo sequence ───────────────────────────────────────────────────────

fn demo_notes(note_secs: f32) -> Vec<(f64, f32, f32)> {
    // C major scale, two octaves up and back
    [
        261.63, 293.66, 329.63, 349.23, 392.00, 440.00, 493.88, 523.25,
        587.33, 659.26, 698.46, 783.99, 880.00,
        783.99, 698.46, 659.26, 587.33, 523.25, 493.88, 440.00,
        392.00, 349.23, 329.63, 293.66, 261.63,
    ].iter().map(|&f| (f, 0.8f32, note_secs)).collect()
}

/// Load, parse, and render to samples — but don't start audio yet.
/// Separating render from play lets the watch loop validate the new
/// version before it stops the currently-playing audio.
fn load_samples(
    path: &str,
    patch_name: Option<&str>,
    sample_rate: u32,
) -> Result<(String, Vec<f64>), String> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read file: {e}"))?;
    let env = lang::parse(&src)?;

    if env.patches.is_empty() {
        return Err(format!("no patches defined in '{path}'"));
    }

    let name = match patch_name {
        Some(n) => {
            if !env.patches.contains_key(n) {
                let avail = env.patches.keys().cloned().collect::<Vec<_>>().join(", ");
                return Err(format!("patch '{n}' not found (available: {avail})"));
            }
            n.to_string()
        }
        None => env.patches.keys().next().unwrap().clone(),
    };

    let notes = demo_notes(0.35);
    let samples = lang::render_patch(&env, &name, &notes, sample_rate)?;
    Ok((name, samples))
}

fn load_and_play(
    path: &str,
    patch_name: Option<&str>,
    sample_rate: u32,
) -> Result<output::AudioOutput, String> {
    let (name, samples) = load_samples(path, patch_name, sample_rate)?;
    println!("playing patch '{name}' at {sample_rate} Hz");
    Ok(output::play_samples(samples))
}

// ── watch mode ────────────────────────────────────────────────────────────────

fn run_watch(path: &str, patch_name: Option<&str>, sample_rate: u32) -> ! {
    use std::time::Duration;

    println!("watching '{path}' for changes (Ctrl-C to quit)");

    let poll = Duration::from_millis(150);
    let mut last_mtime = file_mtime(path);

    // Rendered samples for the current valid version — kept so we can loop
    // without re-parsing, and so a broken save doesn't interrupt playback.
    let mut current_samples: Option<Vec<f64>> = None;
    let mut current_audio:   Option<output::AudioOutput> = None;

    // Initial load
    match load_samples(path, patch_name, sample_rate) {
        Ok((name, samples)) => {
            println!("playing patch '{name}'");
            current_audio   = Some(output::play_samples(samples.clone()));
            current_samples = Some(samples);
        }
        Err(e) => eprintln!("error: {e}"),
    }

    loop {
        std::thread::sleep(poll);

        // ── file changed? ──────────────────────────────────────────────────
        let m = file_mtime(path);
        if m != last_mtime {
            last_mtime = m;

            match load_samples(path, patch_name, sample_rate) {
                Ok((name, samples)) => {
                    // New version is valid: swap out audio atomically.
                    drop(current_audio.take());
                    println!("─ reloaded patch '{name}'");
                    current_audio   = Some(output::play_samples(samples.clone()));
                    current_samples = Some(samples);
                }
                Err(e) => {
                    // Bad save: keep playing what we had, print the problem.
                    eprintln!("─ error (keeping previous version):\n{e}");
                }
            }
        }

        // ── playback finished? loop it ─────────────────────────────────────
        if current_audio.as_ref().map_or(false, |a| a.is_done()) {
            drop(current_audio.take());
            if let Some(ref samples) = current_samples {
                current_audio = Some(output::play_samples(samples.clone()));
            }
        }
    }
}

fn file_mtime(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod snap {
    use std::path::Path;

    const BLOCKS: usize = 10;
    const APPROX_TOL: f64 = 1e-6;

    struct Snapshot {
        samples: usize,
        peak: f64,
        rms: f64,
        rms_blocks: Vec<f64>,
        hash: u64,
    }

    /// Fail if the hash changes — any bit difference is a regression.
    pub fn assert_snapshot(name: &str, samples: &[f64]) {
        check(name, samples, false);
    }

    /// Fail only if the stats diverge beyond tolerance — ignores tiny FP deltas.
    #[allow(dead_code)]
    pub fn assert_snapshot_approx(name: &str, samples: &[f64]) {
        check(name, samples, true);
    }

    fn check(name: &str, samples: &[f64], approx: bool) {
        let snap_dir = Path::new("tests/snapshots");
        std::fs::create_dir_all(snap_dir).unwrap();
        let path = snap_dir.join(format!("{name}.snap"));
        let actual = compute(samples);
        if path.exists() {
            let stored = load(&path);
            if stored.hash == actual.hash {
                return;
            }
            let diffs = stat_diffs(&stored, &actual);
            if approx && diffs.is_empty() {
                return;
            }
            fail(name, &stored, &actual, approx, &diffs);
        } else {
            save(&path, &actual);
            println!("snapshot written: {}", path.display());
        }
    }

    fn compute(samples: &[f64]) -> Snapshot {
        let n = samples.len();
        let peak = samples.iter().copied().map(f64::abs).fold(0f64, f64::max);
        let rms = (samples.iter().map(|s| s * s).sum::<f64>() / n as f64).sqrt();
        let rms_blocks = (0..BLOCKS)
            .map(|i| {
                let lo = i * n / BLOCKS;
                let hi = ((i + 1) * n / BLOCKS).min(n);
                let b = &samples[lo..hi];
                (b.iter().map(|s| s * s).sum::<f64>() / b.len() as f64).sqrt()
            })
            .collect();
        Snapshot { samples: n, peak, rms, rms_blocks, hash: fnv1a(samples) }
    }

    fn fnv1a(samples: &[f64]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for &s in samples {
            for byte in s.to_bits().to_le_bytes() {
                h ^= byte as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
        h
    }

    fn save(path: &Path, s: &Snapshot) {
        let blocks = s.rms_blocks.iter().map(|v| format!("{v:.8}")).collect::<Vec<_>>().join(" ");
        std::fs::write(
            path,
            format!(
                "samples = {}\npeak = {:.8}\nrms = {:.8}\nrms_blocks = {}\nhash = {:#018x}\n",
                s.samples, s.peak, s.rms, blocks, s.hash
            ),
        )
        .unwrap();
    }

    fn load(path: &Path) -> Snapshot {
        let src = std::fs::read_to_string(path).unwrap();
        let mut samples = 0usize;
        let mut peak = 0f64;
        let mut rms = 0f64;
        let mut rms_blocks = Vec::new();
        let mut hash = 0u64;
        for line in src.lines() {
            let Some((k, v)) = line.split_once(" = ") else { continue };
            match k {
                "samples"    => samples = v.parse().unwrap(),
                "peak"       => peak = v.parse().unwrap(),
                "rms"        => rms = v.parse().unwrap(),
                "rms_blocks" => rms_blocks = v.split(' ').map(|s| s.parse().unwrap()).collect(),
                "hash"       => hash = u64::from_str_radix(v.trim_start_matches("0x"), 16).unwrap(),
                _ => {}
            }
        }
        Snapshot { samples, peak, rms, rms_blocks, hash }
    }

    fn stat_diffs(stored: &Snapshot, actual: &Snapshot) -> Vec<String> {
        let mut diffs = Vec::new();
        if stored.samples != actual.samples {
            diffs.push(format!("  samples    {} → {}", stored.samples, actual.samples));
        }
        if (stored.peak - actual.peak).abs() > APPROX_TOL {
            diffs.push(format!("  peak       {:.6} → {:.6}", stored.peak, actual.peak));
        }
        if (stored.rms - actual.rms).abs() > APPROX_TOL {
            diffs.push(format!("  rms        {:.6} → {:.6}", stored.rms, actual.rms));
        }
        for (i, (s, a)) in stored.rms_blocks.iter().zip(&actual.rms_blocks).enumerate() {
            if (s - a).abs() > APPROX_TOL {
                diffs.push(format!("  block[{i:02}]   {s:.6} → {a:.6}"));
            }
        }
        diffs
    }

    fn fail(name: &str, stored: &Snapshot, actual: &Snapshot, approx: bool, diffs: &[String]) -> ! {
        let mut msg = format!("snapshot mismatch: '{name}'");
        if diffs.is_empty() {
            msg.push_str("\n  (hash changed but stats are within tolerance)");
        } else {
            for d in diffs {
                msg.push('\n');
                msg.push_str(d);
            }
        }
        msg.push_str(&format!(
            "\n  hash       {:#018x} → {:#018x}",
            stored.hash, actual.hash
        ));
        if approx && !diffs.is_empty() || !approx {
            msg.push_str(&format!(
                "\n  (delete tests/snapshots/{name}.snap to regenerate)"
            ));
        }
        panic!("{msg}");
    }
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

    // --- Real-time output ---

    #[test]
    #[ignore = "requires audio hardware; run with: cargo test test_realtime_cmaj_scale -- --ignored"]
    fn test_realtime_cmaj_scale() {
        use crate::output::play_samples;

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
        let gate_secs = make_note_adsr().attack_secs + make_note_adsr().decay_secs + 0.25;

        let mut all_samples: Vec<f64> = Vec::new();

        for &freq in scale {
            let event = NoteEvent {
                frequency: freq,
                velocity: 1.0,
                duration_secs: Some(gate_secs),
            };
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

        play_samples(all_samples).wait();
    }

    #[test]
    #[ignore = "requires audio hardware; run with: cargo test test_realtime -- --ignored"]
    fn test_realtime() {
        use crate::output::play;

        let node = NodeDef::Envelope {
            adsr: make_adsr(),
            duration_secs: ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS,
            source: Box::new(NodeDef::Filter {
                kind: FilterType::LowPass,
                cutoff_hz: ParamDef::Signal {
                    node: Box::new(NodeDef::Oscillator {
                        shape: OscillatorShape::Sine,
                        frequency: 0.5,
                    }),
                    scale: 1000.0,
                    offset: 1200.0,
                },
                q: ParamDef::Const(1.5),
                source: Box::new(NodeDef::Oscillator {
                    shape: OscillatorShape::Sawtooth,
                    frequency: FREQUENCY,
                }),
            }),
        };

        play(node).wait();
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

    // ── patchlang tests ──

    #[test]
    fn test_lang_bass_patch() {
        use crate::lang::{parse, render_patch};

        let src = r#"
            bass = patch {
              voices = mono
              osc { shape = saw, freq = freq }
                | lpf  { cutoff = 800, q = 1.5 }
                | adsr { attack = 0.02, decay = 0.1, sustain = 0.7, release = 0.15 }
            }
        "#;

        let env = parse(src).unwrap();

        // C major scale, each note 0.3 s
        let scale: &[(f64, f32, f32)] = &[
            (261.63, 0.8, 0.3),
            (293.66, 0.8, 0.3),
            (329.63, 0.8, 0.3),
            (349.23, 0.8, 0.3),
            (392.00, 0.8, 0.3),
            (440.00, 0.8, 0.3),
            (493.88, 0.8, 0.3),
            (523.25, 0.8, 0.3),
        ];

        let samples = render_patch(&env, "bass", scale, SAMPLE_RATE).unwrap();
        crate::snap::assert_snapshot("lang_bass", &samples);

        let file = File::create("lang_bass.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_lang_supersaw_patch() {
        use crate::lang::{parse, render_patch};

        let src = r#"
            supersaw = patch {
              voices = poly 4
              osc1 = osc { shape = saw, freq = freq }
              osc2 = osc { shape = saw, freq = freq * 1.006 }
              body = mix { osc1, osc2 }
                   | lpf { cutoff = 2000, q = 0.8 }
                   | adsr { attack = 0.3, decay = 0.1, sustain = 0.8, release = 0.5 }
              out = body * velocity
            }
        "#;

        let env = parse(src).unwrap();

        // A minor chord (A, C, E)
        let notes: &[(f64, f32, f32)] = &[
            (440.00, 0.9, 1.0),
            (523.25, 0.9, 1.0),
            (659.25, 0.9, 1.0),
        ];

        let samples = render_patch(&env, "supersaw", notes, SAMPLE_RATE).unwrap();
        crate::snap::assert_snapshot("lang_supersaw", &samples);

        let file = File::create("lang_supersaw.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_lang_effect_chain() {
        use crate::lang::{parse, render_patch};

        let src = r#"
            space = effect {
              delay { time = 0.375, feedback = 0.45, mix = 0.4 }
            }

            lead = patch {
              osc { shape = sine, freq = freq }
                | adsr { attack = 0.05, decay = 0.1, sustain = 0.8, release = 0.25 }
                | space
            }
        "#;

        let env = parse(src).unwrap();

        let notes: &[(f64, f32, f32)] = &[
            (440.00, 0.8, 0.4),
            (493.88, 0.8, 0.4),
            (523.25, 0.8, 0.4),
        ];

        let samples = render_patch(&env, "lead", notes, SAMPLE_RATE).unwrap();
        crate::snap::assert_snapshot("lang_lead_space", &samples);

        let file = File::create("lang_lead_space.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_lang_lfo_minmax() {
        use crate::lang::{parse, render_patch};

        // Inline lfo with min/max: cutoff sweeps 200–2000 Hz at 0.5 Hz
        let src = r#"
            wub = patch {
              osc { shape = saw, freq = freq }
                | lpf { cutoff = lfo { rate = 0.5, min = 200, max = 2000 }, q = 1.2 }
                | adsr { attack = 0.02, decay = 0.1, sustain = 0.8, release = 0.2 }
            }
        "#;

        let env = parse(src).unwrap();
        let notes: &[(f64, f32, f32)] = &[(220.0, 0.8, 0.8), (330.0, 0.8, 0.8)];
        let samples = render_patch(&env, "wub", notes, SAMPLE_RATE).unwrap();
        crate::snap::assert_snapshot("lang_lfo_minmax", &samples);

        let file = File::create("lang_lfo_minmax.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_lang_lfo_scale_offset() {
        use crate::lang::{parse, render_patch};

        // Inline lfo with scale/offset: same sweep as above, different syntax
        let src = r#"
            wub = patch {
              osc { shape = saw, freq = freq }
                | lpf { cutoff = lfo { rate = 0.5, scale = 900, offset = 1100 }, q = 1.2 }
                | adsr { attack = 0.02, decay = 0.1, sustain = 0.8, release = 0.2 }
            }
        "#;

        let env = parse(src).unwrap();
        let notes: &[(f64, f32, f32)] = &[(220.0, 0.8, 0.8), (330.0, 0.8, 0.8)];
        let samples = render_patch(&env, "wub", notes, SAMPLE_RATE).unwrap();
        crate::snap::assert_snapshot("lang_lfo_scale_offset", &samples);

        let file = File::create("lang_lfo_scale_offset.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_lang_lfo_named_binding() {
        use crate::lang::{parse, render_patch};

        // Named binding for lfo, then referenced by name in lpf cutoff param
        let src = r#"
            wub = patch {
              sweep = lfo { rate = 0.8, min = 300, max = 1800 }
              osc { shape = saw, freq = freq }
                | lpf { cutoff = sweep, q = 1.0 }
                | adsr { attack = 0.02, decay = 0.1, sustain = 0.8, release = 0.2 }
            }
        "#;

        let env = parse(src).unwrap();
        let notes: &[(f64, f32, f32)] = &[(220.0, 0.8, 0.8), (330.0, 0.8, 0.8)];
        let samples = render_patch(&env, "wub", notes, SAMPLE_RATE).unwrap();
        crate::snap::assert_snapshot("lang_lfo_named", &samples);

        let file = File::create("lang_lfo_named.wav").unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_lang_lfo_mixed_params_error() {
        use crate::lang::parse;

        // Mixing min/max with scale/offset should be a build-time error
        let src = r#"
            bad = patch {
              osc { shape = saw, freq = freq }
                | lpf { cutoff = lfo { rate = 1.0, min = 200, max = 2000, scale = 900 }, q = 1.0 }
                | adsr { attack = 0.01, decay = 0.1, sustain = 0.8, release = 0.1 }
            }
        "#;

        let env = parse(src).unwrap();
        let notes: &[(f64, f32, f32)] = &[(440.0, 0.8, 0.5)];
        let result = crate::lang::render_patch(&env, "bad", notes, SAMPLE_RATE);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("cannot mix min/max and scale/offset"), "unexpected error: {msg}");
    }

    #[test]
    fn test_lang_sine_lfo_filter_sweep() {
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
