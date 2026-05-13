mod envelope;
mod filter;
mod mixer;
mod oscillator;
mod wav;

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use crate::envelope::{Adsr, AdsrEnvelope};
    use crate::filter::{BiquadFilter, FilterType};
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

    fn write_shape_with_envelope(shape: OscillatorShape, filename: &str) {
        let note_on_samples =
            ((ATTACK_SECS + DECAY_SECS + SUSTAIN_HOLD_SECS) * SAMPLE_RATE as f32) as usize;
        let release_samples = (RELEASE_SECS * SAMPLE_RATE as f32) as usize;
        let total_samples = note_on_samples + release_samples;

        let mut osc = Oscillator::new(shape, FREQUENCY, SAMPLE_RATE);
        let mut samples = vec![0.0f64; total_samples];
        osc.next_samples(&mut samples);

        let mut env = AdsrEnvelope::new(
            Adsr {
                attack_secs: ATTACK_SECS,
                decay_secs: DECAY_SECS,
                sustain_level: SUSTAIN_LEVEL,
                release_secs: RELEASE_SECS,
            },
            SAMPLE_RATE,
        );
        env.apply(&mut samples[..note_on_samples]);
        env.note_off();
        env.apply(&mut samples[note_on_samples..]);

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
        let num_samples = (SAMPLE_RATE as f64 * 2.0) as usize;

        let mut mix = MixedSource::new();
        for freq in notes {
            mix.add_source_equal(Box::new(Oscillator::new(OscillatorShape::Sine, freq, SAMPLE_RATE)));
        }

        let mut samples = vec![0.0f64; num_samples];
        mix.next_samples(&mut samples);

        let file = File::create("cmaj7.wav").unwrap();
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
