//! Fathom's chord: how the bed's chord tones are played as dub stabs.
//!
//! The chord is one paraphonic voice, which plays the stabs of the score (see `SCORE` in
//! `fathom.rs`). Each stab opens a resonant low-pass
//! on the chord and closes it again, cold like struck glass that rings as it fades, warm
//! like a round "bwow" that closes faster than it fades. Only the stab goes to the dub
//! delay, so the echoes carry the groove; between stabs a quiet residual of the chord stays.
//! A slow ride moves the filter like a hand on the cutoff knob, opening while the bass is
//! away from home and closing when it returns.
//!
//! Throws push the delay for a moment, on the stabs the conductor throws (see
//! `score::Throws`).
//!
//! Below the first pattern, the rhythm (from Void) gives a pure drone through the low-pass,
//! as loud as Strata's; above it, swells every second bar, which turn into stabs. Form sets
//! their length, from long, legato chords at the layers to short, clipped ones at the grain. Like the
//! pulse, the chord rests towards Flow 0, and as the rhythm dissolves, the stabs move into
//! their echoes.

use crate::BLOCK;
use crate::dsp::{
    Contour, Curve, Drift, Rng, coefficient, fold, lerp, resonance, smooth, smoothstep,
};
use crate::harmony::{CROSSFADE_SECONDS, Harmony};
use crate::params::Params;
use crate::signals::{Now, Signals};
use crate::voices::bed::Filter;

/// Highest cutoff, so cold pings stay soft in the highs.
const MAX_CUTOFF: f32 = 3000.0;

/// How much stabs vary, so a groove of them does not tire the ear: each opens the filter up
/// to this share less, and hits up to half of it softer.
const VARIATION: f32 = 0.2;

/// Seconds the delay boost takes to rise.
const BOOST_RISE: f32 = 0.05;

/// Share of a stab's dry sound left once the rhythm has dissolved: the rest of it is heard
/// only in its echoes.
const DISSOLVED_DRY: f32 = 0.3;

/// Each pair when cold and warm.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Peak level of a stab, relative to the drone; lower when warm, where stabs last
    /// longer. Swells, which sit on the full drone, rise only by `swell_level`.
    pub stab_level: [f32; 2],
    pub swell_level: f32,
    /// Level of the chord between stabs at a full rhythm, relative to the drone.
    pub residual: f32,
    /// Lowest cutoff in Hz for a chord whose lowest tone is at 240 Hz; it follows the chord
    /// by half its interval.
    pub floor: f32,
    /// How far above the floor the drone's filter sits, in octaves.
    pub drone_offset: [f32; 2],
    /// How far a stab and a swell open the filter, in octaves.
    pub opening: [f32; 2],
    pub swell_opening: f32,
    /// Resonance in dB at the cutoff, of the drone and of stabs.
    pub drone_resonance: [f32; 2],
    pub stab_resonance: [f32; 2],
    /// Above this cutoff, in Hz, the resonance falls away.
    pub resonance_corner: f32,
    /// How much faster the filter closes than the level fades: 1 follows it, 3 closes three
    /// times as fast.
    pub follow: [f32; 2],
    /// Stab envelope: attack in seconds, hold in 16ths, and release time constant in seconds
    /// at 100 BPM (it follows the tempo).
    pub attack: [f32; 2],
    pub hold: [f32; 2],
    pub release: [f32; 2],
    /// How long stabs are over Form, as a factor of their attack, hold and release: long and
    /// legato at the layers, short and clipped at the grain.
    pub length: Curve,
    /// Swell envelope: attack in seconds, hold in 16ths, release time constant in seconds.
    pub swell: [f32; 3],
    /// How far the slow ride moves the filter either way, in octaves.
    pub ride: f32,
    /// How far the height of the light moves the filter, in octaves from the bottom to the
    /// top: light up high opens it, light down low closes it, light in the middle leaves
    /// it (see `Features::height`); 0 ignores the height.
    pub height: f32,
    /// Send of a stab to the dub delay; thrown stabs send fully.
    pub send: [f32; 2],
    /// How far off centre the light must sit, from 0 to 1, for the echoes to start on the
    /// side away from it (see `Features::balance`): they answer across the room. Within
    /// that, they stay where they started, on the left at first; `None` ignores the balance.
    pub echo_side: Option<f32>,
    /// How sparse light fills the space: as the share of lit voxels falls from half to
    /// nothing, the delay's feedback rises by up to this much (see `Features::density`; the
    /// score throws more too); 0 ignores the density.
    pub sparse: f32,
}

impl Settings {
    pub const DEFAULT: Self = Self {
        stab_level: [2.4, 2.0],
        swell_level: 0.5,
        residual: 0.14,
        floor: 300.0,
        drone_offset: [1.8, 1.0],
        opening: [2.4, 1.9],
        swell_opening: 1.5,
        drone_resonance: [4.0, 2.0],
        stab_resonance: [6.0, 3.0],
        resonance_corner: 1800.0,
        follow: [1.0, 3.0],
        attack: [0.0015, 0.005],
        hold: [0.25, 0.45],
        release: [0.07, 0.16],
        length: Curve::new(&[[0.0, 2.5], [0.5, 1.0], [1.0, 0.45]]),
        swell: [0.5, 4.0, 1.2],
        ride: 0.8,
        height: 1.0,
        send: [0.3, 0.45],
        echo_side: Some(0.1),
        sparse: 0.06,
    };
}

/// The chord's gains at one sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gains {
    /// For the chord between stabs, which follows the drone's dip.
    pub residual: f32,
    /// For the stab, on top of the residual.
    pub dry: f32,
    /// For the chord into the dub delay.
    pub send: f32,
}

/// Interpolates on a logarithmic scale, for times.
fn log_lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (b / a).powf(t)
}

pub struct Chord {
    sample_rate: f32,
    settings: Settings,
    register: [f32; 2],
    rng: Rng,
    /// The latest smoothed parameters, and the tempo.
    params: Params,
    tempo: f32,
    away: bool,
    /// Stabs in the current bar.
    stabs: usize,
    /// How sparse the light is, from 0 (half lit or more) to 1 (dark), and the feedback it
    /// adds to the delay.
    sparseness: f32,
    space: f32,
    /// Whether the echoes start on the right.
    echo_right: bool,
    /// How far the chord plays, from 0 at Flow 0 (resting) to 1.
    rest: f32,
    /// Whether the first block has set the slewed values.
    primed: bool,
    contour: Contour,
    velocity: f32,
    /// How far this stab opens the filter, as a share of the full opening.
    reach: f32,
    thrown: bool,
    stab_level: f32,
    morph: f32,
    residual: f32,
    drift: Drift,
    arc: f32,
    /// Lowest chord tone, for the filter's key follow, gliding to a new chord's.
    lowest: f32,
    lowest_target: f32,
    filter: Filter,
    boost: f32,
    boost_peak: f32,
    boost_hold: f32,
    boost_release: f32,
    send: f32,
    send_target: f32,
    /// The send of an ordinary stab now.
    send_level: f32,
    send_slew: f32,
    count: u64,
}

impl Chord {
    /// `register` is the range the bed folds the chord into.
    pub fn new(
        sample_rate: f32,
        settings: Settings,
        register: [f32; 2],
        harmony: &Harmony,
        mut rng: Rng,
    ) -> Self {
        let mut chord = Self {
            sample_rate,
            settings,
            register,
            drift: Drift::new(&mut rng),
            rng,
            params: Params::default(),
            tempo: 100.0,
            away: false,
            stabs: 0,
            sparseness: 0.0,
            space: 0.0,
            echo_right: false,
            rest: 1.0,
            primed: false,
            contour: Contour::default(),
            velocity: 0.0,
            reach: 1.0,
            thrown: false,
            stab_level: 0.0,
            morph: 0.0,
            residual: 1.0,
            arc: -1.0,
            lowest: register[0],
            lowest_target: register[0],
            filter: Filter {
                cutoff: 1000.0,
                resonance: 0.0,
                reference: [1000.0, 0.0],
            },
            boost: 0.0,
            boost_peak: 0.0,
            boost_hold: 0.0,
            boost_release: 1.0,
            send: 0.0,
            send_target: 0.0,
            send_level: 0.0,
            send_slew: 1.0 - (-1.0 / (0.005 * sample_rate)).exp(),
            count: 0,
        };
        chord.set_harmony(harmony, true);
        chord
    }

    /// Follows a new chord, for the filter's key follow: at once if `snap`, otherwise
    /// gliding along with the crossfade, so the old chord's filter does not jump.
    pub fn set_harmony(&mut self, harmony: &Harmony, snap: bool) {
        let [low, high] = self.register;
        self.lowest_target = harmony.tones()[1..]
            .iter()
            .map(|tone| fold(tone.frequency, low, high))
            .fold(high, f32::min);
        if snap {
            self.lowest = self.lowest_target;
        }
    }

    /// The low-pass on the chord now.
    pub fn filter(&self) -> Filter {
        self.filter
    }

    /// Where the echoes start: 0 on the left, 1 on the right.
    pub fn echo_start(&self) -> f32 {
        self.echo_right as u8 as f32
    }

    /// How hard the dub delay is thrown now, with what sparse light adds.
    pub fn boost(&self) -> f32 {
        self.boost + self.space
    }

    /// From the drone (0) to stabs (1), following the rhythm slowly.
    pub fn morph(&self) -> f32 {
        self.morph
    }

    /// Stabs and swells played so far.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reads tempo, away and the stabs of the bar from the signals.
    pub fn update_block(&mut self, params: &Params, signals: &Signals) {
        let settings = &self.settings;
        let block_seconds = BLOCK as f32 / self.sample_rate;
        let block_rate = 1.0 / block_seconds;
        self.params = *params;
        self.tempo = signals.tempo;
        self.away = signals.away;
        self.stabs = signals.stabs;
        let bar_seconds = 240.0 / self.tempo.max(1.0);
        let warmth = smooth(params.heat);

        // Drone to stabs, slewed over a bar; towards Flow 0 the chord rests, like the pulse.
        self.rest = smoothstep(0.0, 0.15, params.flow_setting);
        let morph = smoothstep(0.2, 0.42, params.rhythm) * self.rest;
        let residual = lerp(
            1.0,
            settings.residual,
            smoothstep(0.2, 0.6, params.rhythm) * self.rest,
        );
        let slew = if self.primed {
            coefficient(bar_seconds, block_rate)
        } else {
            self.primed = true;
            1.0
        };
        self.morph += (morph - self.morph) * slew;
        self.residual += (residual - self.residual) * slew;
        self.lowest *= (self.lowest_target / self.lowest)
            .powf(coefficient(CROSSFADE_SECONDS / 3.0, block_rate));

        // The slow hand on the cutoff knob, opening while the bass is away.
        let wander = self
            .drift
            .advance((0.015 + 0.05 * params.flow) * block_seconds, &mut self.rng);
        let arc_step = 2.0 * block_seconds / (4.0 * bar_seconds);
        let arc = if self.away { 1.0 } else { -1.0 };
        self.arc += (arc - self.arc).clamp(-arc_step, arc_step);
        let ride = 0.6 * (2.0 * wander - 1.0) + 0.4 * self.arc;

        let morph = self.morph;
        let floor = (settings.floor * (self.lowest / 240.0).sqrt()).clamp(200.0, 450.0);
        let offset = lerp(settings.drone_offset[0], settings.drone_offset[1], warmth);
        let height = settings.height * (params.light.height - 0.5);
        let base = floor * 2.0_f32.powf(offset * (1.0 - morph) + settings.ride * ride + height);
        let opening = lerp(
            settings.swell_opening,
            lerp(settings.opening[0], settings.opening[1], warmth),
            morph,
        );
        let decibels = lerp(
            lerp(
                settings.drone_resonance[0],
                settings.drone_resonance[1],
                warmth,
            ),
            lerp(
                settings.stab_resonance[0],
                settings.stab_resonance[1],
                warmth,
            ),
            morph,
        );
        let corner = settings.resonance_corner;
        let tamed = |cutoff: f32| resonance(decibels) / (1.0 + (cutoff / corner).powi(2));
        let follow = lerp(settings.follow[0], settings.follow[1], warmth);
        let envelope = self.contour.value().powf(follow);
        let lift = (0.6 + 0.4 * self.velocity) * self.reach;
        let cutoff = (base * 2.0_f32.powf(opening * lift * envelope)).clamp(80.0, MAX_CUTOFF);
        // The chord keeps its loudness at the filter's most open state, a full stab; the
        // drone keeps it as the filter is, so its ride and its swells' opening only brighten
        // it.
        let open = (base * 2.0_f32.powf(opening * lerp(lift * envelope, 1.0, morph)))
            .clamp(80.0, MAX_CUTOFF);
        self.filter = Filter {
            cutoff,
            resonance: tamed(cutoff),
            reference: [open, tamed(open)],
        };

        // The delay boost rises quickly, holds, and releases over bars.
        if self.boost_hold > 0.0 {
            self.boost_hold -= block_seconds;
            self.boost =
                (self.boost + self.boost_peak * block_seconds / BOOST_RISE).min(self.boost_peak);
        } else {
            self.boost *= (-block_seconds / self.boost_release).exp();
        }

        self.send_level =
            lerp(settings.send[0], settings.send[1], warmth) * (0.75 + 0.5 * params.void);
        // Sparse light fills the space, a complement rather than a mirror.
        self.sparseness = params.light.sparseness();
        self.space = settings.sparse * self.sparseness;
        // Echoes answer the light across the room.
        if let Some(threshold) = settings.echo_side {
            let balance = params.light.balance;
            if balance < -threshold {
                self.echo_right = true;
            } else if balance > threshold {
                self.echo_right = false;
            }
        }
        self.send_target = if self.thrown { 1.0 } else { self.send_level };
    }

    /// A stab of `velocity`, thrown if asked.
    fn trigger(&mut self, velocity: f32, throw: Option<[f32; 3]>) {
        let settings = &self.settings;
        self.thrown = throw.is_some();
        if let Some([peak, hold, release]) = throw {
            let bar_seconds = 240.0 / self.tempo.max(1.0);
            // A smaller throw never cuts a bigger one short.
            self.boost_peak = peak.max(self.boost);
            self.boost_hold = hold * bar_seconds;
            self.boost_release = release * bar_seconds;
        }
        // The previous stab's send must not carry over into this one's attack.
        self.send_target = if self.thrown { 1.0 } else { self.send_level };
        if !self.thrown {
            self.send = self.send_target;
        }

        let warmth = smooth(self.params.heat);
        let morph = self.morph;
        let sixteenth = 15.0 / self.tempo.max(1.0);
        let [swell_attack, swell_hold, swell_release] = settings.swell;
        let length = settings.length.at(self.params.form);
        let attack = log_lerp(
            swell_attack,
            lerp(settings.attack[0], settings.attack[1], warmth) * length,
            morph,
        );
        let hold = log_lerp(
            swell_hold,
            lerp(settings.hold[0], settings.hold[1], warmth) * length,
            morph,
        ) * sixteenth;
        let release = log_lerp(
            swell_release,
            lerp(settings.release[0], settings.release[1], warmth) * length * 100.0
                / self.tempo.max(1.0),
            morph,
        ) * if self.thrown { 1.5 } else { 1.0 };
        // The stab's gain stays continuous: the new velocity takes over from where the
        // previous stab is.
        let current = self.velocity * self.stab_level * self.contour.value();
        // Sparse stabs hit harder, so the loudness holds as the patterns thin out.
        let density = lerp(1.0, (4.0 / self.stabs.max(1) as f32).powf(0.25), morph);
        self.stab_level = lerp(
            settings.swell_level,
            lerp(settings.stab_level[0], settings.stab_level[1], warmth) * density,
            morph,
        );
        self.reach = 1.0 - VARIATION * self.rng.unit();
        self.velocity = velocity
            * (0.85 + 0.3 * self.params.liveliness)
            * (1.0 - 0.5 * VARIATION * self.rng.unit())
            * if self.thrown { 1.15 } else { 1.0 };
        let start = current / (self.velocity * self.stab_level).max(1e-6);
        self.contour
            .trigger_from(start, attack, hold, release, self.sample_rate);
        self.count += 1;
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> Gains {
        if now.hits.stab > 0.0 {
            self.trigger(now.hits.stab, now.hits.throw);
        }
        let envelope = self.contour.next();
        self.send += (self.send_target - self.send) * self.send_slew;
        let stab = self.velocity * envelope * self.rest;
        let dry = 1.0 - (1.0 - DISSOLVED_DRY) * self.params.dissolve;
        Gains {
            residual: self.residual,
            dry: stab * self.stab_level * dry,
            send: stab * self.send,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Conductor;
    use crate::harmony::Palette;
    use crate::harmony::tests::palette;
    use crate::instruments::fathom::SCORE;
    use crate::score::{Score, Throws};
    use crate::signals::BAR;
    use nova_core::Controls;
    use nova_core::Features;

    const RATE: f32 = 8000.0;

    #[test]
    fn stabs_leave_the_beats_to_the_kick() {
        for (index, pattern) in SCORE.stab.list.iter().enumerate() {
            for step in 0..64 {
                // The coldest pattern plays only the off-beat eighths.
                let free = if index == 0 {
                    step % 4 == 2
                } else {
                    step % 4 != 0
                };
                assert!(free || pattern.weight(step) == 0, "{index}, step {step}");
            }
        }
    }

    /// Plays the chord on the conductor's grid for `seconds`, with half the voxels lit;
    /// returns the stabs, the thrown stabs, the largest boost and the latest gains.
    fn play(rhythm: f32, flow: f32, throws: Throws, seconds: f32) -> (u64, u64, f32, Gains) {
        play_away(rhythm, flow, throws, seconds, None, 0.5)
    }

    /// Like `play`, with the bass held away from home or at home if given, and the given
    /// share of lit voxels.
    fn play_away(
        rhythm: f32,
        flow: f32,
        throws: Throws,
        seconds: f32,
        away: Option<bool>,
        density: f32,
    ) -> (u64, u64, f32, Gains) {
        let palettes: [Palette; 1] = [palette()];
        let harmony = Harmony::from_palette(&palettes[0]);
        // Every stab of the patterns, not only the strongest of each bar.
        let score = Score {
            throws,
            stabs: BAR,
            ..SCORE
        };
        let mut conductor = Conductor::new(RATE, harmony, score, 1);
        if let Some(away) = away {
            conductor.hold_away(away);
        }
        let mut signals = Signals::new(harmony);
        let mut chord = Chord::new(
            RATE,
            Settings::DEFAULT,
            [180.0, 720.0],
            &harmony,
            Rng::new(1),
        );
        let params = Params {
            rhythm,
            flow,
            flow_setting: flow,
            light: Features {
                density,
                ..Features::NEUTRAL
            },
            ..Params::default()
        };
        let (mut thrown, mut boost, mut stab) = (0, 0.0_f32, Gains::default());
        for frame in 0..(seconds * RATE) as usize {
            if frame % BLOCK == 0 {
                conductor.update_block(&params, &Controls::default(), 0.0, &palettes, &mut signals);
                chord.update_block(&params, &signals);
                boost = boost.max(chord.boost());
                let filter = chord.filter();
                assert!((80.0..=MAX_CUTOFF).contains(&filter.cutoff));
                assert!(filter.resonance >= 0.0 && filter.reference[1] >= 0.0);
            }
            let count = chord.count();
            stab = chord.next(&conductor.next());
            if chord.count() > count && chord.thrown {
                thrown += 1;
            }
        }
        (chord.count(), thrown, boost, stab)
    }

    #[test]
    fn form_sets_how_long_stabs_last() {
        // Samples a stab stays above a quarter of its peak, on average.
        let sounding = |form: f32| {
            let palettes: [Palette; 1] = [palette()];
            let harmony = Harmony::from_palette(&palettes[0]);
            let mut conductor = Conductor::new(RATE, harmony, SCORE, 1);
            let mut signals = Signals::new(harmony);
            let mut chord = Chord::new(
                RATE,
                Settings::DEFAULT,
                [180.0, 720.0],
                &harmony,
                Rng::new(1),
            );
            let params = Params {
                form,
                ..Params::default()
            };
            let mut levels = Vec::new();
            for frame in 0..(RATE as usize * 12) {
                if frame % BLOCK == 0 {
                    conductor.update_block(
                        &params,
                        &Controls::default(),
                        0.0,
                        &palettes,
                        &mut signals,
                    );
                    chord.update_block(&params, &signals);
                }
                levels.push(chord.next(&conductor.next()).dry);
            }
            let peak = levels.iter().copied().fold(0.0, f32::max);
            let above = levels.iter().filter(|&&level| level > 0.25 * peak).count();
            above as f32 / chord.count() as f32
        };
        let [legato, neutral, clipped] = [0.0, 0.5, 1.0].map(sounding);
        assert!(
            legato > 1.5 * neutral && neutral > 1.5 * clipped,
            "{legato}, {neutral}, {clipped}"
        );
    }

    #[test]
    fn rhythm_and_flow_decide_whether_stabs_play() {
        let (stabs, _, _, stab) = play(0.05, 0.5, Throws::SHORT, 20.0);
        assert_eq!(stabs, 0, "a drone without rhythm");
        assert!((stab.residual - 1.0).abs() < 1e-3);
        let (stabs, _, _, stab) = play(0.9, 0.0, Throws::SHORT, 20.0);
        assert_eq!(stabs, 0, "resting at Flow 0");
        assert!(stab.residual > 0.99);
        let (swells, _, _, _) = play(0.2, 0.5, Throws::SHORT, 20.0);
        let (stabs, _, _, stab) = play(0.9, 0.5, Throws::SHORT, 20.0);
        assert!(
            swells > 0 && stabs > 4 * swells,
            "{swells} swells, {stabs} stabs"
        );
        assert!(stab.residual < 1.5 * Settings::DEFAULT.residual);
    }

    #[test]
    fn stabs_take_over_from_each_other_without_steps() {
        // Right after the rhythm jumps up, the stabs still have long holds and overlap, and their
        // velocities alternate.
        let rate = 48_000.0;
        let palettes: [Palette; 1] = [palette()];
        let harmony = Harmony::from_palette(&palettes[0]);
        let mut conductor = Conductor::new(rate, harmony, SCORE, 1);
        let mut signals = Signals::new(harmony);
        let mut chord = Chord::new(
            rate,
            Settings::DEFAULT,
            [180.0, 720.0],
            &harmony,
            Rng::new(1),
        );
        let (mut peak, mut steepest, mut previous) = (0.0_f32, 0.0_f32, 0.0_f32);
        for frame in 0..(rate as usize * 12) {
            let params = Params {
                rhythm: if frame < rate as usize * 4 { 0.1 } else { 0.9 },
                heat: 1.0,
                ..Params::default()
            };
            if frame % BLOCK == 0 {
                conductor.update_block(&params, &Controls::default(), 0.0, &palettes, &mut signals);
                chord.update_block(&params, &signals);
            }
            let dry = chord.next(&conductor.next()).dry;
            peak = peak.max(dry);
            steepest = steepest.max((dry - previous).abs());
            previous = dry;
        }
        assert!(steepest < 0.02 * peak, "{steepest} of {peak}");
    }

    #[test]
    fn phrases_throw_the_delay_tunably() {
        let (stabs, short, boost, _) = play(0.9, 0.5, Throws::SHORT, 60.0);
        let (_, long, _, _) = play(0.9, 0.5, Throws::LONG, 60.0);
        // About 20 bars a minute at 80 BPM: a throw every four bars, or every sixteen.
        assert!((4..=8).contains(&short), "{short} of {stabs}");
        assert!(long < short && long >= 1, "{long}");
        assert!(
            boost > 0.15 && boost <= Throws::SHORT.section[0] + 1e-6,
            "{boost}"
        );
        let never = Throws {
            phrase_bars: 0,
            loose: 0.0,
            ..Throws::SHORT
        };
        for away in [false, true] {
            let (_, none, boost, _) = play_away(0.9, 0.5, never, 60.0, Some(away), 0.5);
            assert_eq!((none, boost), (0, 0.0), "away {away}");
        }
        let (_, away, _, _) = play_away(0.9, 0.5, Throws::SHORT, 60.0, Some(true), 0.5);
        assert!(away > short, "twice as often while away: {away} vs {short}");

        // Sparse light fills the space: more feedback and throws of its own.
        let feedback = Settings::DEFAULT.sparse;
        let (_, sparse, boost, _) = play_away(0.9, 0.5, never, 60.0, Some(false), 0.0);
        assert!(sparse > 0 && boost > feedback, "{sparse}, {boost}");
    }
}
