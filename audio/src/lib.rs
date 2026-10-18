//! The sound of the Nova voxel display: an engine for drone and ambient sound, played with
//! the same controls as the light (see Audio in the README, and `doc/audio_concept.md` for
//! what is open).
//!
//! It knows nothing of the host that runs it, nor of the display: it plays from the bus
//! (`nova_core::Bus`), which carries the controls, the tide clock and the features of the
//! light, and leaves its status there. Layered by role, each layer using only those before
//! it:
//!
//! - `dsp`: building blocks (transfer curves, oscillators, filters, envelopes, delay lines,
//!   a limiter)
//! - `voices`: sound sources and effects (bed, air, thump, tick, stab, sub, bell, pluck,
//!   organ, dub delay, reverb)
//! - `instruments`: voices played together, with their character as a preset
//! - `engine`: the conductor's shared musical time and harmony, the instrument, the master
//! - `io`: the native output
//!
//! Beside them: `params` (the smoothed controls and light the voices read), `harmony` (the
//! tonal world of a palette), `score` (an instrument's rhythm, which the conductor plays)
//! and `signals` (what the engine's parts tell each other).
//!
//! The DSP is plain code, without heavily generic abstractions, which can run hundreds of
//! times slower in debug builds and make the stream drop out.

pub mod dsp;
mod engine;
mod harmony;
mod instruments;
mod io;
mod params;
mod score;
mod signals;
mod voices;

/// Samples between control updates: the engine's block.
const BLOCK: usize = 32;

pub use engine::{Activity, Engine};
pub use harmony::{Color, Harmony, MAX_COLORS, Palette, Tone};
pub use instruments::{DEFAULT_INSTRUMENT, INSTRUMENTS, Preset, bayou, fathom, shoal, strata};
pub use io::Output;
pub use score::{Pattern, Patterns, Score, Throws};
pub use signals::{Change, Signals};
