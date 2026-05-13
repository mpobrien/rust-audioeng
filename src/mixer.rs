use crate::oscillator::SampleSource;

pub struct MixedSource {
    sources: Vec<(Box<dyn SampleSource>, f64)>,
    tmp: Vec<f64>,
}

impl MixedSource {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            tmp: Vec::new(),
        }
    }

    pub fn add_source(&mut self, source: Box<dyn SampleSource>, gain: f64) {
        self.sources.push((source, gain));
    }

    pub fn add_source_equal(&mut self, source: Box<dyn SampleSource>) {
        let n = (self.sources.len() + 1) as f64;
        for (_, gain) in &mut self.sources {
            *gain = 1.0 / n;
        }
        self.sources.push((source, 1.0 / n));
    }
}

impl SampleSource for MixedSource {
    fn next_samples(&mut self, buf: &mut [f64]) {
        buf.fill(0.0);
        if self.tmp.len() < buf.len() {
            self.tmp.resize(buf.len(), 0.0);
        }
        for (source, gain) in &mut self.sources {
            let tmp = &mut self.tmp[..buf.len()];
            source.next_samples(tmp);
            for (o, s) in buf.iter_mut().zip(tmp.iter()) {
                *o += s * *gain;
            }
        }
    }
}

pub struct Mixer {
    tracks: Vec<(Vec<f64>, f64)>, // (samples, gain)
}

impl Mixer {
    pub fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    pub fn add_track(&mut self, samples: Vec<f64>, gain: f64) {
        self.tracks.push((samples, gain));
    }

    /// Add a track with gain automatically set to 1/n so all equal-gain
    /// tracks sum to at most 1.0.
    pub fn add_track_equal(&mut self, samples: Vec<f64>) {
        let n = (self.tracks.len() + 1) as f64;
        // rescale existing tracks to 1/n
        for (_, gain) in &mut self.tracks {
            *gain = 1.0 / n;
        }
        self.tracks.push((samples, 1.0 / n));
    }

    pub fn mix(&self) -> Vec<f64> {
        let len = self.tracks.iter().map(|(s, _)| s.len()).max().unwrap_or(0);
        let mut out = vec![0.0f64; len];
        for (samples, gain) in &self.tracks {
            for (o, s) in out.iter_mut().zip(samples.iter()) {
                *o += s * gain;
            }
        }
        out
    }
}
