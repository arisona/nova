//! The pulse: tuned sub thumps, noise ticks and chord stabs on the conductor's grid of 16ths.
//!
//! Form decides how many pulses each voice spreads over its loop (Euclidean patterns).
//! Patterns change only at bar boundaries, so the rhythm never feels random; visual activity
//! shapes the dynamics, not where steps fall. A sudden rise in visual activity may add one
//! stab on the next off-beat. Cold is straight, short and bright; warm swings, with softer,
//! longer thumps and darker stabs.
//!
//! The drone follows the thumps (`Settings::coupling`): the chord, the air and the reverb dip
//! and swell back, and the synth bass gets a short accent, so bass and thump fuse into one. A
//! chord gate on the grid lets the drone pulse by itself: on closed steps of a Euclidean
//! pattern the chord, the air and the reverb dip, and the chord's upper partials more, like
//! a filter; hard and short when cold, rounder and darker when warm. The gate plays while
//! the bass is away from home and fades out when it returns: tension and release. The dip,
//! the gate and the accent go out as signals.

use crate::BLOCK;
use crate::dsp::{Envelope, Rng, coefficient, euclidean, lerp, smooth, smoothstep};
use crate::harmony::Harmony;
use crate::params::Params;
use crate::signals::{BAR, Now, Signals};
use crate::voices::stab::STAB_TONES;
use crate::voices::{Stab, Thump, Tick};

/// How the pulse places its steps: everything on one 16-step bar, or loops of 16, 12 and
/// 7 steps that phase against each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rhythm {
    Euclidean,
    Polymeter,
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub rhythm: Rhythm,
    /// Form at which thumps, ticks and stabs start; an infinite start leaves a voice out.
    pub starts: [f32; 3],
    /// Thumps per bar as Form rises from their start, in four even steps.
    pub thump_counts: [usize; 4],
    pub thump_level: f32,
    pub tick_level: f32,
    pub stab_level: f32,
    /// When the rhythm loosens (see the conductor's Looseness tide): the chance of a quiet
    /// ghost tick on an empty step and of leaving out a tick off the beat. Thumps and stabs
    /// stay exact.
    pub ghost: f32,
    pub drop: f32,
    /// How strongly the drone follows the pulse: 0 leaves it untouched, 1 pumps like dub.
    /// Form scales it in from the first thumps.
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
    /// How far the drone dips on the closed steps of the chord gate at full Form; 0 keeps it
    /// steady. Form scales it in from 0.25 to 0.7.
    pub chord_gate: f32,
    /// Length of a gate step in 16ths: 1 chops on 16ths, 2 on 8ths.
    pub chord_gate_grid: usize,
    /// Share of open steps at Form 0.25 and from Form 0.7: few closed steps, then choppier.
    pub chord_gate_open: [f32; 2],
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
        rhythm: Rhythm::Euclidean,
        starts: [0.2, 0.35, 0.45],
        thump_counts: [1, 2, 3, 4],
        thump_level: 0.24,
        tick_level: 0.15,
        stab_level: 0.3,
        ghost: 0.2,
        drop: 0.12,
        coupling: 0.5,
        duck: 1.0,
        duck_shape: [[0.0, 0.4], [0.15, 0.85]],
        duck_attack: 0.008,
        chord_gate: 0.5,
        chord_gate_grid: 1,
        chord_gate_open: [0.875, 0.5625],
        chord_gate_ramp: [0.004, 0.012],
        chord_gate_duty: [0.5, 0.65],
        chord_gate_volume: [1.0, 0.7],
        chord_gate_away_only: true,
        bass_accent_release: [0.12, 0.22],
    };
}

/// What the pulse hands to the space: its dry signal and its sends.
#[derive(Clone, Copy, Default)]
pub struct PulseOut {
    pub dry: [f32; 2],
    pub delay_send: f32,
    pub reverb_send: [f32; 2],
    /// Gain for the drone (chord, air and reverb), with the dip and the chord gate.
    pub drone: f32,
}

/// Pulses per voice, fixed for a bar.
#[derive(Clone, Copy, Default)]
struct Patterns {
    thumps: usize,
    ticks: usize,
    stabs: usize,
    /// Open steps of the chord gate, of `BAR / chord_gate_grid`.
    chord_gate: usize,
}

pub struct Pulse {
    sample_rate: f32,
    settings: Settings,
    params: Option<Params>,
    looseness: f32,
    tempo: f32,
    patterns: Patterns,
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
    /// Coupling scaled in by Form, and the releases for the current tempo and Heat.
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
    /// The 16th within the bar now.
    bar_step: usize,
    chord_value: f32,
    chord_depth: f32,
    chord_volume: f32,
    /// Whether the bass is away from home, and the gate's fade with it, from 0 to 1.
    chord_away: bool,
    chord_section: f32,
    chord_ramp: f32,
    chord_duty: f32,
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
            looseness: 0.0,
            tempo: 0.0,
            patterns: Patterns::default(),
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
            bar_step: 0,
            chord_value: 1.0,
            chord_depth: 0.0,
            chord_volume: 1.0,
            chord_away: false,
            chord_section: 0.0,
            chord_ramp: 1.0,
            chord_duty: 1.0,
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
        fn fold(mut frequency: f32, low: f32) -> f32 {
            while frequency < low {
                frequency *= 2.0;
            }
            while frequency >= 2.0 * low {
                frequency *= 0.5;
            }
            frequency
        }
        self.root = harmony.root();
        let base = fold(self.root, 110.0);
        let colors = &harmony.tones()[1..];
        self.chord = [
            base,
            colors
                .first()
                .map_or(base * 1.5, |tone| fold(tone.frequency, 180.0)),
            colors
                .get(1)
                .map_or(base * 2.0, |tone| fold(tone.frequency, 180.0)),
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
        smoothstep(0.12, 0.3, params.form) * smoothstep(0.0, 0.15, params.flow_setting)
    }

    /// Reads the tempo, the looseness, whether the bass is away and onsets from the signals.
    pub fn update_block(&mut self, params: &Params, signals: &Signals) {
        let settings = &self.settings;
        let block_seconds = BLOCK as f32 / self.sample_rate;
        self.seconds += block_seconds;
        self.params = Some(*params);
        self.tempo = signals.tempo;
        self.looseness = signals.looseness;
        self.chord_away = signals.away;
        // A sudden rise in visual activity adds a stab on the next off-beat, if thumps play.
        if signals.onset && self.patterns.thumps > 0 && settings.stab_level > 0.0 {
            self.accent = true;
        }
        self.level_step = (Self::gate(params) - self.level) * coefficient(1.0, 1.0 / block_seconds)
            / BLOCK as f32;
        let warmth = smooth(params.heat);
        self.coupling = settings.coupling * smoothstep(0.2, 0.6, params.form);
        // Thumps spread evenly over a bar of 16 sixteenths.
        self.duck_gap = 240.0 / (self.tempo * self.patterns.thumps.max(1) as f32);
        let shape = settings.duck_shape;
        self.duck_shape = [0, 1].map(|end| lerp(shape[0][end], shape[1][end], warmth));
        let playing = (self.chord_away || !settings.chord_gate_away_only) as u8 as f32;
        let fade = block_seconds * self.tempo / 240.0;
        self.chord_section += (playing - self.chord_section).clamp(-fade, fade);
        // Like the pulse, the gate rests at Flow 0.
        self.chord_depth = settings.chord_gate
            * smoothstep(0.25, 0.7, params.form)
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

    fn new_bar(&mut self) {
        if let Some(harmony) = self.next_harmony.take() {
            self.adopt(&harmony);
        }
        let Some(params) = self.params else {
            return;
        };
        let settings = &self.settings;
        let [thumps, ticks, stabs] = settings.starts;
        let form = params.form;
        let open = Self::gate(&params) > 0.01;
        let (tick_steps, stab_steps) = self.loop_lengths();
        let gate_steps = BAR / settings.chord_gate_grid;
        let scaled = |low: f32, count: f32| ((form - low) / (1.0 - low)).clamp(0.0, 1.0) * count;
        self.patterns = Patterns {
            thumps: if open && form >= thumps {
                settings.thump_counts[(scaled(thumps, 4.0) as usize).min(3)]
            } else {
                0
            },
            ticks: if open && form >= ticks {
                scaled(ticks, tick_steps as f32 * 0.625).round() as usize
            } else {
                0
            },
            stabs: if open && form >= stabs {
                1 + (scaled(stabs, stab_steps as f32 * 0.19) as usize).min(stab_steps / 6)
            } else {
                0
            },
            chord_gate: if open && form >= 0.25 {
                let reach = ((form - 0.25) / 0.45).clamp(0.0, 1.0);
                let [few, many] = settings.chord_gate_open;
                (lerp(few, many, reach) * gate_steps as f32).round() as usize
            } else {
                gate_steps
            },
        };
    }

    /// Loop lengths of ticks and stabs; thumps always follow the bar.
    fn loop_lengths(&self) -> (usize, usize) {
        match self.settings.rhythm {
            Rhythm::Euclidean => (BAR, BAR),
            Rhythm::Polymeter => (12, 7),
        }
    }

    fn play(&mut self, step: u64) {
        let bar_step = (step % BAR as u64) as usize;
        if bar_step == 0 {
            self.new_bar();
        }
        let Some(params) = self.params else {
            return;
        };
        let (tick_steps, stab_steps) = self.loop_lengths();
        let (tick_rotation, stab_rotation) = match self.settings.rhythm {
            Rhythm::Euclidean => (2, 2),
            Rhythm::Polymeter => (1, 3),
        };
        let even = step.is_multiple_of(2);
        let patterns = self.patterns;
        let grid = self.settings.chord_gate_grid;
        self.bar_step = bar_step;
        self.chord_open = euclidean(patterns.chord_gate, BAR / grid, 0, bar_step / grid);

        if euclidean(patterns.thumps, BAR, 0, bar_step) {
            let velocity = if bar_step == 0 { 1.0 } else { 0.82 };
            self.thump.trigger(velocity, self.root, params.heat);
            self.duck.trigger(velocity);
            self.bass_accent.trigger(velocity);
            self.counts[0] += 1;
        }
        let tick = euclidean(
            patterns.ticks,
            tick_steps,
            tick_rotation,
            (step % tick_steps as u64) as usize,
        );
        // Loose: a few ticks off the beat drop out, a few empty steps get a ghost tick.
        let loose = self.looseness;
        let tick = if tick {
            (bar_step.is_multiple_of(4) || self.rng.unit() >= self.settings.drop * loose)
                .then_some(1.0)
        } else if patterns.ticks > 0 && loose > 0.0 && self.rng.unit() < self.settings.ghost * loose
        {
            Some(0.4)
        } else {
            None
        };
        if let Some(scale) = tick {
            let breath = 0.85 + 0.15 * (std::f32::consts::TAU * self.seconds / 11.0).sin();
            let velocity =
                if even { 0.8 } else { 0.55 } * (0.75 + 0.5 * params.activity) * breath * scale;
            let pan = if even { -0.35 } else { 0.35 } + self.rng.range(-0.1, 0.1);
            self.tick.trigger(velocity, pan, params.heat);
            self.counts[1] += 1;
        }
        let stab = euclidean(
            patterns.stabs,
            stab_steps,
            stab_rotation,
            (step % stab_steps as u64) as usize,
        );
        let accent = self.accent && !even && patterns.thumps > 0;
        if stab || accent {
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
            self.play(step);
        }

        // Where in the current gate step we are, from 0 to 1, with the swing.
        let grid = self.settings.chord_gate_grid;
        let position = ((self.bar_step % grid) as f32 + now.progress) / grid as f32;
        let target = (self.chord_open && position < self.chord_duty) as u8 as f32;
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
        PulseOut {
            dry: [
                (thump + tick[0] + 0.5 * stab) * level,
                (thump + tick[1] + 0.5 * stab) * level,
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
    use nova_core::Controls;

    const RATE: f32 = 8000.0;
    const ATTACK: f32 = Settings::DEFAULT.duck_attack;

    fn params(form: f32) -> Params {
        Params {
            form,
            ..Params::default()
        }
    }

    /// Plays the pulse on the conductor's grid for `seconds`, with the bass away or at home
    /// and the given looseness, calling `inspect` after every sample.
    fn play(
        pulse: &mut Pulse,
        params: &Params,
        away: bool,
        looseness: f32,
        seconds: f32,
        mut inspect: impl FnMut(&Pulse),
    ) {
        let palettes: [Palette; 1] = [palette()];
        let harmony = Harmony::from_palette(&palettes[0]);
        let mut conductor = Conductor::new(RATE, harmony);
        let mut signals = Signals::new(harmony);
        for frame in 0..(seconds * RATE) as usize {
            if frame % BLOCK == 0 {
                conductor.update_block(params, &Controls::default(), 0.0, &palettes, &mut signals);
                signals.away = away;
                signals.looseness = looseness;
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
        for (form, coupled) in [(0.1, false), (0.8, true)] {
            let mut pulse = pulse(1);
            let (mut deepest, mut accent, mut released) = (0.0_f32, 0.0_f32, 0);
            let seconds = 20.0;
            play(&mut pulse, &params(form), false, 0.0, seconds, |pulse| {
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
        for (form, heat, flow, away, choppy) in [
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
                ..params(form)
            };
            let (mut deepest, mut steepest, mut previous) = (0.0_f32, 0.0_f32, 0.0_f32);
            play(&mut pulse, &params, away, 0.0, 10.0, |pulse| {
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

    fn ticks(looseness: f32, seed: u64) -> u64 {
        let mut pulse = pulse(seed);
        play(&mut pulse, &params(0.7), false, looseness, 40.0, |_| {});
        pulse.counts()[1]
    }

    #[test]
    fn rhythm_is_exact_unless_the_tide_loosens_it() {
        let strict = ticks(0.0, 1);
        assert!(strict > 50, "{strict}");
        assert_eq!(strict, ticks(0.0, 2), "no chance involved while strict");
        let loose = [ticks(1.0, 1), ticks(1.0, 2)];
        assert!(
            loose.iter().all(|&count| count != strict),
            "{strict} vs {loose:?}"
        );
        assert!(loose[0] != loose[1]);
    }
}
