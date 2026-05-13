mod oscillator;
mod wav;

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::BufWriter;
    use crate::oscillator::{Oscillator, OscillatorShape, SampleSource};
    use crate::wav::write_wav;

    const SAMPLE_RATE: u32 = 44100;
    const FREQUENCY: f64 = 261.63;

    fn write_shape(shape: OscillatorShape, filename: &str) {
        let mut osc = Oscillator::new(shape, FREQUENCY, SAMPLE_RATE);
        let mut samples = vec![0.0f64; SAMPLE_RATE as usize];
        osc.next_samples(&mut samples);
        let file = File::create(filename).unwrap();
        write_wav(&mut BufWriter::new(file), 1, SAMPLE_RATE, &samples).unwrap();
    }

    #[test]
    fn test_sine()     { write_shape(OscillatorShape::Sine,     "middle_c_sine.wav");     }

    #[test]
    fn test_square()   { write_shape(OscillatorShape::Square,   "middle_c_square.wav");   }

    #[test]
    fn test_sawtooth() { write_shape(OscillatorShape::Sawtooth, "middle_c_sawtooth.wav"); }

    #[test]
    fn test_triangle() { write_shape(OscillatorShape::Triangle, "middle_c_triangle.wav"); }
}
