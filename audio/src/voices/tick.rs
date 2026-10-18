//! The tick: a short burst of noise, high-passed, with a metallic resonance when cold.

use crate::dsp::{Envelope, Svf, lerp, smooth};

pub struct Tick {
    sample_rate: f32,
    envelope: Envelope,
    decay: f32,
    pan: [f32; 2],
    high: Svf,
    ring: Svf,
    ring_mix: f32,
}

impl Tick {
    pub fn new(sample_rate: f32) -> Self {
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

    pub fn trigger(&mut self, velocity: f32, pan: f32, heat: f32) {
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
    pub fn next(&mut self, noise: f32) -> [f32; 2] {
        if self.envelope.idle(15.0 * self.decay) {
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
