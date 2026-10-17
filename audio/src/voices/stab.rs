//! The chord stab: additive tones whose upper partials die away faster, like a closing
//! filter. A retriggered stab first fades out over 3 ms.

use crate::dsp::{Rotor, lerp, smooth};

/// Tones of a chord stab, and partials per tone.
pub const STAB_TONES: usize = 3;
const STAB_PARTIALS: usize = 10;

pub struct Stab {
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
    pub fn new(sample_rate: f32) -> Self {
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

    pub fn trigger(&mut self, velocity: f32, chord: [f32; STAB_TONES], heat: f32) {
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

    pub fn normalize(&mut self) {
        for rotor in self.rotors.iter_mut().flatten() {
            rotor.normalize();
        }
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
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
