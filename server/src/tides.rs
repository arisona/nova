//! Tides: slow, subtle variation brought in by the system rather than the user.
//!
//! A tide is a smooth, bounded swell: two sines whose periods are a golden ratio apart,
//! so they never line up exactly and the result feels irregular rather than looping.
//! It needs no state, so light and (later) sound can compute identical values.
//!
//! ```text
//! tide(t) = ½·(sin(2π·t/P + φ₁) + sin(2π·t/(1.618·P) + φ₂))    ∈ [−1, 1]
//! control:  v′ = v + TIDE_DEPTH·depth·4v(1 − v)·tide(t)
//! ```
//!
//! Applied to a control, the swing tapers to nothing at 0 and 1, so both extremes stay
//! exact; in particular Flow 0 stays 0. Sliders keep showing the user's setting; tides
//! only move the effective value around it.

use std::f64::consts::{GOLDEN_RATIO, TAU};

/// When true, the tide clock only advances while Flow is above 0 ("active seconds"), so
/// Flow 0 keeps the display completely still. When false, tides follow wall-clock time
/// and keep moving at Flow 0.
pub const FREEZE_WITH_FLOW: bool = true;

/// Scales every tide; 0 switches them all off.
pub const TIDE_DEPTH: f32 = 1.0;

/// Longest frame step the tide clock takes, so a stalled frame never jumps a tide.
const MAX_STEP_SECONDS: f32 = 0.25;

pub struct Tide {
    period: f64, // seconds
    depth: f32,
    phases: [f64; 2], // radians, so tides don't all start together
}

impl Tide {
    pub const fn new(period: f64, depth: f32, phases: [f64; 2]) -> Self {
        Self {
            period,
            depth,
            phases,
        }
    }

    /// The swell at `seconds`, in [−1, 1].
    pub fn value(&self, seconds: f64) -> f32 {
        let slow = (TAU * seconds / (GOLDEN_RATIO * self.period) + self.phases[1]).sin();
        let fast = (TAU * seconds / self.period + self.phases[0]).sin();
        (0.5 * (fast + slow)) as f32
    }

    /// The swell scaled by this tide's depth and `TIDE_DEPTH`, in [−depth, depth].
    pub fn offset(&self, seconds: f64) -> f32 {
        TIDE_DEPTH * self.depth * self.value(seconds)
    }

    /// Moves a control value in [0, 1] by up to the depth, tapering to nothing at 0 and 1.
    pub fn apply(&self, value: f32, seconds: f64) -> f32 {
        (value + 4.0 * value * (1.0 - value) * self.offset(seconds)).clamp(0.0, 1.0)
    }
}

// Global tides, applied to the controls of every content module. Periods are distinct
// primes so the tides rarely line up.
pub const HEAT: Tide = Tide::new(37.0, 0.08, [0.0, 1.9]);
pub const FLOW: Tide = Tide::new(23.0, 0.10, [2.1, 4.4]);
pub const FORM: Tide = Tide::new(53.0, 0.06, [4.2, 0.7]);
pub const VOID: Tide = Tide::new(29.0, 0.08, [1.1, 3.3]);

/// The tide clock.
#[derive(Default)]
pub struct TideClock {
    seconds: f64,
}

impl TideClock {
    /// Advances by a frame's real time; with `FREEZE_WITH_FLOW`, only while Flow is above 0.
    pub fn advance(&mut self, delta: f32, flow: f32) {
        if !FREEZE_WITH_FLOW || flow > 0.0 {
            self.seconds += delta.clamp(0.0, MAX_STEP_SECONDS) as f64;
        }
    }

    pub fn seconds(&self) -> f64 {
        self.seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tides_keep_extremes_exact_and_freeze_with_flow() {
        for seconds in [0.0, 7.3, 1000.0] {
            for tide in [&HEAT, &FLOW, &FORM, &VOID] {
                assert_eq!(tide.apply(0.0, seconds), 0.0);
                assert_eq!(tide.apply(1.0, seconds), 1.0);
                assert!((tide.apply(0.5, seconds) - 0.5).abs() <= tide.depth * TIDE_DEPTH);
            }
        }
        let mut clock = TideClock::default();
        clock.advance(0.2, 0.0);
        assert_eq!(clock.seconds(), if FREEZE_WITH_FLOW { 0.0 } else { 0.2 });
        clock.advance(10.0, 0.5);
        assert!(clock.seconds() <= 0.2 + MAX_STEP_SECONDS as f64);
    }
}
