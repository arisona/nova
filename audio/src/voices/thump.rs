//! The thump: a sine tuned to the root, folded into 40 to 80 Hz, with a short downward pitch
//! sweep and, when warm, gentle saturation.

use crate::dsp::{Envelope, fold, lerp, smooth};

pub struct Thump {
    sample_rate: f32,
    envelope: Envelope,
    phase: f32,
    frequency: f32,
    drop: f32,
    decay: f32,
    drive: f32,
}

impl Thump {
    pub fn new(sample_rate: f32) -> Self {
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

    pub fn trigger(&mut self, velocity: f32, root: f32, heat: f32) {
        let warmth = smooth(heat);
        self.frequency = fold(root, 40.0, 80.0);
        self.drop = lerp(2.6, 1.8, warmth);
        self.decay = lerp(0.16, 0.42, warmth);
        self.drive = lerp(1.0, 2.5, warmth);
        self.envelope.trigger(velocity);
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.envelope.idle(12.0 * self.decay) {
            return 0.0;
        }
        let pitch =
            self.frequency * (1.0 + (self.drop - 1.0) * (-self.envelope.age() / 0.02).exp());
        self.phase = (self.phase + pitch / self.sample_rate).fract();
        let envelope = self
            .envelope
            .next(0.003, self.decay, 1.0 / self.sample_rate);
        let sample = (std::f32::consts::TAU * self.phase).sin() * envelope;
        (sample * self.drive).tanh() / self.drive.tanh()
    }
}
