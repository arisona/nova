//! The pulse: tuned sub thumps, noise ticks and chord stabs on a 16th-note grid.
//!
//! Form decides how many pulses each voice spreads over its loop (Euclidean patterns),
//! the slider setting of Flow the tempo. Patterns change only at bar boundaries, so the
//! rhythm never feels random; visual activity shapes the dynamics, not where steps fall.
//! A sudden rise in visual activity may add one stab on the next off-beat. Cold is straight,
//! short and bright; warm swings, with softer, longer thumps and darker stabs.

use super::{BLOCK, Harmony, Params, Rhythm};
use crate::audio::dsp::{Rng, Rotor, Svf, coefficient, euclidean, lerp, smooth, smoothstep};

/// Steps per bar (16ths).
const BAR: usize = 16;

const THUMP_LEVEL: f32 = 0.24;
const TICK_LEVEL: f32 = 0.15;
const STAB_LEVEL: f32 = 0.3;

/// Tempo range in beats per minute, over the Flow slider.
const SLOWEST: f32 = 80.0;
const FASTEST: f32 = 110.0;

/// When the rhythm loosens (see `tides::LOOSENESS`): the chance of a quiet ghost tick on an
/// empty step and of leaving out a tick off the beat, and extra swing. Thumps and stabs
/// stay exact.
const GHOST: f32 = 0.2;
const DROP: f32 = 0.12;
const LOOSE_SWING: f32 = 0.025;

/// Seconds between stabs added by visual onsets.
const ACCENT_COOLDOWN: f32 = 8.0;

const STAB_TONES: usize = 3;
const STAB_PARTIALS: usize = 10;

/// What the pulse hands to the space: its dry signal and its sends.
#[derive(Clone, Copy, Default)]
pub struct PulseOut {
    pub dry: [f32; 2],
    pub delay_send: f32,
    pub reverb_send: [f32; 2],
}

/// Pulses per voice and swing, fixed for a bar.
#[derive(Clone, Copy, Default)]
struct Patterns {
    thumps: usize,
    ticks: usize,
    stabs: usize,
    swing: f32,
}

pub struct Pulse {
    sample_rate: f32,
    rhythm: Rhythm,
    params: Option<Params>,
    tempo: f32,
    pair_phase: f64,
    odd_played: bool,
    step: u64,
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
    cooldown: f32,
    seconds: f32,
    counts: [u64; 3],
    rng: Rng,
}

impl Pulse {
    pub fn new(sample_rate: f32, rhythm: Rhythm, harmony: &Harmony, rng: Rng) -> Self {
        let mut pulse = Self {
            sample_rate,
            rhythm,
            params: None,
            tempo: lerp(SLOWEST, FASTEST, 0.5),
            pair_phase: 0.999,
            odd_played: true,
            step: u64::MAX - 1,
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
            cooldown: ACCENT_COOLDOWN,
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

    pub fn request_accent(&mut self) {
        if self.cooldown <= 0.0 {
            self.accent = true;
        }
    }

    pub fn tempo(&self) -> f32 {
        self.tempo
    }

    #[cfg(test)]
    pub fn counts(&self) -> [u64; 3] {
        self.counts
    }

    fn gate(params: &Params) -> f32 {
        smoothstep(0.12, 0.3, params.form) * smoothstep(0.0, 0.15, params.flow_setting)
    }

    pub fn update_block(&mut self, params: &Params) {
        let block_seconds = BLOCK as f32 / self.sample_rate;
        self.seconds += block_seconds;
        self.cooldown -= block_seconds;
        // The first block takes the tempo as it is, like the other controls.
        if self.params.replace(*params).is_none() {
            self.tempo = lerp(SLOWEST, FASTEST, params.flow_setting);
        }
        self.level_step = (Self::gate(params) - self.level) * coefficient(1.0, 1.0 / block_seconds)
            / BLOCK as f32;
        // Exponential approach with a slew limit, so tempo changes stay gentle.
        let target = lerp(SLOWEST, FASTEST, params.flow_setting);
        let change = (target - self.tempo) * coefficient(2.5, 1.0 / block_seconds);
        self.tempo += change.clamp(-8.0 * block_seconds, 8.0 * block_seconds);
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
        let form = params.form;
        let open = Self::gate(&params) > 0.01;
        let (tick_steps, stab_steps) = self.loop_lengths();
        let scaled = |low: f32, count: f32| ((form - low) / (1.0 - low)).clamp(0.0, 1.0) * count;
        self.patterns = Patterns {
            thumps: if open && form >= 0.2 {
                1 + (scaled(0.2, 4.0) as usize).min(3)
            } else {
                0
            },
            ticks: if open && form >= 0.35 {
                scaled(0.35, tick_steps as f32 * 0.625).round() as usize
            } else {
                0
            },
            stabs: if open && form >= 0.45 {
                1 + (scaled(0.45, stab_steps as f32 * 0.19) as usize).min(stab_steps / 6)
            } else {
                0
            },
            swing: lerp(0.5, 0.57, smooth(params.heat)) + LOOSE_SWING * params.looseness,
        };
    }

    /// Loop lengths of ticks and stabs; thumps always follow the bar.
    fn loop_lengths(&self) -> (usize, usize) {
        match self.rhythm {
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
        let (tick_rotation, stab_rotation) = match self.rhythm {
            Rhythm::Euclidean => (2, 2),
            Rhythm::Polymeter => (1, 3),
        };
        let even = step.is_multiple_of(2);
        let patterns = self.patterns;

        if euclidean(patterns.thumps, BAR, 0, bar_step) {
            let velocity = if bar_step == 0 { 1.0 } else { 0.82 };
            self.thump.trigger(velocity, self.root, params.heat);
            self.counts[0] += 1;
        }
        let tick = euclidean(
            patterns.ticks,
            tick_steps,
            tick_rotation,
            (step % tick_steps as u64) as usize,
        );
        // Loose: a few ticks off the beat drop out, a few empty steps get a ghost tick.
        let loose = params.looseness;
        let tick = if tick {
            (bar_step.is_multiple_of(4) || self.rng.unit() >= DROP * loose).then_some(1.0)
        } else if patterns.ticks > 0 && loose > 0.0 && self.rng.unit() < GHOST * loose {
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
                self.cooldown = ACCENT_COOLDOWN;
            }
            self.stabs[self.next_stab].trigger(0.9, self.chord, params.heat);
            self.next_stab = 1 - self.next_stab;
            self.counts[2] += 1;
        }
    }

    #[inline]
    pub fn next(&mut self) -> PulseOut {
        self.pair_phase += (self.tempo / 30.0 / self.sample_rate) as f64;
        if self.pair_phase >= 1.0 {
            self.pair_phase -= 1.0;
            self.step = self.step.wrapping_add(2);
            self.odd_played = false;
            self.play(self.step);
        }
        if !self.odd_played && self.pair_phase >= self.patterns.swing as f64 {
            self.odd_played = true;
            self.play(self.step.wrapping_add(1));
        }

        self.level = (self.level + self.level_step).clamp(0.0, 1.0);
        let noise = self.rng.range(-1.0, 1.0);
        let thump = self.thump.next() * THUMP_LEVEL;
        let tick = self.tick.next(noise).map(|sample| sample * TICK_LEVEL);
        let stab = (self.stabs[0].next() + self.stabs[1].next()) * STAB_LEVEL;
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
        }
    }
}

/// Envelope with a click-free retrigger: the previous note's level decays quickly while the
/// new one rises.
#[derive(Clone, Copy, Default)]
struct Envelope {
    age: f32,
    carry: f32,
    velocity: f32,
    value: f32,
}

impl Envelope {
    fn trigger(&mut self, velocity: f32) {
        self.carry = self.value;
        self.velocity = velocity;
        self.age = 0.0;
    }

    #[inline]
    fn next(&mut self, attack: f32, decay: f32, step: f32) -> f32 {
        self.value = self.carry * (-self.age / 0.003).exp()
            + self.velocity * smooth(self.age / attack) * (-self.age / decay).exp();
        self.age += step;
        self.value
    }
}

/// A sine thump tuned to the root (folded into 40 to 80 Hz), with a short downward pitch
/// sweep and, when warm, gentle saturation.
struct Thump {
    sample_rate: f32,
    envelope: Envelope,
    phase: f32,
    frequency: f32,
    drop: f32,
    decay: f32,
    drive: f32,
}

impl Thump {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            envelope: Envelope::default(),
            phase: 0.0,
            frequency: 50.0,
            drop: 2.0,
            decay: 0.3,
            drive: 1.0,
        }
    }

    fn trigger(&mut self, velocity: f32, root: f32, heat: f32) {
        let warmth = smooth(heat);
        let mut frequency = root;
        while frequency < 40.0 {
            frequency *= 2.0;
        }
        while frequency >= 80.0 {
            frequency *= 0.5;
        }
        self.frequency = frequency;
        self.drop = lerp(2.6, 1.8, warmth);
        self.decay = lerp(0.16, 0.42, warmth);
        self.drive = lerp(1.0, 2.5, warmth);
        self.envelope.trigger(velocity);
    }

    #[inline]
    fn next(&mut self) -> f32 {
        if self.envelope.velocity == 0.0 || self.envelope.age > 12.0 * self.decay {
            return 0.0;
        }
        let pitch = self.frequency * (1.0 + (self.drop - 1.0) * (-self.envelope.age / 0.02).exp());
        self.phase = (self.phase + pitch / self.sample_rate).fract();
        let envelope = self
            .envelope
            .next(0.003, self.decay, 1.0 / self.sample_rate);
        let sample = (std::f32::consts::TAU * self.phase).sin() * envelope;
        (sample * self.drive).tanh() / self.drive.tanh()
    }
}

/// A short noise tick: high-passed, with a metallic resonance when cold.
struct Tick {
    sample_rate: f32,
    envelope: Envelope,
    decay: f32,
    pan: [f32; 2],
    high: Svf,
    ring: Svf,
    ring_mix: f32,
}

impl Tick {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            envelope: Envelope::default(),
            decay: 0.02,
            pan: [std::f32::consts::FRAC_1_SQRT_2; 2],
            high: Svf::new(6000.0, 0.7, sample_rate),
            ring: Svf::new(9000.0, 6.0, sample_rate),
            ring_mix: 0.3,
        }
    }

    fn trigger(&mut self, velocity: f32, pan: f32, heat: f32) {
        let warmth = smooth(heat);
        self.decay = lerp(0.010, 0.030, warmth);
        self.high
            .set(lerp(5500.0, 2500.0, warmth), 0.7, self.sample_rate);
        self.ring.set(
            lerp(7000.0, 4000.0, warmth),
            lerp(9.0, 2.0, warmth),
            self.sample_rate,
        );
        self.ring_mix = lerp(0.55, 0.1, warmth);
        let angle = (0.5 + 0.5 * pan.clamp(-1.0, 1.0)) * std::f32::consts::FRAC_PI_2;
        self.pan = [angle.cos(), angle.sin()];
        self.envelope.trigger(velocity);
    }

    #[inline]
    fn next(&mut self, noise: f32) -> [f32; 2] {
        if self.envelope.velocity == 0.0 || self.envelope.age > 15.0 * self.decay {
            return [0.0; 2];
        }
        let envelope = self
            .envelope
            .next(0.0006, self.decay, 1.0 / self.sample_rate);
        let high = self.high.highpass(noise);
        let ring = self.ring.bandpass(noise);
        let sample = (high * (1.0 - self.ring_mix) + 1.5 * ring * self.ring_mix) * envelope;
        [sample * self.pan[0], sample * self.pan[1]]
    }
}

/// A chord stab: additive tones whose upper partials die away faster, like a closing
/// filter. A retriggered voice first fades out over 3 ms.
struct Stab {
    sample_rate: f32,
    rotors: [[Rotor; STAB_PARTIALS]; STAB_TONES],
    amplitudes: [[f32; STAB_PARTIALS]; STAB_TONES],
    decays: [[f32; STAB_PARTIALS]; STAB_TONES],
    age: f32,
    gain: f32,
    active: bool,
    pending: Option<(f32, [f32; STAB_TONES], f32)>,
}

impl Stab {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            rotors: [[Rotor::new(0.0); STAB_PARTIALS]; STAB_TONES],
            amplitudes: [[0.0; STAB_PARTIALS]; STAB_TONES],
            decays: [[0.0; STAB_PARTIALS]; STAB_TONES],
            age: 0.0,
            gain: 0.0,
            active: false,
            pending: None,
        }
    }

    fn trigger(&mut self, velocity: f32, chord: [f32; STAB_TONES], heat: f32) {
        if self.active && self.amplitudes[0][0] * self.gain > 1e-4 {
            self.pending = Some((velocity, chord, heat));
        } else {
            self.start(velocity, chord, heat);
        }
    }

    fn start(&mut self, velocity: f32, chord: [f32; STAB_TONES], heat: f32) {
        let warmth = smooth(heat);
        let decay = lerp(0.14, 0.32, warmth);
        let tilt = lerp(1.4, 1.0, warmth);
        let damping = lerp(0.5, 1.2, warmth);
        let top = (0.4 * self.sample_rate).min(5000.0);
        for (tone, frequency) in chord.iter().enumerate() {
            for partial in 0..STAB_PARTIALS {
                let partial_frequency = frequency * (partial + 1) as f32;
                self.rotors[tone][partial].set_frequency(partial_frequency, self.sample_rate);
                self.amplitudes[tone][partial] = if partial_frequency < top {
                    velocity * ((partial + 1) as f32).powf(-tilt) / STAB_TONES as f32
                } else {
                    0.0
                };
                let time = decay / (1.0 + damping * partial as f32);
                self.decays[tone][partial] = (-1.0 / (time * self.sample_rate)).exp();
            }
        }
        self.age = 0.0;
        self.gain = 1.0;
        self.active = true;
    }

    fn normalize(&mut self) {
        for rotor in self.rotors.iter_mut().flatten() {
            rotor.normalize();
        }
    }

    #[inline]
    fn next(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }
        if self.pending.is_some() {
            self.gain -= 1.0 / (0.003 * self.sample_rate);
            if self.gain <= 0.0 {
                let (velocity, chord, heat) = self.pending.take().unwrap_or_default();
                self.start(velocity, chord, heat);
            }
        }
        let mut sum = 0.0;
        for tone in 0..STAB_TONES {
            for partial in 0..STAB_PARTIALS {
                sum += self.rotors[tone][partial].next() * self.amplitudes[tone][partial];
                self.amplitudes[tone][partial] *= self.decays[tone][partial];
            }
        }
        let attack = smooth(self.age / 0.004);
        self.age += 1.0 / self.sample_rate;
        if self.age > 3.0 {
            self.active = false;
        }
        sum * attack * self.gain.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Harmony;
    use super::*;
    use crate::audio::features::Features;
    use crate::palettes::PALETTES;

    fn ticks(looseness: f32, seed: u64) -> u64 {
        let rate = 8000.0;
        let harmony = Harmony::from_palette(&PALETTES[0]);
        let mut pulse = Pulse::new(rate, Rhythm::Euclidean, &harmony, Rng::new(seed));
        let params = Params {
            heat: 0.5,
            flow: 0.5,
            form: 0.7,
            void: 0.5,
            flow_setting: 0.5,
            activity: 0.0,
            looseness,
            layers: Features::default().layers,
        };
        for frame in 0..(rate as usize * 40) {
            if frame % BLOCK == 0 {
                pulse.update_block(&params);
            }
            pulse.next();
        }
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
