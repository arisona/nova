//! The air: noise through narrow resonators tuned to the harmony, folded into 500 Hz to
//! 4 kHz. Wind and breath, strongest when cold. Dust instead of noise, sparse impulses that
//! ring the resonators like grains of sand on glass, turns the breath into a crackle.

use crate::BLOCK;
use crate::dsp::{Curve, Drift, Rng, Svf, coefficient, lerp, smooth};
use crate::harmony::{Harmony, MAX_TONES};
use crate::params::Params;

/// How narrow the resonators are over Void, as a factor of their Q: broader when close, so
/// the dry air does not whistle.
const NARROW: Curve = Curve(&[[0.0, 0.6], [0.5, 1.0]]);

/// Impulses per second of the dust.
const DUST_RATE: f32 = 150.0;

pub struct Air {
    sample_rate: f32,
    resonators: [[Svf; 2]; MAX_TONES],
    frequencies: [f32; MAX_TONES],
    targets: [f32; MAX_TONES],
    drifts: [Drift; MAX_TONES],
    gains: [f32; MAX_TONES],
    count: usize,
    level: f32,
    step: f32,
    /// How much of the noise is dust, and the gains of noise and dust for it.
    dust: f32,
    mix: [f32; 2],
    rng: Rng,
}

impl Air {
    pub fn new(sample_rate: f32, harmony: &Harmony, mut rng: Rng) -> Self {
        let mut air = Self {
            sample_rate,
            resonators: [[Svf::default(); 2]; MAX_TONES],
            frequencies: [1000.0; MAX_TONES],
            targets: [1000.0; MAX_TONES],
            drifts: std::array::from_fn(|_| Drift::new(&mut rng)),
            gains: [0.0; MAX_TONES],
            count: 0,
            level: 0.0,
            step: 0.0,
            dust: 0.0,
            mix: [1.0, 0.0],
            rng,
        };
        air.change(harmony);
        air.frequencies = air.targets;
        air
    }

    /// Switches to a harmony at once, without gliding.
    pub fn load(&mut self, harmony: &Harmony) {
        self.count = 0;
        self.change(harmony);
    }

    pub fn change(&mut self, harmony: &Harmony) {
        // Resonators keep gliding from where they are; new ones start at their target.
        for (index, tone) in harmony.tones().iter().enumerate() {
            let mut frequency = tone.frequency;
            while frequency < 500.0 {
                frequency *= 2.0;
            }
            while frequency >= 4000.0 {
                frequency *= 0.5;
            }
            self.targets[index] = frequency;
            if index >= self.count {
                self.frequencies[index] = frequency;
            }
        }
        self.count = harmony.count;
    }

    /// `dust` is how much of the noise is dust, from 0 to 1.
    pub fn update_block(&mut self, params: &Params, dust: f32) {
        let block_seconds = BLOCK as f32 / self.sample_rate;
        let glide = coefficient(2.0, self.sample_rate / BLOCK as f32);
        let warmth = smooth(params.heat);
        let q = lerp(20.0, 10.0, warmth) * NARROW.at(params.void);
        // Narrower bands pass less noise: keep the power independent of Q.
        let normalization = (q / 10.0).sqrt();
        let gust_rate = (0.04 + 0.25 * params.flow) * block_seconds;
        // Even when cold, darker when warm.
        let tilt = lerp(0.0, -1.0, warmth);
        for index in 0..self.count {
            self.frequencies[index] += (self.targets[index] - self.frequencies[index]) * glide;
            for resonator in &mut self.resonators[index] {
                resonator.set(self.frequencies[index], q, self.sample_rate);
            }
            let gust =
                self.drifts[index].advance(gust_rate * (1.0 + 0.3 * index as f32), &mut self.rng);
            self.gains[index] = (0.25 + 0.75 * gust * gust)
                * (self.frequencies[index] / 1000.0).powf(tilt)
                * normalization;
        }
        let target = lerp(0.09, 0.025, warmth) * (0.8 + 0.5 * params.void);
        self.step = (target - self.level) / BLOCK as f32;
        // Dust as loud as the noise: uniform noise has a variance of 1/3.
        self.dust = dust.clamp(0.0, 1.0);
        let impulse = (self.sample_rate / (3.0 * DUST_RATE)).sqrt();
        self.mix = [(1.0 - self.dust).sqrt(), self.dust.sqrt() * impulse];
    }

    #[inline]
    pub fn next(&mut self) -> [f32; 2] {
        self.level += self.step;
        let mut noise = [self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0)];
        if self.dust > 0.0 {
            let chance = DUST_RATE / self.sample_rate;
            for sample in &mut noise {
                let grain = if self.rng.unit() < chance {
                    if self.rng.unit() < 0.5 { -1.0 } else { 1.0 }
                } else {
                    0.0
                };
                *sample = *sample * self.mix[0] + grain * self.mix[1];
            }
        }
        let mut output = [0.0; 2];
        for index in 0..self.count {
            for channel in 0..2 {
                output[channel] +=
                    self.resonators[index][channel].bandpass(noise[channel]) * self.gains[index];
            }
        }
        output.map(|sample| sample * self.level)
    }
}
