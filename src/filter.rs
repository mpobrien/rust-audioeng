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

pub struct ModulatedFilterSource {
    source: Box<dyn SampleSource>,
    filter: BiquadFilter,
    cutoff: CompiledParam,
    q: CompiledParam,
}

impl ModulatedFilterSource {
    pub fn new(
        source: Box<dyn SampleSource>,
        filter: BiquadFilter,
        cutoff: CompiledParam,
        q: CompiledParam,
    ) -> Self {
        Self { source, filter, cutoff, q }
    }
}

impl SampleSource for ModulatedFilterSource {
    fn next_samples(&mut self, buf: &mut [f64]) {
        self.source.next_samples(buf);
        for sample in buf.iter_mut() {
            let cutoff = self.cutoff.next_value();
            let q = self.q.next_value();
            self.filter.set_cutoff_q(cutoff, q);
            *sample = self.filter.tick(*sample);
        }
    }
}

pub enum CompiledParam {
    Const(f64),
    Signal { source: Box<dyn SampleSource>, scale: f64, offset: f64 },
    /// output = source * depth * scale + offset
    ModSignal { source: Box<dyn SampleSource>, depth: Box<dyn SampleSource>, scale: f64, offset: f64 },
}

impl CompiledParam {
    pub fn next_value(&mut self) -> f64 {
        match self {
            CompiledParam::Const(v) => *v,
            CompiledParam::Signal { source, scale, offset } => {
                let mut buf = [0.0f64; 1];
                source.next_samples(&mut buf);
                buf[0] * *scale + *offset
            }
            CompiledParam::ModSignal { source, depth, scale, offset } => {
                let mut buf = [0.0f64; 1];
                let mut dep = [0.0f64; 1];
                source.next_samples(&mut buf);
                depth.next_samples(&mut dep);
                buf[0] * dep[0] * *scale + *offset
            }
        }
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
    filter_type: FilterType,
    sample_rate: u32,
    coeffs: Coeffs,
    s1: f64,
    s2: f64,
}

impl BiquadFilter {
    pub fn new(filter_type: FilterType, cutoff_hz: f64, q: f64, sample_rate: u32) -> Self {
        Self {
            filter_type,
            sample_rate,
            coeffs: compute_coeffs(filter_type, cutoff_hz, q, sample_rate),
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn set_cutoff_q(&mut self, cutoff_hz: f64, q: f64) {
        self.coeffs = compute_coeffs(self.filter_type, cutoff_hz, q, self.sample_rate);
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
