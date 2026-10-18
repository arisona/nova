//! Strata: a drone bed with air above it, a pulse and a space (see `doc/audio_concept.md`).
//!
//! The bed and the air follow the conductor's harmony; the pulse plays on its grid; the
//! drone (chord, air and reverb) follows the pulse sample by sample; the space adds the dub
//! delay, the reverb and distance. Void moves it from ambient to rhythmic and back into
//! the distance; Form from still layers to grain: the chord breaks into grains, the air
//! into dust.

pub mod pulse;
pub mod space;

use crate::dsp::{Curve, Rng};
use crate::harmony::Harmony;
use crate::params::Params;
use crate::score::{Pattern, Patterns, Score, Throws};
use crate::signals::{Change, Now, Signals};
use crate::voices::{Air, Bed};
use pulse::Pulse;
use space::Space;

pub use crate::voices::bed;

/// Strata's rhythm: thumps from a rhythm of 0.2, ticks from 0.35, stabs from 0.45 and the
/// chord gate from 0.25, each growing denser towards the full rhythm; no throws.
pub const SCORE: Score = Score {
    kick: Patterns {
        from: 0.2,
        list: &[Pattern::new("9... 3... 7... 5...")],
    },
    tick: Patterns {
        from: 0.35,
        list: &[Pattern::new("5.72 4.81 5.72 4.91")],
    },
    stab: Patterns {
        from: 0.45,
        list: &[Pattern::new(".... ..6. ..3. ..9.")],
    },
    bass: Patterns::NONE,
    bell: Patterns::NONE,
    gate: Patterns {
        from: 0.25,
        list: &[Pattern::new(".9.6 .8.. .9.5 .7.4")],
    },
    echo: &[3.0],
    ghost: 0.2,
    drop: 0.12,
    throws: Throws::NONE,
    fills: 0,
    breaks: 0,
};

/// Everything that sets Strata's character, as data: instruments that share this code
/// differ only in their presets.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub score: Score,
    pub bed: bed::Settings,
    pub pulse: pulse::Settings,
    pub space: space::Settings,
    /// How much of the air is dust over Form, from 0 to 1.
    pub dust: Curve,
    /// Output level in dB, so the instruments meet at the same loudness (see the sketches'
    /// report).
    pub level: f32,
}

impl Preset {
    pub const DEFAULT: Self = Self {
        score: SCORE,
        bed: bed::Settings {
            // Still layers, and grains above the middle.
            motion: Curve(&[[0.0, 0.35], [0.5, 1.0]]),
            grains: Curve(&[[0.5, 0.0], [1.0, 0.85]]),
            ..bed::Settings::DEFAULT
        },
        pulse: pulse::Settings::DEFAULT,
        space: space::Settings::DEFAULT,
        dust: Curve(&[[0.5, 0.0], [1.0, 0.75]]),
        level: -0.6,
    };

    /// Strata with ticks in loops of 12 steps and stabs in loops of 7, which phase against
    /// the bar.
    pub const POLYMETER: Self = Self {
        score: Score {
            tick: Patterns {
                from: 0.35,
                list: &[Pattern::new("5.8. 3.9. 4.7.")],
            },
            stab: Patterns {
                from: 0.45,
                list: &[Pattern::new("..9..6.")],
            },
            ..SCORE
        },
        ..Self::DEFAULT
    };
}

pub struct Strata {
    bed: Bed,
    air: Air,
    pulse: Pulse,
    space: Space,
    dust: Curve,
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
            dust: preset.dust,
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
        self.air.update_block(params, self.dust.at(params.form));
        self.pulse.update_block(params, signals);
        self.space
            .update_block(params, signals.tempo, signals.echo, 0.0);
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
