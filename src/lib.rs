pub mod amp;
pub mod delay;
pub mod envelope;
pub mod filter;
pub mod gate;
pub mod graph;
pub mod lang;
pub mod mixer;
pub mod oscillator;
pub mod voice;

use wasm_bindgen::prelude::*;

/// Render a named phrase from patchlang source to f32 PCM samples (mono, 44100 Hz).
/// Returns an empty array on parse or render error.
#[wasm_bindgen]
pub fn render_phrase_wasm(src: &str, phrase_name: &str, sample_rate: u32) -> Vec<f32> {
    let env = match lang::parse(src) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    match lang::render_phrase(&env, phrase_name, sample_rate) {
        Ok(samples) => samples.into_iter().map(|s| s as f32).collect(),
        Err(_) => vec![],
    }
}
