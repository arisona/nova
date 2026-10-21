//! What the engine's parts tell each other: the signals, once per block, and the musical
//! time, once per sample.
//!
//! The signals are a plain struct with one field per signal rather than a modulation
//! matrix. The conductor writes musical time and harmony, the instruments write
//! what others may follow (how far the drone dips with the thumps, the chord gate, the bass
//! accent), and every instrument reads what it needs. Within an instrument, voices may still
//! hand each other signals at audio rate.

use crate::harmony::Harmony;

/// Steps per bar (16ths).
pub const BAR: usize = 16;

/// Bars per phrase.
pub const PHRASE: usize = 4;

/// What changed in the harmony at the start of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The first harmony: take it at once.
    Load,
    /// A new palette: crossfade to the new tones (over `harmony::CROSSFADE_SECONDS`).
    Palette,
    /// The bass moves to a new note: glide to it.
    Bass,
    /// Halfway through a section: one chord tone changes its octave.
    Voice,
}

#[derive(Clone, Copy, Debug)]
pub struct Signals {
    // From the conductor.
    /// Tempo in beats per minute.
    pub tempo: f32,
    /// The current harmony, over the current bass.
    pub harmony: Harmony,
    /// What changed in the harmony with this block, if anything.
    pub change: Option<Change>,
    /// Whether the bass is away from home: tension rather than release.
    pub away: bool,
    /// How loose the rhythm is, from 0 (strict) to 1, from the Looseness tide.
    pub looseness: f32,
    /// Whether the visual activity rose suddenly (see `engine::Activity`).
    pub onset: bool,
    /// Stabs in the current bar.
    pub stabs: usize,
    /// Time of the dub delay in 16ths.
    pub echo: f32,

    // From the instruments.
    /// How far the drone dips with the thumps, from 0 to 1.
    pub dip: f32,
    /// How far the chord gate dips the chord, from 0 to 1.
    pub gate: f32,
    /// The bass's accent on thumps, from 0 to 1.
    pub accent: f32,
}

impl Signals {
    pub fn new(harmony: Harmony) -> Self {
        Self {
            tempo: 0.0,
            harmony,
            change: None,
            away: false,
            looseness: 0.0,
            onset: false,
            stabs: 0,
            echo: 3.0,
            dip: 0.0,
            gate: 0.0,
            accent: 0.0,
        }
    }
}

/// The musical time at one sample, from the conductor: a grid of 16ths, in pairs whose
/// second half starts late when the rhythm swings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Now {
    /// The 16th that starts at this sample, counted from the start, if one does.
    pub step: Option<u64>,
    /// How far into the current 16th, from 0 to 1.
    pub progress: f32,
    /// What the score plays on that 16th.
    pub hits: Hits,
}

/// What the score plays on a 16th: a velocity per part, 0 where it rests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hits {
    pub kick: f32,
    pub tick: f32,
    /// A stab, and how long it holds in 16ths where the score writes it, 0 where not.
    pub stab: f32,
    pub hold: u8,
    /// A note of the bass line, and its frequency ratio to the root.
    pub bass: f32,
    pub note: f32,
    /// A note of the melody, and the degree of the chord it plays, from 1.
    pub melody: f32,
    pub degree: u8,
    /// The chord gate closes.
    pub gate: f32,
    /// The stab is thrown into the dub delay: boost, hold and release in bars.
    pub throw: Option<[f32; 3]>,
}
