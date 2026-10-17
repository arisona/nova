//! Strata: a drone bed with air above it, a pulse and a space (see `doc/audio_concept.md`).
//!
//! The bed and the air follow the conductor's harmony; the pulse plays on its grid; the
//! drone (chord, air and reverb) follows the pulse sample by sample; the space adds the dub
//! delay, the reverb and distance. Form moves it from ambient to rhythmic.

pub mod pulse;
pub mod space;

use crate::dsp::Rng;
use crate::harmony::Harmony;
use crate::params::Params;
use crate::signals::{Change, Now, Signals};
use crate::voices::{Air, Bed};
use pulse::Pulse;
use space::Space;

pub use crate::voices::bed;
pub use pulse::Rhythm;

/// Everything that sets Strata's character, as data: instruments that share this code
/// differ only in their presets.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub bed: bed::Settings,
    pub pulse: pulse::Settings,
    pub space: space::Settings,
}

impl Preset {
    pub const DEFAULT: Self = Self {
        bed: bed::Settings::DEFAULT,
        pulse: pulse::Settings::DEFAULT,
        space: space::Settings::DEFAULT,
    };

    /// This preset with another rhythm.
    pub const fn with_rhythm(mut self, rhythm: Rhythm) -> Self {
        self.pulse.rhythm = rhythm;
        self
    }
}

pub struct Strata {
    bed: Bed,
    air: Air,
    pulse: Pulse,
    space: Space,
}

impl Strata {
    /// Starts from `harmony` until the conductor loads the first one.
    pub fn new(sample_rate: f32, preset: &Preset, harmony: &Harmony, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            bed: Bed::new(sample_rate, preset.bed, harmony, Rng::new(rng.next_u64())),
            air: Air::new(sample_rate, harmony, Rng::new(rng.next_u64())),
            pulse: Pulse::new(sample_rate, preset.pulse, harmony, Rng::new(rng.next_u64())),
            space: Space::new(sample_rate, preset.space),
        }
    }

    pub fn update_block(&mut self, params: &Params, signals: &mut Signals) {
        // The pulse as it is after the previous block, for the bed and anyone else.
        self.pulse.publish(signals);
        let harmony = &signals.harmony;
        match signals.change {
            Some(Change::Load) => {
                self.bed.load(harmony);
                self.air.load(harmony);
                self.pulse.adopt(harmony);
            }
            Some(Change::Palette) => {
                self.bed.change(harmony);
                self.air.change(harmony);
                self.pulse.change(harmony);
            }
            Some(Change::Bass) => {
                self.bed.set_bass(harmony.root());
                self.air.change(harmony);
                self.pulse.change(harmony);
            }
            Some(Change::Voice) => self.bed.move_voice(),
            None => {}
        }
        self.bed.update_block(params, signals);
        self.air.update_block(params);
        self.pulse.update_block(params, signals);
        self.space.update_block(params, signals.tempo, 0.0);
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        let pulse = self.pulse.next(now);
        // The drone follows the pulse: chord, air and reverb dip with each thump and with
        // the chord gate.
        let bed = self.bed.next(pulse.drone);
        let air = self.air.next().map(|sample| sample * pulse.drone);
        self.space.process(bed, air, pulse)
    }

    /// Steps the pulse has played, per voice: thumps, ticks and stabs.
    pub fn steps(&self) -> [u64; 3] {
        self.pulse.counts()
    }
}
