//! Strata: a drone bed with air above it, a pulse and a space (see Audio in the README).
//!
//! The bed and the air follow the conductor's harmony; the pulse plays on its grid; the
//! drone (chord, air and reverb) follows the pulse sample by sample; the space adds the dub
//! delay, the reverb and distance. Void moves it from ambient to rhythmic and back into
//! the distance; Form from still layers to grain: the chord breaks into grains, the air
//! into dust. Sparse light fills the space with more air and hall.

use crate::dsp::{Curve, Rng};
use crate::harmony::Harmony;
use crate::instruments::drone::Drone;
use crate::instruments::{pulse, space};
use crate::params::Params;
use crate::score::{Pattern, Patterns, Score, Throws};
use crate::signals::{BAR, Now, Signals};
use crate::voices::bed;

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
    stabs: BAR,
    bass: Patterns::NONE,
    basses: &[],
    melody: Patterns::NONE,
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
    /// How sparse light fills the space: as the share of lit voxels falls from half to
    /// nothing, the air and the hall grow by up to this many dB (see `Features::density`);
    /// 0 ignores the density.
    pub sparse: f32,
    /// Output level in dB, so the instruments meet at the same loudness (see the sketches'
    /// report).
    pub level: f32,
}

impl Preset {
    pub const DEFAULT: Self = Self {
        score: SCORE,
        bed: bed::Settings {
            // Still layers, and grains above the middle.
            motion: Curve::new(&[[0.0, 0.35], [0.5, 1.0]]),
            grains: Curve::new(&[[0.5, 0.0], [1.0, 0.85]]),
            ..bed::Settings::DEFAULT
        },
        pulse: pulse::Settings::DEFAULT,
        space: space::Settings::DEFAULT,
        dust: Curve::new(&[[0.5, 0.0], [1.0, 0.75]]),
        sparse: 3.0,
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
    drone: Drone,
    dust: Curve,
    sparse: f32,
    reverb_level: f32,
    /// The gain of the air from sparse light.
    air_gain: f32,
}

impl Strata {
    /// Starts from `harmony` until the conductor loads the first one.
    pub fn new(sample_rate: f32, preset: &Preset, harmony: &Harmony, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            drone: Drone::new(
                sample_rate,
                preset.bed,
                preset.pulse,
                preset.space,
                harmony,
                &mut rng,
            ),
            dust: preset.dust,
            sparse: preset.sparse,
            reverb_level: preset.space.reverb_level,
            air_gain: 1.0,
        }
    }

    pub fn update_block(&mut self, params: &Params, signals: &mut Signals) {
        self.drone.follow(signals);
        // Sparse light fills the space, a complement rather than a mirror.
        self.air_gain = 10.0_f32.powf(self.sparse * params.light.sparseness() / 20.0);
        self.drone
            .space
            .set_reverb_level(self.reverb_level * self.air_gain);
        self.drone
            .update_block(params, signals, self.dust.at(params.form), 0.0);
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        let drone = &mut self.drone;
        let pulse = drone.pulse.next(now);
        // The drone follows the pulse: chord, air and reverb dip with each thump and with
        // the chord gate.
        let bed = drone.bed.next(pulse.drone);
        let air = drone
            .air
            .next()
            .map(|sample| sample * pulse.drone * self.air_gain);
        drone.space.process(bed, air, pulse)
    }

    /// Steps the pulse has played, per voice: thumps, ticks and stabs.
    pub fn steps(&self) -> [u64; 3] {
        self.drone.pulse.counts()
    }
}
