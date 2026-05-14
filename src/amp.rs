use crate::filter::CompiledParam;
use crate::oscillator::SampleSource;

/// Scales a source signal by a level that can be constant or modulated.
pub struct AmplifiedSource {
    source: Box<dyn SampleSource>,
    level: CompiledParam,
}

impl AmplifiedSource {
    pub fn new(source: Box<dyn SampleSource>, level: CompiledParam) -> Self {
        Self { source, level }
    }
}

impl SampleSource for AmplifiedSource {
    fn next_samples(&mut self, buf: &mut [f64]) {
        self.source.next_samples(buf);
        for sample in buf.iter_mut() {
            *sample *= self.level.next_value();
        }
    }
}
