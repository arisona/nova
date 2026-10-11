//! Small DSP building blocks for the instrument. Everything allocates only on construction,
//! so it can run in the audio callback.

use std::f32::consts::{PI, TAU};

pub use crate::content::common::Rng;

/// Cubic ease from 0 to 1.
pub fn smooth(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

/// 0 below `low`, 1 above `high`, eased in between.
pub fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    smooth((value - low) / (high - low))
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// One-pole coefficient for a time constant, at a given update rate.
pub fn coefficient(seconds: f32, rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * rate).max(1e-6)).exp()
}

/// A sine oscillator that rotates a phasor: phase-continuous, cheap per sample, and
/// cheap to retune. Call `normalize` now and then to keep its amplitude at 1.
#[derive(Clone, Copy)]
pub struct Rotor {
    re: f32,
    im: f32,
    cos: f32,
    sin: f32,
}

impl Rotor {
    pub fn new(phase: f32) -> Self {
        Self {
            re: phase.cos(),
            im: phase.sin(),
            cos: 1.0,
            sin: 0.0,
        }
    }

    pub fn set_frequency(&mut self, frequency: f32, sample_rate: f32) {
        let step = TAU * frequency / sample_rate;
        self.cos = step.cos();
        self.sin = step.sin();
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        let re = self.re * self.cos - self.im * self.sin;
        let im = self.re * self.sin + self.im * self.cos;
        self.re = re;
        self.im = im;
        im
    }

    pub fn normalize(&mut self) {
        let correction = 1.5 - 0.5 * (self.re * self.re + self.im * self.im);
        self.re *= correction;
        self.im *= correction;
    }
}

/// Zero-delay-feedback state-variable filter (Simper). Coefficients are set at control
/// rate; `tick` returns low-pass, normalized band-pass (unity gain at the centre) and
/// high-pass outputs.
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl Svf {
    pub fn new(frequency: f32, q: f32, sample_rate: f32) -> Self {
        let mut filter = Self::default();
        filter.set(frequency, q, sample_rate);
        filter
    }

    pub fn set(&mut self, frequency: f32, q: f32, sample_rate: f32) {
        let g = (PI * frequency.clamp(5.0, sample_rate * 0.45) / sample_rate).tan();
        self.k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    #[inline]
    pub fn tick(&mut self, input: f32) -> (f32, f32, f32) {
        let v3 = input - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, self.k * v1, input - self.k * v1 - v2)
    }

    #[inline]
    pub fn lowpass(&mut self, input: f32) -> f32 {
        self.tick(input).0
    }

    #[inline]
    pub fn bandpass(&mut self, input: f32) -> f32 {
        self.tick(input).1
    }

    #[inline]
    pub fn highpass(&mut self, input: f32) -> f32 {
        self.tick(input).2
    }
}

/// One-pole low-pass, also used as a smoother.
#[derive(Clone, Copy, Default)]
pub struct OnePole {
    pub value: f32,
    coefficient: f32,
}

impl OnePole {
    pub fn new(frequency: f32, sample_rate: f32) -> Self {
        let mut filter = Self::default();
        filter.set(frequency, sample_rate);
        filter
    }

    pub fn set(&mut self, frequency: f32, sample_rate: f32) {
        self.coefficient = 1.0 - (-TAU * frequency / sample_rate).exp();
    }

    #[inline]
    pub fn tick(&mut self, input: f32) -> f32 {
        self.value += self.coefficient * (input - self.value);
        self.value
    }
}

/// A smooth random walk in [0, 1]: eased steps between random targets. `advance` takes
/// the number of targets to move through, so its rate can change without jumps.
#[derive(Clone, Copy)]
pub struct Drift {
    from: f32,
    to: f32,
    position: f32,
}

impl Drift {
    pub fn new(rng: &mut Rng) -> Self {
        Self {
            from: rng.unit(),
            to: rng.unit(),
            position: rng.unit(),
        }
    }

    pub fn advance(&mut self, steps: f32, rng: &mut Rng) -> f32 {
        self.position += steps;
        while self.position >= 1.0 {
            self.position -= 1.0;
            self.from = self.to;
            self.to = rng.unit();
        }
        lerp(self.from, self.to, smooth(self.position))
    }
}

/// A delay line with whole and fractional (linearly interpolated) reads. Its length is a
/// power of two, so positions wrap with a mask.
pub struct DelayLine {
    buffer: Vec<f32>,
    mask: usize,
    position: usize,
}

impl DelayLine {
    pub fn new(max_seconds: f32, sample_rate: f32) -> Self {
        let length = ((max_seconds * sample_rate) as usize + 2).next_power_of_two();
        Self {
            buffer: vec![0.0; length],
            mask: length - 1,
            position: 0,
        }
    }

    /// The sample written `delay` writes ago, from 1 (the latest) to the length. Read
    /// before writing, it delays by `delay` samples.
    #[inline]
    pub fn tap(&self, delay: usize) -> f32 {
        let delay = delay.clamp(1, self.mask + 1);
        self.buffer[(self.position + 1).wrapping_sub(delay) & self.mask]
    }

    /// Like `tap`, between samples.
    #[inline]
    pub fn read(&self, delay: f32) -> f32 {
        let delay = delay.clamp(1.0, self.mask as f32);
        let whole = delay as usize;
        let fraction = delay - whole as f32;
        let a = self.buffer[(self.position + 1).wrapping_sub(whole) & self.mask];
        let b = self.buffer[self.position.wrapping_sub(whole) & self.mask];
        a + (b - a) * fraction
    }

    #[inline]
    pub fn write(&mut self, value: f32) {
        self.position = (self.position + 1) & self.mask;
        self.buffer[self.position] = value;
    }
}

/// Whether `step` (of `steps`) is a hit in the Euclidean pattern that spreads `pulses`
/// as evenly as possible, rotated left by `rotation`.
pub fn euclidean(pulses: usize, steps: usize, rotation: usize, step: usize) -> bool {
    pulses > 0 && ((step + rotation) % steps * pulses) % steps < pulses
}

/// Approximate magnitude of the K-weighting of loudness measurement (ITU-R BS.1770) at
/// `frequency`: a high-pass around 38 Hz and a +4 dB shelf above about 1.5 kHz.
pub fn k_weight(frequency: f32) -> f32 {
    let x = frequency / 38.0;
    let high_pass = x * x / ((1.0 - x * x).powi(2) + (x / 0.5).powi(2)).sqrt();
    let y = frequency / 1500.0;
    let shelf = ((1.0 + 2.51 * y * y) / (1.0 + y * y)).sqrt();
    high_pass * shelf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotor_keeps_frequency_and_amplitude() {
        let sample_rate = 48_000.0;
        let mut rotor = Rotor::new(0.0);
        rotor.set_frequency(1000.0, sample_rate);
        let mut crossings = 0;
        let mut previous = 0.0;
        let mut peak = 0.0_f32;
        for frame in 0..48_000 {
            let value = rotor.next();
            if previous < 0.0 && value >= 0.0 {
                crossings += 1;
            }
            previous = value;
            peak = peak.max(value.abs());
            if frame % 32 == 0 {
                rotor.normalize();
            }
        }
        assert!((999..=1001).contains(&crossings), "{crossings}");
        assert!((peak - 1.0).abs() < 1e-3);
    }

    #[test]
    fn delay_lines_delay_by_whole_and_fractional_samples() {
        let mut line = DelayLine::new(0.001, 1000.0);
        for value in 1..=5 {
            line.write(value as f32);
        }
        assert_eq!(line.tap(1), 5.0);
        assert_eq!(line.tap(3), 3.0);
        assert_eq!(line.read(2.5), 3.5);
        assert_eq!(line.tap(0), 5.0, "delays below one sample read the latest");
    }

    #[test]
    fn euclidean_patterns_spread_pulses_evenly() {
        let pattern = |pulses, steps, rotation| -> Vec<usize> {
            (0..steps)
                .filter(|&step| euclidean(pulses, steps, rotation, step))
                .collect()
        };
        assert_eq!(pattern(4, 16, 0), [0, 4, 8, 12]);
        assert_eq!(pattern(4, 16, 2), [2, 6, 10, 14]);
        assert_eq!(pattern(3, 8, 0), [0, 3, 6]);
        assert_eq!(pattern(0, 16, 0), Vec::<usize>::new());
        assert_eq!(pattern(5, 12, 0).len(), 5);
    }

    #[test]
    fn filters_pass_and_reject() {
        let sample_rate = 48_000.0;
        let energy = |frequency: f32, mut filter: Svf, output: usize| {
            let mut rotor = Rotor::new(0.0);
            rotor.set_frequency(frequency, sample_rate);
            let mut sum = 0.0;
            for frame in 0..48_000 {
                let (low, band, high) = filter.tick(rotor.next());
                if frame >= 24_000 {
                    let value = [low, band, high][output];
                    sum += value * value;
                }
            }
            (sum / 12_000.0).sqrt()
        };
        let filter = Svf::new(1000.0, 0.707, sample_rate);
        assert!(energy(100.0, filter, 0) > 0.95);
        assert!(energy(10_000.0, filter, 0) < 0.02);
        assert!(energy(10_000.0, filter, 2) > 0.95);
        let resonator = Svf::new(1000.0, 10.0, sample_rate);
        assert!((energy(1000.0, resonator, 1) - 1.0).abs() < 0.02);
        assert!(k_weight(30.0) < 0.5 && (k_weight(1000.0) - 1.1).abs() < 0.2);
    }
}
