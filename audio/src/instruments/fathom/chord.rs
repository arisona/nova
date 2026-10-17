//! Fathom's chord: how the bed's chord tones are played as dub stabs.
//!
//! The chord is one paraphonic voice. Stabs land on off-beat eighths (even 16ths, so swing
//! never moves them, and never the downbeat, which belongs to the kick), in patterns that
//! grow denser with Form and change only at bar starts. Each stab opens a resonant low-pass
//! on the chord and closes it again, cold like struck glass that rings as it fades, warm
//! like a round "bwow" that closes faster than it fades. Only the stab goes to the dub
//! delay, so the echoes carry the groove; between stabs a quiet residual of the chord stays.
//! A slow ride moves the filter like a hand on the cutoff knob, opening while the bass is
//! away from home and closing when it returns.
//!
//! Throws push the delay for a moment: the last stab of a phrase, the first stab after the
//! bass or the palette changes, after a sudden rise in visual activity, and now and then
//! when the rhythm loosens. How often is tunable (`Throws`), for short or long listening.
//!
//! Below the first pattern, Form gives a pure drone through the low-pass, as loud as
//! Strata's; above it, swells every second bar, which turn into stabs. Like the pulse, the
//! chord rests towards Flow 0.

use crate::BLOCK;
use crate::dsp::{Contour, Drift, Rng, coefficient, lerp, resonance, smooth, smoothstep};
use crate::harmony::{CROSSFADE_SECONDS, Harmony};
use crate::params::Params;
use crate::signals::{BAR, Now, Signals};
use crate::voices::bed::Filter;

/// Steps of a bar as a bit set.
const fn steps(list: &[usize]) -> u16 {
    let mut mask = 0;
    let mut index = 0;
    while index < list.len() {
        mask |= 1 << list[index];
        index += 1;
    }
    mask
}

/// Stab steps at home, per pattern level: swells (every second bar), then stabs ever more
/// often, up to all four off-beat eighths. While the bass is away, every pattern moves a
/// beat earlier, still on the off-beats, and with it the phrase's last stab.
const PATTERNS: [u16; 5] = [
    steps(&[6]),
    steps(&[6]),
    steps(&[6, 14]),
    steps(&[6, 10, 14]),
    steps(&[2, 6, 10, 14]),
];

/// How far Form must cross a pattern's threshold before the pattern changes, either way.
const HYSTERESIS: f32 = 0.02;

/// Highest cutoff, so cold pings stay soft in the highs.
const MAX_CUTOFF: f32 = 4500.0;

/// Seconds the delay boost takes to rise.
const BOOST_RISE: f32 = 0.05;

/// When and how hard stabs are thrown into the delay.
#[derive(Clone, Copy, Debug)]
pub struct Throws {
    /// Every this many bars the phrase's last stab is thrown, twice as often while the bass
    /// is away; 0 never.
    pub phrase_bars: u64,
    /// Whether bass and palette changes, and sudden rises in visual activity, throw the
    /// next stab.
    pub sections: bool,
    pub onsets: bool,
    /// Chance per bar of a throw at full Looseness (see the conductor's tide).
    pub loose: f32,
    /// Delay boost, and its hold and release in bars: of phrase and loose throws, and of
    /// the bigger section and onset throws.
    pub phrase: [f32; 3],
    pub section: [f32; 3],
}

impl Throws {
    /// For short listening: a throw every few bars.
    pub const SHORT: Self = Self {
        phrase_bars: 4,
        sections: true,
        onsets: true,
        loose: 0.3,
        phrase: [0.18, 0.5, 1.0],
        section: [0.28, 1.0, 2.0],
    };

    /// For long listening: rarer gestures that keep the attention without wearing out.
    pub const LONG: Self = Self {
        phrase_bars: 16,
        sections: true,
        onsets: false,
        loose: 0.1,
        phrase: [0.15, 0.5, 1.0],
        section: [0.24, 1.0, 2.0],
    };
}

/// Each pair when cold and warm.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Form at which each pattern starts, from swells to all four off-beats.
    pub levels: [f32; 5],
    /// Peak level of a stab, relative to the drone; lower when warm, where stabs last
    /// longer. Swells, which sit on the full drone, rise only by `swell_level`.
    pub stab_level: [f32; 2],
    pub swell_level: f32,
    /// Level of the chord between stabs at high Form, relative to the drone.
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
    /// nothing, the delay's feedback rises by up to the first, and the chance per bar of a
    /// throw by up to the second (see `Features::density`); 0 ignores the density.
    pub sparse: [f32; 2],
    pub throws: Throws,
}

impl Settings {
    pub const DEFAULT: Self = Self {
        levels: [0.15, 0.3, 0.42, 0.58, 0.72],
        stab_level: [3.3, 2.6],
        swell_level: 0.5,
        residual: 0.06,
        floor: 300.0,
        drone_offset: [1.8, 1.0],
        opening: [3.0, 2.2],
        swell_opening: 1.5,
        drone_resonance: [4.0, 2.0],
        stab_resonance: [10.0, 4.0],
        resonance_corner: 2500.0,
        follow: [1.0, 3.0],
        attack: [0.0015, 0.005],
        hold: [0.25, 0.45],
        release: [0.07, 0.16],
        swell: [0.5, 4.0, 1.2],
        ride: 0.8,
        height: 1.0,
        send: [0.3, 0.45],
        echo_side: Some(0.1),
        sparse: [0.06, 0.25],
        throws: Throws::SHORT,
    };
}

/// The chord's gains at one sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stab {
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

/// Pattern level for `form` from `current`, with hysteresis.
fn pattern_level(form: f32, current: usize, levels: &[f32; 5]) -> usize {
    let mut level = 0;
    for (index, &threshold) in levels.iter().enumerate() {
        let margin = if index < current {
            -HYSTERESIS
        } else {
            HYSTERESIS
        };
        if form >= threshold + margin {
            level = index + 1;
        }
    }
    level
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
    looseness: f32,
    /// How sparse the light is, from 0 (half lit or more) to 1 (dark), and the feedback it
    /// adds to the delay.
    sparseness: f32,
    space: f32,
    /// Whether the echoes start on the right.
    echo_right: bool,
    /// How far the chord plays, from 0 at Flow 0 (resting) to 1.
    rest: f32,
    level: usize,
    bar: u64,
    /// Whether the bass was away at the start of the bar: patterns change only there.
    bar_away: bool,
    /// Whether the first block has set the slewed values.
    primed: bool,
    contour: Contour,
    velocity: f32,
    thrown: bool,
    stab_level: f32,
    /// Throws waiting for the next stab: a big one (sections and onsets) or a phrase-sized
    /// one (looseness).
    armed: Option<[f32; 3]>,
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
            looseness: 0.0,
            sparseness: 0.0,
            space: 0.0,
            echo_right: false,
            rest: 1.0,
            level: 0,
            bar: 0,
            bar_away: false,
            primed: false,
            contour: Contour::default(),
            velocity: 0.0,
            thrown: false,
            stab_level: 0.0,
            armed: None,
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
            .map(|tone| {
                let mut frequency = tone.frequency;
                while frequency < low {
                    frequency *= 2.0;
                }
                while frequency >= high {
                    frequency *= 0.5;
                }
                frequency
            })
            .fold(high, f32::min);
        if snap {
            self.lowest = self.lowest_target;
        }
    }

    /// Throws the next stab, for a change of bass or palette.
    pub fn arm_section(&mut self) {
        if self.settings.throws.sections {
            self.armed = Some(self.settings.throws.section);
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

    /// From the drone (0) to stabs (1), following Form slowly.
    pub fn morph(&self) -> f32 {
        self.morph
    }

    /// Stabs and swells played so far.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reads tempo, away, looseness and onsets from the signals.
    pub fn update_block(&mut self, params: &Params, signals: &Signals) {
        let settings = &self.settings;
        let block_seconds = BLOCK as f32 / self.sample_rate;
        let block_rate = 1.0 / block_seconds;
        self.params = *params;
        self.tempo = signals.tempo;
        self.away = signals.away;
        self.looseness = signals.looseness;
        if signals.onset && settings.throws.onsets && self.level >= 2 {
            self.armed = Some(settings.throws.section);
        }
        let bar_seconds = 240.0 / self.tempo.max(1.0);
        let warmth = smooth(params.heat);

        // Drone to stabs, slewed over a bar; towards Flow 0 the chord rests, like the pulse.
        self.rest = smoothstep(0.0, 0.15, params.flow_setting);
        let morph = smoothstep(0.2, 0.42, params.form) * self.rest;
        let residual = lerp(
            1.0,
            settings.residual,
            smoothstep(0.2, 0.6, params.form) * self.rest,
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
        let height = settings.height * (params.image.height - 0.5);
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
        let lift = 0.6 + 0.4 * self.velocity;
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
        self.sparseness = ((0.5 - params.image.density) * 2.0).clamp(0.0, 1.0);
        self.space = settings.sparse[0] * self.sparseness;
        // Echoes answer the light across the room.
        if let Some(threshold) = settings.echo_side {
            let balance = params.image.balance;
            if balance < -threshold {
                self.echo_right = true;
            } else if balance > threshold {
                self.echo_right = false;
            }
        }
        self.send_target = if self.thrown { 1.0 } else { self.send_level };
    }

    fn new_bar(&mut self, bar: u64) {
        self.bar = bar;
        self.bar_away = self.away;
        self.level = if self.rest < 0.01 {
            0
        } else {
            pattern_level(self.params.form, self.level, &self.settings.levels)
        };
        if self.level < 2 {
            self.armed = None;
        } else if self.armed.is_none()
            && self.rng.unit()
                < self.settings.throws.loose * self.looseness
                    + self.settings.sparse[1] * self.sparseness
        {
            self.armed = Some(self.settings.throws.phrase);
        }
    }

    /// The current pattern; swells play every second bar.
    fn pattern(&self) -> u16 {
        if self.level == 0 || (self.level == 1 && !self.bar.is_multiple_of(2)) {
            return 0;
        }
        let pattern = PATTERNS[self.level - 1];
        if self.bar_away {
            pattern.rotate_right(4)
        } else {
            pattern
        }
    }

    fn trigger(&mut self, bar_step: usize, pattern: u16) {
        let settings = &self.settings;
        let throws = settings.throws;
        // The phrase's last stab, at the end of every few bars.
        let period = if self.bar_away && throws.phrase_bars > 0 {
            (throws.phrase_bars / 2).max(1)
        } else {
            throws.phrase_bars
        };
        let last = bar_step == 15 - pattern.leading_zeros() as usize;
        let phrase = self.level >= 2 && period > 0 && last && (self.bar + 1).is_multiple_of(period);
        let throw = if self.level < 2 {
            None
        } else {
            self.armed.take().or(phrase.then_some(throws.phrase))
        };
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
        let attack = log_lerp(
            swell_attack,
            lerp(settings.attack[0], settings.attack[1], warmth),
            morph,
        );
        let hold = log_lerp(
            swell_hold,
            lerp(settings.hold[0], settings.hold[1], warmth),
            morph,
        ) * sixteenth;
        let release = log_lerp(
            swell_release,
            lerp(settings.release[0], settings.release[1], warmth) * 100.0 / self.tempo.max(1.0),
            morph,
        ) * if self.thrown { 1.5 } else { 1.0 };
        // The stab's gain stays continuous: the new velocity takes over from where the
        // previous stab is.
        let current = self.velocity * self.stab_level * self.contour.value();
        // Sparse stabs hit harder, so the loudness holds as the patterns thin out.
        let density = if self.level >= 2 {
            (4.0 / pattern.count_ones() as f32).powf(0.25)
        } else {
            1.0
        };
        self.stab_level = lerp(
            settings.swell_level,
            lerp(settings.stab_level[0], settings.stab_level[1], warmth) * density,
            morph,
        );
        let accent = if bar_step % 8 == 6 { 1.0 } else { 0.85 };
        self.velocity =
            accent * (0.85 + 0.3 * self.params.activity) * if self.thrown { 1.15 } else { 1.0 };
        let start = current / (self.velocity * self.stab_level).max(1e-6);
        self.contour
            .trigger_from(start, attack, hold, release, self.sample_rate);
        self.count += 1;
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> Stab {
        if let Some(step) = now.step {
            let bar_step = (step % BAR as u64) as usize;
            if bar_step == 0 {
                self.new_bar(step / BAR as u64);
            }
            let pattern = self.pattern();
            if pattern & (1 << bar_step) != 0 {
                self.trigger(bar_step, pattern);
            }
        }
        let envelope = self.contour.next();
        self.send += (self.send_target - self.send) * self.send_slew;
        let stab = self.velocity * envelope * self.rest;
        Stab {
            residual: self.residual,
            dry: stab * self.stab_level,
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
    use nova_core::Controls;
    use nova_core::Features;

    const RATE: f32 = 8000.0;

    #[test]
    fn patterns_are_off_beat_eighths_growing_denser() {
        let mut previous = 0;
        for &pattern in &PATTERNS {
            for mask in [pattern, pattern.rotate_right(4)] {
                assert_eq!(mask & !steps(&[2, 6, 10, 14]), 0, "{mask:b}");
            }
            assert!(pattern.count_ones() >= previous);
            previous = pattern.count_ones();
        }
        let levels = Settings::DEFAULT.levels;
        assert_eq!(pattern_level(0.0, 0, &levels), 0);
        assert_eq!(pattern_level(1.0, 0, &levels), 5);
        // Near a threshold, the pattern holds either way.
        assert_eq!(pattern_level(0.43, 2, &levels), 2);
        assert_eq!(pattern_level(0.41, 3, &levels), 3);
        assert_eq!(pattern_level(0.45, 2, &levels), 3);
        assert_eq!(pattern_level(0.39, 3, &levels), 2);
    }

    /// Plays the chord on the conductor's grid for `seconds`, with half the voxels lit;
    /// returns the stabs, the thrown stabs, the largest boost and the latest gains.
    fn play(form: f32, flow: f32, throws: Throws, seconds: f32) -> (u64, u64, f32, Stab) {
        play_away(form, flow, throws, seconds, None, 0.5)
    }

    /// Like `play`, with the bass held away from home or at home if given, and the given
    /// share of lit voxels.
    fn play_away(
        form: f32,
        flow: f32,
        throws: Throws,
        seconds: f32,
        away: Option<bool>,
        density: f32,
    ) -> (u64, u64, f32, Stab) {
        let palettes: [Palette; 1] = [palette()];
        let harmony = Harmony::from_palette(&palettes[0]);
        let mut conductor = Conductor::new(RATE, harmony);
        let mut signals = Signals::new(harmony);
        let settings = Settings {
            throws,
            ..Settings::DEFAULT
        };
        let mut chord = Chord::new(RATE, settings, [180.0, 720.0], &harmony, Rng::new(1));
        let params = Params {
            form,
            flow,
            flow_setting: flow,
            image: Features {
                density,
                ..Features::default()
            },
            ..Params::default()
        };
        let (mut thrown, mut boost, mut stab) = (0, 0.0_f32, Stab::default());
        for frame in 0..(seconds * RATE) as usize {
            if frame % BLOCK == 0 {
                conductor.update_block(&params, &Controls::default(), 0.0, &palettes, &mut signals);
                if let Some(away) = away {
                    signals.away = away;
                }
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
    fn form_and_flow_decide_whether_stabs_play() {
        let (stabs, _, _, stab) = play(0.05, 0.5, Throws::SHORT, 20.0);
        assert_eq!(stabs, 0, "a drone at low Form");
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
        assert!(stab.residual < 0.1);
    }

    #[test]
    fn stabs_take_over_from_each_other_without_steps() {
        // Right after Form jumps up, the stabs still have long holds and overlap, and their
        // velocities alternate.
        let rate = 48_000.0;
        let palettes: [Palette; 1] = [palette()];
        let harmony = Harmony::from_palette(&palettes[0]);
        let mut conductor = Conductor::new(rate, harmony);
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
                form: if frame < rate as usize * 4 { 0.1 } else { 0.9 },
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
        // About 23 bars a minute at 95 BPM: a throw every four bars, or every sixteen.
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
        let [feedback, _] = Settings::DEFAULT.sparse;
        let (_, sparse, boost, _) = play_away(0.9, 0.5, never, 60.0, Some(false), 0.0);
        assert!(sparse > 0 && boost > feedback, "{sparse}, {boost}");
    }
}
