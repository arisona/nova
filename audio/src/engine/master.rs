//! The master: what every instrument's mix goes through on its way out.
//!
//! Everything below 120 Hz becomes mono (subwoofers sum their channels), infrasound goes,
//! and a look-ahead limiter keeps peaks below about −1 dBFS. Volume comes last, on a squared
//! curve for finer adjustment at low levels, with a short ramp so changes never click; the
//! output fades in from silence when the engine starts.

use crate::dsp::{Limiter, Svf, smoothstep};

/// Largest sample value: −1 dBFS.
const CEILING: f32 = 0.891;

/// Seconds over which the output fades in after construction.
const FADE_IN_SECONDS: f32 = 1.5;

/// Seconds a change of Volume takes from silence to full level.
const VOLUME_RAMP_SECONDS: f32 = 0.02;

pub struct Master {
    sample_rate: f32,
    /// Fourth-order Butterworth high-pass for the side signal.
    side: [Svf; 2],
    infrasonic: [Svf; 2],
    limiter: Limiter,
    volume: f32,
    gain: f32,
    fade_in: f32,
}

impl Master {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            side: [
                Svf::new(120.0, 0.541, sample_rate),
                Svf::new(120.0, 1.307, sample_rate),
            ],
            infrasonic: [Svf::new(18.0, 0.707, sample_rate); 2],
            limiter: Limiter::new(sample_rate, CEILING, 0.003, 0.25),
            volume: 0.0,
            gain: 0.0,
            fade_in: 0.0,
        }
    }

    /// From 0 (silent) to 1, sanitized by the engine.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    #[inline]
    pub fn process(&mut self, mix: [f32; 2]) -> [f32; 2] {
        let mid = 0.5 * (mix[0] + mix[1]);
        let side = self
            .side
            .iter_mut()
            .fold(0.5 * (mix[0] - mix[1]), |side, filter| {
                filter.highpass(side)
            });
        let stereo = [mid + side, mid - side];
        let mut input = [0.0; 2];
        for channel in 0..2 {
            input[channel] = self.infrasonic[channel].highpass(stereo[channel]);
        }
        let limited = self.limiter.process(input);

        let ramp = 1.0 / (VOLUME_RAMP_SECONDS * self.sample_rate);
        let target = self.volume * self.volume;
        self.gain += (target - self.gain).clamp(-ramp, ramp);
        self.fade_in = (self.fade_in + 1.0 / (FADE_IN_SECONDS * self.sample_rate)).min(1.0);
        let gain = self.gain * smoothstep(0.0, 1.0, self.fade_in);
        limited.map(|sample| {
            if sample.is_finite() {
                sample * gain
            } else {
                0.0
            }
        })
    }
}
