//! The pulse: tuned sub thumps, noise ticks and chord stabs, played as the conductor's score
//! hands them out.
//!
//! Visual activity shapes the dynamics, not where steps fall. A sudden rise in visual
//! activity may add one stab on the next off-beat. Cold is straight, short and bright; warm
//! swings, with softer, longer thumps and darker stabs.
//!
//! The drone follows the thumps (`Settings::coupling`): the chord, the air and the reverb dip
//! and swell back, and the synth bass gets a short accent, so bass and thump fuse into one. A
//! chord gate on the grid lets the drone pulse by itself: on the steps where the score closes
//! it, the chord, the air and the reverb dip, and the chord's upper partials more, like
//! a filter; hard and short when cold, rounder and darker when warm. The gate plays while
//! the bass is away from home and fades out when it returns: tension and release. The dip,
//! the gate and the accent go out as signals. As the rhythm dissolves, stabs move into their
//! echoes.

use crate::BLOCK;
use crate::dsp::{Envelope, Rng, coefficient, fold, lerp, smooth, smoothstep};
use crate::harmony::Harmony;
use crate::params::Params;
use crate::signals::{BAR, Hits, Now, Signals};
use crate::voices::stab::STAB_TONES;
use crate::voices::{Stab, Thump, Tick};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Levels of thumps, ticks and stabs; a stab level of 0 leaves the stabs to others.
    pub thump_level: f32,
    pub tick_level: f32,
    pub stab_level: f32,
    /// How strongly the drone follows the pulse: 0 leaves it untouched, 1 pumps like dub.
    /// The rhythm scales it in from the first thumps.
    pub coupling: f32,
    /// Deepest dip of the chord, air and reverb at full coupling, as a fraction: at 1, the
    /// coupling is the depth of the dip.
    pub duck: f32,
    /// Shape of the dip when cold and warm: where the recovery starts and ends, as fractions
    /// of the gap between thumps. [0.0, 0.5] recovers at once and is back halfway through
    /// the gap (tight micro-pumping); [0.3, 1.0] stays down for 30% of the gap and is back
    /// just as the next thump comes (low and slow); an end above 1 never quite comes back.
    pub duck_shape: [[f32; 2]; 2],
    /// Seconds the dip takes to reach its depth.
    pub duck_attack: f32,
    /// How far the drone dips on the closed steps of the chord gate at a rhythm of 0.7 and
    /// more; 0 keeps it steady. The rhythm scales it in from 0.25; the score closes more
    /// steps as it rises.
    pub chord_gate: f32,
    /// Ramp of the chord gate in seconds, and the share of an open step it stays open, when
    /// cold and warm: a hard trance gate, or a rounder one.
    pub chord_gate_ramp: [f32; 2],
    pub chord_gate_duty: [f32; 2],
    /// Share of the gate's dip that is volume, when cold and warm; the rest is brightness
    /// (see `voices/bed.rs`), so the warm gate is more filter than volume.
    pub chord_gate_volume: [f32; 2],
    /// Whether the chord gate plays only while the bass is away from home, fading in and out
    /// over a bar; otherwise it always plays.
    pub chord_gate_away_only: bool,
    /// Release of the bass accent in seconds, when cold and warm.
    pub bass_accent_release: [f32; 2],
}

impl Settings {
    pub const DEFAULT: Self = Self {
        thump_level: 0.24,
        tick_level: 0.15,
        stab_level: 0.3,
        coupling: 0.5,
        duck: 1.0,
        duck_shape: [[0.0, 0.4], [0.15, 0.85]],
        duck_attack: 0.008,
        chord_gate: 0.5,
        chord_gate_ramp: [0.004, 0.012],
        chord_gate_duty: [0.5, 0.65],
        chord_gate_volume: [1.0, 0.7],
        chord_gate_away_only: true,
        bass_accent_release: [0.12, 0.22],
    };
}

/// Share of a stab's dry sound left once the rhythm has dissolved: the rest of it is heard
/// only in its echoes.
const DISSOLVED_DRY: f32 = 0.2;

/// What the pulse hands to the space: its dry signal and its sends.
#[derive(Clone, Copy, Default)]
pub struct PulseOut {
    pub dry: [f32; 2],
    pub delay_send: f32,
    pub reverb_send: [f32; 2],
    /// Gain for the drone (chord, air and reverb), with the dip and the chord gate.
    pub drone: f32,
}

pub struct Pulse {
    sample_rate: f32,
    settings: Settings,
    params: Option<Params>,
    tempo: f32,
    /// Thumps in the current bar and the one before.
    thumps: [usize; 2],
    level: f32,
    level_step: f32,
    root: f32,
    chord: [f32; STAB_TONES],
    next_harmony: Option<Harmony>,
    thump: Thump,
    tick: Tick,
    stabs: [Stab; 2],
    next_stab: usize,
    accent: bool,
    duck: Duck,
    bass_accent: Envelope,
    /// Coupling scaled in by the rhythm, and the releases for the current tempo and Heat.
    coupling: f32,
    /// Seconds between thumps, and the dip's shape for the current Heat.
    duck_gap: f32,
    duck_shape: [f32; 2],
    bass_accent_release: f32,
    bass_accent_value: f32,
    duck_value: f32,
    /// Whether the chord gate is open on the current step, its openness from 0 to 1, how
    /// deep it dips, its ramp coefficient and duty.
    chord_open: bool,
    chord_value: f32,
    chord_depth: f32,
    chord_volume: f32,
    /// Whether the bass is away from home, and the gate's fade with it, from 0 to 1.
    chord_away: bool,
    chord_section: f32,
    chord_ramp: f32,
    chord_duty: f32,
    /// Share of the stabs' dry sound, falling as the rhythm dissolves.
    stab_dry: f32,
    seconds: f32,
    counts: [u64; 3],
    rng: Rng,
}

impl Pulse {
    pub fn new(sample_rate: f32, settings: Settings, harmony: &Harmony, rng: Rng) -> Self {
        let mut pulse = Self {
            sample_rate,
            settings,
            params: None,
            tempo: 0.0,
            thumps: [0; 2],
            level: 0.0,
            level_step: 0.0,
            root: 0.0,
            chord: [0.0; STAB_TONES],
            next_harmony: None,
            thump: Thump::new(sample_rate),
            tick: Tick::new(sample_rate),
            stabs: [Stab::new(sample_rate), Stab::new(sample_rate)],
            next_stab: 0,
            accent: false,
            duck: Duck::default(),
            bass_accent: Envelope::default(),
            coupling: 0.0,
            duck_gap: 1.0,
            duck_shape: settings.duck_shape[0],
            bass_accent_release: 0.1,
            bass_accent_value: 0.0,
            duck_value: 0.0,
            chord_open: true,
            chord_value: 1.0,
            chord_depth: 0.0,
            chord_volume: 1.0,
            chord_away: false,
            chord_section: 0.0,
            chord_ramp: 1.0,
            chord_duty: 1.0,
            stab_dry: 1.0,
            seconds: 0.0,
            counts: [0; 3],
            rng,
        };
        pulse.adopt(harmony);
        pulse
    }

    /// Takes over a new harmony at the next bar.
    pub fn change(&mut self, harmony: &Harmony) {
        self.next_harmony = Some(*harmony);
    }

    /// Takes over a harmony at once.
    pub fn adopt(&mut self, harmony: &Harmony) {
        self.root = harmony.root();
        let base = fold(self.root, 110.0, 220.0);
        let colors = &harmony.tones()[1..];
        self.chord = [
            base,
            colors
                .first()
                .map_or(base * 1.5, |tone| fold(tone.frequency, 180.0, 360.0)),
            colors
                .get(1)
                .map_or(base * 2.0, |tone| fold(tone.frequency, 180.0, 360.0)),
        ];
    }

    /// Steps played so far, per voice: thumps, ticks and stabs.
    pub fn counts(&self) -> [u64; 3] {
        self.counts
    }

    /// How far the chord gate dips the chord now, from 0 to `Settings::chord_gate`.
    fn chord_gate(&self) -> f32 {
        self.chord_depth * (1.0 - self.chord_value)
    }

    /// Writes the dip, the chord gate and the bass accent, as they are now, as signals.
    pub fn publish(&self, signals: &mut Signals) {
        signals.dip = self.duck_value;
        signals.gate = self.chord_gate();
        signals.accent = self.bass_accent_value;
    }

    fn gate(params: &Params) -> f32 {
        smoothstep(0.12, 0.3, params.rhythm) * smoothstep(0.0, 0.15, params.flow_setting)
    }

    /// Reads the tempo, whether the bass is away and onsets from the signals.
    pub fn update_block(&mut self, params: &Params, signals: &Signals) {
        let settings = &self.settings;
        let block_seconds = BLOCK as f32 / self.sample_rate;
        self.seconds += block_seconds;
        self.params = Some(*params);
        self.tempo = signals.tempo;
        self.chord_away = signals.away;
        // A sudden rise in visual activity adds a stab on the next off-beat, if thumps play.
        if signals.onset
            && self.thumps.iter().any(|&thumps| thumps > 0)
            && settings.stab_level > 0.0
        {
            self.accent = true;
        }
        self.level_step = (Self::gate(params) - self.level) * coefficient(1.0, 1.0 / block_seconds)
            / BLOCK as f32;
        let warmth = smooth(params.heat);
        self.coupling = settings.coupling * smoothstep(0.2, 0.6, params.rhythm);
        self.stab_dry = 1.0 - (1.0 - DISSOLVED_DRY) * params.dissolve;
        // As if the thumps spread evenly over the bar.
        self.duck_gap = 240.0 / (self.tempo * self.thumps[1].max(1) as f32);
        let shape = settings.duck_shape;
        self.duck_shape = [0, 1].map(|end| lerp(shape[0][end], shape[1][end], warmth));
        let playing = (self.chord_away || !settings.chord_gate_away_only) as u8 as f32;
        let fade = block_seconds * self.tempo / 240.0;
        self.chord_section += (playing - self.chord_section).clamp(-fade, fade);
        // Like the pulse, the gate rests at Flow 0.
        self.chord_depth = settings.chord_gate
            * smoothstep(0.25, 0.7, params.rhythm)
            * self.chord_section
            * smoothstep(0.0, 0.15, params.flow_setting);
        let volume = settings.chord_gate_volume;
        self.chord_volume = lerp(volume[0], volume[1], warmth);
        let ramp = lerp(
            settings.chord_gate_ramp[0],
            settings.chord_gate_ramp[1],
            warmth,
        );
        self.chord_ramp = 1.0 - (-1.0 / (ramp * self.sample_rate)).exp();
        self.chord_duty = lerp(
            settings.chord_gate_duty[0],
            settings.chord_gate_duty[1],
            warmth,
        );
        let release = settings.bass_accent_release;
        self.bass_accent_release = lerp(release[0], release[1], warmth);
        for stab in &mut self.stabs {
            stab.normalize();
        }
    }

    fn play(&mut self, step: u64, hits: &Hits) {
        let bar_step = (step % BAR as u64) as usize;
        if bar_step == 0 {
            if let Some(harmony) = self.next_harmony.take() {
                self.adopt(&harmony);
            }
            self.thumps = [0, self.thumps[0]];
        }
        let Some(params) = self.params else {
            return;
        };
        let even = step.is_multiple_of(2);
        self.chord_open = hits.gate == 0.0;

        if hits.kick > 0.0 {
            self.thump.trigger(hits.kick, self.root, params.heat);
            self.duck.trigger(hits.kick);
            self.bass_accent.trigger(hits.kick);
            self.thumps[0] += 1;
            self.counts[0] += 1;
        }
        if hits.tick > 0.0 {
            let breath = 0.85 + 0.15 * (std::f32::consts::TAU * self.seconds / 11.0).sin();
            let velocity = 0.8 * hits.tick * (0.75 + 0.5 * params.liveliness) * breath;
            let pan = if even { -0.35 } else { 0.35 } + self.rng.range(-0.1, 0.1);
            self.tick.trigger(velocity, pan, params.heat);
            self.counts[1] += 1;
        }
        let accent = self.accent && !even && self.thumps.iter().any(|&thumps| thumps > 0);
        if (hits.stab > 0.0 || accent) && self.settings.stab_level > 0.0 {
            if accent {
                self.accent = false;
            }
            self.stabs[self.next_stab].trigger(0.9, self.chord, params.heat);
            self.next_stab = 1 - self.next_stab;
            self.counts[2] += 1;
        }
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> PulseOut {
        if let Some(step) = now.step {
            self.play(step, &now.hits);
        }

        // Open for the first part of an open step, with the swing.
        let target = (self.chord_open && now.progress < self.chord_duty) as u8 as f32;
        self.chord_value += (target - self.chord_value) * self.chord_ramp;

        let settings = &self.settings;
        self.level = (self.level + self.level_step).clamp(0.0, 1.0);
        let noise = self.rng.range(-1.0, 1.0);
        let thump = self.thump.next() * settings.thump_level;
        let tick = self
            .tick
            .next(noise)
            .map(|sample| sample * settings.tick_level);
        let stab = (self.stabs[0].next() + self.stabs[1].next()) * settings.stab_level;
        let step = 1.0 / self.sample_rate;
        // Never below silence, however deep the dip and the coupling go.
        let duck = (self
            .duck
            .next(step, settings.duck_attack, self.duck_gap, self.duck_shape)
            * settings.duck
            * self.coupling)
            .min(1.0);
        self.duck_value = duck;
        self.bass_accent_value =
            self.bass_accent.next(0.004, self.bass_accent_release, step) * self.coupling;
        let level = self.level;
        let dry = 0.5 * self.stab_dry * stab;
        PulseOut {
            dry: [
                (thump + tick[0] + dry) * level,
                (thump + tick[1] + dry) * level,
            ],
            delay_send: (stab + 0.12 * (tick[0] + tick[1])) * level,
            reverb_send: [
                (0.3 * tick[0] + 0.25 * stab) * level,
                (0.3 * tick[1] + 0.25 * stab) * level,
            ],
            drone: (1.0 - duck) * (1.0 - self.chord_volume * self.chord_gate()),
        }
    }
}

/// The drone's dip after a thump, from 0 to 1: it reaches its depth within the attack time,
/// holds, then recovers along an S-curve between two points of the gap to the next thump. A
/// thump during a dip only deepens it, so the drone never jumps up.
#[derive(Clone, Copy)]
struct Duck {
    start: f32,
    peak: f32,
    age: f32,
    value: f32,
}

impl Default for Duck {
    fn default() -> Self {
        Self {
            start: 0.0,
            peak: 0.0,
            age: 1e9,
            value: 0.0,
        }
    }
}

impl Duck {
    fn trigger(&mut self, velocity: f32) {
        self.start = self.value;
        self.peak = self.value.max(velocity);
        self.age = 0.0;
    }

    /// `attack` is in seconds, `gap` the time between thumps, `shape` the points of the gap,
    /// from 0 to 1 or more, where the recovery starts and ends.
    #[inline]
    fn next(&mut self, step: f32, attack: f32, gap: f32, shape: [f32; 2]) -> f32 {
        self.age += step;
        let attack = smooth(self.age / attack);
        let back = shape[1].max(shape[0] + 1e-3);
        let release = 1.0 - smoothstep(shape[0], back, self.age / gap);
        self.value = lerp(self.start, self.peak, attack) * release;
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Conductor;
    use crate::harmony::Palette;
    use crate::harmony::tests::palette;
    use crate::instruments::strata::SCORE;
    use nova_core::Controls;

    const RATE: f32 = 8000.0;
    const ATTACK: f32 = Settings::DEFAULT.duck_attack;

    fn params(rhythm: f32) -> Params {
        Params {
            rhythm,
            ..Params::default()
        }
    }

    /// Plays the pulse to Strata's score for `seconds`, with the bass away or at home,
    /// calling `inspect` after every sample.
    fn play(
        pulse: &mut Pulse,
        params: &Params,
        away: bool,
        seconds: f32,
        mut inspect: impl FnMut(&Pulse),
    ) {
        let palettes: [Palette; 1] = [palette()];
        let harmony = Harmony::from_palette(&palettes[0]);
        let mut conductor = Conductor::new(RATE, harmony, SCORE, 1);
        let mut signals = Signals::new(harmony);
        for frame in 0..(seconds * RATE) as usize {
            if frame % BLOCK == 0 {
                conductor.update_block(params, &Controls::default(), 0.0, &palettes, &mut signals);
                signals.away = away;
                pulse.update_block(params, &signals);
            }
            pulse.next(&conductor.next());
            inspect(pulse);
        }
    }

    fn pulse(seed: u64) -> Pulse {
        let harmony = Harmony::from_palette(&palette());
        Pulse::new(RATE, Settings::DEFAULT, &harmony, Rng::new(seed))
    }

    #[test]
    fn dip_holds_recovers_with_the_gap_and_only_deepens() {
        let step = 0.001;
        let mut duck = Duck::default();
        assert_eq!(duck.next(step, ATTACK, 1.0, [0.3, 1.0]), 0.0);
        duck.trigger(1.0);
        let mut values = Vec::new();
        for _ in 0..1000 {
            values.push(duck.next(step, ATTACK, 1.0, [0.3, 1.0]));
        }
        assert!(values[20] > 0.99 && values[299] > 0.99, "holds");
        assert!(values[650] > 0.3 && values[650] < 0.7, "halfway back");
        assert!(values[999] < 0.001, "back at the next thump");

        // A slow shape is still low at the next thump; the new one only deepens it.
        let mut duck = Duck::default();
        duck.trigger(1.0);
        for _ in 0..500 {
            duck.next(step, ATTACK, 0.5, [0.2, 2.0]);
        }
        let mut previous = duck.value;
        assert!(previous > 0.5);
        duck.trigger(0.82);
        for _ in 0..20 {
            let value = duck.next(step, ATTACK, 0.5, [0.2, 2.0]);
            assert!(value >= previous - 1e-6, "{value} after {previous}");
            previous = value;
        }
    }

    #[test]
    fn drone_dips_and_bass_accents_with_thumps() {
        let Settings { duck, coupling, .. } = Settings::DEFAULT;
        for (rhythm, coupled) in [(0.1, false), (0.8, true)] {
            let mut pulse = pulse(1);
            let (mut deepest, mut accent, mut released) = (0.0_f32, 0.0_f32, 0);
            let seconds = 20.0;
            play(&mut pulse, &params(rhythm), false, seconds, |pulse| {
                deepest = deepest.max(pulse.duck_value);
                accent = accent.max(pulse.bass_accent_value);
                released += (pulse.duck_value < 0.5 * (duck * coupling).min(1.0)) as usize;
            });
            if coupled {
                // The first thump of a bar has full velocity; the envelope peaks just below it.
                let dip = (duck * coupling).min(1.0);
                assert!(
                    (0.9 * dip..=dip).contains(&deepest),
                    "dip {deepest} of {dip}"
                );
                assert!(
                    (0.9 * coupling..=coupling).contains(&accent),
                    "accent {accent}"
                );
                assert!(
                    released > (seconds * RATE) as usize / 5,
                    "swells back between thumps"
                );
            } else {
                assert_eq!((deepest, accent), (0.0, 0.0), "untouched without thumps");
            }
        }
    }

    #[test]
    fn chord_gate_chops_the_chord_on_the_grid() {
        let Settings {
            chord_gate,
            chord_gate_ramp,
            chord_gate_away_only,
            ..
        } = Settings::DEFAULT;
        for (rhythm, heat, flow, away, choppy) in [
            (0.1, 0.0, 0.5, true, false),
            (0.8, 0.0, 0.5, false, !chord_gate_away_only),
            (0.8, 0.0, 0.0, true, false),
            (0.8, 0.0, 0.5, true, true),
            (0.8, 1.0, 0.5, true, true),
        ] {
            let mut pulse = pulse(1);
            let params = Params {
                heat,
                flow,
                flow_setting: flow,
                ..params(rhythm)
            };
            let (mut deepest, mut steepest, mut previous) = (0.0_f32, 0.0_f32, 0.0_f32);
            play(&mut pulse, &params, away, 10.0, |pulse| {
                let gate = pulse.chord_gate();
                deepest = deepest.max(gate);
                steepest = steepest.max((gate - previous).abs());
                previous = gate;
            });
            if !choppy {
                assert_eq!(deepest, 0.0, "steady without the gate");
            } else {
                assert!(deepest > 0.9 * chord_gate, "{heat}: {deepest}");
                // Hard when cold, soft when warm: the largest change per sample.
                let ramp = lerp(chord_gate_ramp[0], chord_gate_ramp[1], heat);
                let expected = chord_gate / (ramp * RATE);
                assert!(
                    steepest < 1.1 * expected && steepest > 0.5 * expected,
                    "{steepest}"
                );
            }
        }
    }
}
