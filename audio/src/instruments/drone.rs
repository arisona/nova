//! The drone: bed, air, pulse and space, which every instrument plays (see Audio in the
//! README).
//!
//! It follows the conductor's harmony and updates its parts in order; each instrument sets
//! them up for the block, adds its own voices and mixes them in its own way.

use crate::dsp::Rng;
use crate::harmony::Harmony;
use crate::instruments::pulse::{self, Pulse};
use crate::instruments::space::{self, Space};
use crate::params::Params;
use crate::signals::{Change, Signals};
use crate::voices::{Air, Bed, bed};

pub struct Drone {
    pub bed: Bed,
    pub air: Air,
    pub pulse: Pulse,
    pub space: Space,
}

impl Drone {
    /// Starts from `harmony` until the conductor loads the first one; takes three seeds
    /// from `rng`.
    pub fn new(
        sample_rate: f32,
        bed: bed::Settings,
        pulse: pulse::Settings,
        space: space::Settings,
        harmony: &Harmony,
        rng: &mut Rng,
    ) -> Self {
        Self {
            bed: Bed::new(sample_rate, bed, harmony, Rng::new(rng.next_u64())),
            air: Air::new(sample_rate, harmony, Rng::new(rng.next_u64())),
            pulse: Pulse::new(sample_rate, pulse, harmony, Rng::new(rng.next_u64())),
            space: Space::new(sample_rate, space),
        }
    }

    /// First in every block: hands out the pulse as it is after the previous block, for the
    /// bed and anyone else, and follows a change of the harmony, which it returns for the
    /// instrument's own voices.
    pub fn follow(&mut self, signals: &mut Signals) -> Option<Change> {
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
        signals.change
    }

    /// Once the instrument has set the parts up for the block: `dust` is the share of dust
    /// in the air, `boost` throws the dub delay (see `voices/dub_delay.rs`).
    pub fn update_block(&mut self, params: &Params, signals: &mut Signals, dust: f32, boost: f32) {
        self.bed.update_block(params, signals);
        self.air.update_block(params, dust);
        self.pulse.update_block(params, signals);
        self.space
            .update_block(params, signals.tempo, signals.echo, boost);
    }
}
