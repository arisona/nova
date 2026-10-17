//! Instruments: voices played together, each interpreting the same controls in its own way.
//! An instrument's character is a preset, plain data, so several instruments can share one
//! implementation.
//!
//! Instruments are an enum rather than trait objects: plain code that stays fast in debug
//! builds, where the audio has to keep up in real time too.

pub mod fathom;
pub mod strata;

use crate::harmony::Harmony;
use crate::params::Params;
use crate::score::Score;
use crate::signals::{Now, Signals};
use fathom::Fathom;
use strata::Strata;

/// Which instrument to play, and how.
// Presets are constants, copied only when an engine starts: their size does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug)]
pub enum Preset {
    Strata(strata::Preset),
    Fathom(fathom::Preset),
}

/// The instruments to choose from, by name, in alphabetical order.
pub const INSTRUMENTS: [(&str, Preset); 2] = [
    ("Fathom", Preset::Fathom(fathom::Preset::DEFAULT)),
    ("Strata", Preset::Strata(strata::Preset::DEFAULT)),
];

/// The instrument new installations play.
pub const DEFAULT_INSTRUMENT: &str = "Strata";

impl Default for Preset {
    fn default() -> Self {
        Self::Strata(strata::Preset::DEFAULT)
    }
}

impl Preset {
    /// Its rhythm, which the conductor plays.
    pub fn score(&self) -> Score {
        match self {
            Self::Strata(preset) => preset.score,
            Self::Fathom(preset) => preset.score,
        }
    }

    /// The instrument of that name in `INSTRUMENTS`, if there is one.
    pub fn named(name: &str) -> Option<Self> {
        INSTRUMENTS
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, preset)| *preset)
    }
}

/// Boxed, since instruments are large.
pub enum Instrument {
    Strata(Box<Strata>),
    Fathom(Box<Fathom>),
}

impl Instrument {
    /// Starts from `harmony` until the conductor loads the first one; `seed` varies the
    /// details.
    pub fn new(preset: &Preset, sample_rate: f32, harmony: &Harmony, seed: u64) -> Self {
        match preset {
            Preset::Strata(preset) => {
                Self::Strata(Box::new(Strata::new(sample_rate, preset, harmony, seed)))
            }
            Preset::Fathom(preset) => {
                Self::Fathom(Box::new(Fathom::new(sample_rate, preset, harmony, seed)))
            }
        }
    }

    /// Once per block, after the conductor: reads the signals and writes what others may
    /// follow.
    pub fn update_block(&mut self, params: &Params, signals: &mut Signals) {
        match self {
            Self::Strata(strata) => strata.update_block(params, signals),
            Self::Fathom(fathom) => fathom.update_block(params, signals),
        }
    }

    /// The stereo mix at the next sample, before the master.
    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        match self {
            Self::Strata(strata) => strata.next(now),
            Self::Fathom(fathom) => fathom.next(now),
        }
    }

    /// Steps played so far: thumps, ticks and stabs.
    pub fn steps(&self) -> [u64; 3] {
        match self {
            Self::Strata(strata) => strata.steps(),
            Self::Fathom(fathom) => fathom.steps(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruments_are_found_by_name() {
        assert!(matches!(
            Preset::named(DEFAULT_INSTRUMENT),
            Some(Preset::Strata(_))
        ));
        assert!(matches!(Preset::named("Fathom"), Some(Preset::Fathom(_))));
        assert!(Preset::named("Theremin").is_none());
    }
}
