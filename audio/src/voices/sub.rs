//! The sub: the notes of a bass line, a sine with a little of its second and third harmonic
//! that glides from note to note. Cold is short and clean, warm longer, rounder and gently
//! saturated. A new note takes over from where the previous one is, so notes never click.

use crate::dsp::{Contour, lerp, smooth};

pub struct Sub {
    sample_rate: f32,
    contour: Contour,
    velocity: f32,
    phase: f32,
    frequency: f32,
    target: f32,
    /// Glide per sample towards the target, as a share of the way.
    glide: f32,
    drive: f32,
    /// Level of the second and third harmonic.
    overtones: [f32; 2],
}

impl Sub {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            contour: Contour::default(),
            velocity: 0.0,
            phase: 0.0,
            frequency: 0.0,
            target: 0.0,
            glide: 1.0,
            drive: 1.0,
            overtones: [0.0; 2],
        }
    }

    /// A note at `frequency`, holding for `hold` seconds before it fades.
    pub fn trigger(&mut self, velocity: f32, frequency: f32, hold: f32, heat: f32) {
        let warmth = smooth(heat);
        let current = self.velocity * self.contour.value();
        if current < 1e-4 {
            self.frequency = frequency;
        }
        self.target = frequency;
        let glide = lerp(0.015, 0.05, warmth);
        self.glide = 1.0 - (-1.0 / (glide * self.sample_rate)).exp();
        self.drive = lerp(1.0, 2.2, warmth);
        self.overtones = [lerp(0.15, 0.3, warmth), lerp(0.08, 0.12, warmth)];
        self.velocity = velocity;
        let start = current / velocity.max(1e-6);
        let release = lerp(0.06, 0.16, warmth);
        self.contour
            .trigger_from(start, 0.006, hold, release, self.sample_rate);
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        let level = self.velocity * self.contour.next();
        if level < 1e-5 {
            return 0.0;
        }
        self.frequency += (self.target - self.frequency) * self.glide;
        self.phase = (self.phase + self.frequency / self.sample_rate).fract();
        let angle = std::f32::consts::TAU * self.phase;
        let [second, third] = self.overtones;
        let sample = angle.sin() + second * (2.0 * angle).sin() + third * (3.0 * angle).sin();
        (sample * self.drive).tanh() / self.drive.tanh() * level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_glide_and_fade() {
        let rate = 8000.0;
        let mut sub = Sub::new(rate);
        assert_eq!(sub.next(), 0.0, "silent until the first note");
        sub.trigger(1.0, 50.0, 0.2, 0.5);
        let peak = (0..800).map(|_| sub.next().abs()).fold(0.0, f32::max);
        assert!(peak > 0.5, "{peak}");
        sub.trigger(1.0, 100.0, 0.0, 0.5);
        assert!(sub.frequency < 51.0, "the new note glides from the old one");
        for _ in 0..8000 {
            sub.next();
        }
        assert!((sub.frequency - 100.0).abs() < 0.1);
        assert!(sub.next().abs() < 1e-3, "and fades");
    }
}
