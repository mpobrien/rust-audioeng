use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig, SupportedStreamConfig};
use ringbuf::HeapRb;

use phogbank::graph::{NodeDef, compile};
use phogbank::oscillator::SampleSource;

/// Frames held in the ring buffer between the render thread and audio callback.
/// ~185ms at 44100 Hz — enough headroom to absorb scheduling jitter without
/// adding perceptible latency.
const RING_BUFFER_FRAMES: usize = 8192;

/// Frames the render thread produces per iteration.
const RENDER_CHUNK: usize = 256;

/// A live audio stream. Keeps the CPAL stream and render thread alive.
/// Dropping this stops playback immediately. Call [`wait`](Self::wait) to
/// block until the source finishes naturally.
pub struct AudioOutput {
    _stream: Stream,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    sample_rate: u32,
}

impl AudioOutput {
    /// Block until the source signals `is_done()`, then return.
    /// Sleeps one ring-buffer duration after the render thread exits so the
    /// callback has time to drain before the stream is dropped.
    pub fn wait(mut self) {
        if let Some(h) = self.thread.take() {
            h.join().ok();
        }
        let drain_ms = (RING_BUFFER_FRAMES as u64 * 1000) / self.sample_rate as u64;
        thread::sleep(Duration::from_millis(drain_ms + 50));
    }

    /// Returns `true` once the render thread has finished (all samples written
    /// to the ring buffer). The callback may still be draining the last frames.
    pub fn is_done(&self) -> bool {
        self.thread.as_ref().map_or(true, |h| h.is_finished())
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.thread.take() {
            h.join().ok();
        }
    }
}

/// Compile `node` using the device's actual sample rate and start real-time
/// audio output.
pub fn play(node: NodeDef) -> AudioOutput {
    let (device, supported) = open_default_device();
    let source = compile(node, supported.sample_rate().0);
    start_stream(source, device, supported)
}

/// Play a pre-rendered sample buffer in real time.
///
/// The buffer should have been rendered at the same sample rate as the output
/// device (typically 44100 or 48000 Hz). A mismatch causes a pitch shift.
pub fn play_samples(samples: Vec<f64>) -> AudioOutput {
    let (device, supported) = open_default_device();
    start_stream(Box::new(BufferPlayer::new(samples)), device, supported)
}

// --- internals ---

/// Returns the sample rate of the system's default output device.
pub fn device_sample_rate() -> u32 {
    open_default_device().1.sample_rate().0
}

fn open_default_device() -> (cpal::Device, SupportedStreamConfig) {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");
    let supported = device
        .default_output_config()
        .expect("no default output config");
    (device, supported)
}

fn start_stream(
    source: Box<dyn SampleSource>,
    device: cpal::Device,
    supported: SupportedStreamConfig,
) -> AudioOutput {
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();

    let rb = HeapRb::<f32>::new(RING_BUFFER_FRAMES);
    let (mut producer, mut consumer) = rb.split();

    let stop = Arc::new(AtomicBool::new(false));
    let stop_render = Arc::clone(&stop);

    // Audio callback — runs on the driver's high-priority thread.
    // Must not allocate, block, or do I/O.
    let stream = match sample_format {
        SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                for frame in data.chunks_mut(channels) {
                    let s = consumer.pop().unwrap_or(0.0); // underrun → silence
                    frame.iter_mut().for_each(|ch| *ch = s);
                }
            },
            |e| eprintln!("audio stream error: {e}"),
            None,
        ),
        fmt => panic!("unsupported sample format {fmt:?}"),
    }
    .expect("failed to build output stream");

    stream.play().expect("failed to start audio stream");

    // Render thread — pulls from the source and feeds the ring buffer.
    // Sleeps briefly when the buffer is full to avoid busy-waiting.
    let thread = thread::spawn(move || {
        let mut source = source;
        let mut chunk = vec![0.0f64; RENDER_CHUNK];

        while !stop_render.load(Ordering::Relaxed) && !source.is_done() {
            source.next_samples(&mut chunk);
            for &s in &chunk {
                while producer.push(s as f32).is_err() {
                    if stop_render.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_micros(200));
                }
            }
        }
    });

    AudioOutput {
        _stream: stream,
        stop,
        thread: Some(thread),
        sample_rate,
    }
}

/// Wraps a pre-rendered `Vec<f64>` buffer as a [`SampleSource`].
struct BufferPlayer {
    samples: Vec<f64>,
    pos: usize,
}

impl BufferPlayer {
    fn new(samples: Vec<f64>) -> Self {
        Self { samples, pos: 0 }
    }
}

impl SampleSource for BufferPlayer {
    fn next_samples(&mut self, buf: &mut [f64]) {
        for out in buf.iter_mut() {
            *out = if self.pos < self.samples.len() {
                let s = self.samples[self.pos];
                self.pos += 1;
                s
            } else {
                0.0
            };
        }
    }

    fn is_done(&self) -> bool {
        self.pos >= self.samples.len()
    }
}
