mod output;
mod synth;

// The instrument is rendered offline by the sketches until it replaces the live synth
// (see doc/audio_concept.md).
#[cfg_attr(not(test), allow(dead_code))]
mod dsp;
#[cfg_attr(not(test), allow(dead_code))]
mod features;
#[cfg_attr(not(test), allow(dead_code))]
mod instrument;
#[cfg(test)]
mod sketches;

pub use output::AudioService;
pub use synth::{AudioControls, Synth};
