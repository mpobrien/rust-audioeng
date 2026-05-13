use crate::oscillator::SampleSource;

pub struct FilteredSource {
    source: Box<dyn SampleSource>,
    filter: BiquadFilter,
}

impl FilteredSource {
    pub fn new(source: Box<dyn SampleSource>, filter: BiquadFilter) -> Self {
        Self { source, filter }
    }
}

impl SampleSource for FilteredSource {
    fn next_samples(&mut self, buf: &mut [f64]) {
        self.source.next_samples(buf);
        self.filter.process(buf);
    }
}

#[derive(Clone, Copy)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

struct Coeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

pub struct BiquadFilter {
    coeffs: Coeffs,
    // state for "transposed direct form II"
    s1: f64,
    s2: f64,
}

impl BiquadFilter {
    pub fn new(filter_type: FilterType, cutoff_hz: f64, q: f64, sample_rate: u32) -> Self {
        Self {
            coeffs: compute_coeffs(filter_type, cutoff_hz, q, sample_rate),
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn process(&mut self, buf: &mut [f64]) {
        for sample in buf.iter_mut() {
            *sample = self.tick(*sample);
        }
    }

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.coeffs.b0 * x + self.s1;
        self.s1 = self.coeffs.b1 * x - self.coeffs.a1 * y + self.s2;
        self.s2 = self.coeffs.b2 * x - self.coeffs.a2 * y;
        y
    }
}

/// Use the algorithm from RBJ cookbook to convert cutoff/q/sample rate into
/// biquad coefficients.
fn compute_coeffs(filter_type: FilterType, cutoff_hz: f64, q: f64, sample_rate: u32) -> Coeffs {
    let w0 = 2.0 * std::f64::consts::PI * cutoff_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let (b0, b1, b2, a0, a1, a2) = match filter_type {
        FilterType::LowPass => (
            (1.0 - cos_w0) / 2.0,
            1.0 - cos_w0,
            (1.0 - cos_w0) / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::HighPass => (
            (1.0 + cos_w0) / 2.0,
            -(1.0 + cos_w0),
            (1.0 + cos_w0) / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::BandPass => (
            sin_w0 / 2.0,
            0.0,
            -sin_w0 / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::Notch => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
    };

    Coeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}
