//! Small DSP building blocks for the voices. Everything allocates only on construction, so
//! it can run in the audio callback.

mod limiter;

use std::f32::consts::{PI, TAU};

pub use limiter::Limiter;

/// A small, fast, deterministic random generator (SplitMix64), so seeded voices replay
/// the same sound.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

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

/// A transfer curve through points `[x, y]`, in order of x, eased between each pair: flat
/// at every point, so plateaus and soft shoulders come for free, and never beyond the
/// values of its points. Before the first point and after the last it holds their values.
#[derive(Clone, Copy, Debug)]
pub struct Curve(pub &'static [[f32; 2]]);

impl Curve {
    pub fn at(&self, x: f32) -> f32 {
        let points = self.0;
        if x <= points[0][0] {
            return points[0][1];
        }
        for pair in points.windows(2) {
            let [[x0, y0], [x1, y1]] = [pair[0], pair[1]];
            if x <= x1 {
                return lerp(y0, y1, smoothstep(x0, x1, x));
            }
        }
        points[points.len() - 1][1]
    }
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

    /// The next sample. An oscillator, not an iterator: it never ends.
    #[allow(clippy::should_implement_trait)]
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

    /// Sets a state that has nearly died away to zero, so a silent filter does not compute
    /// with subnormal numbers, which are slow on some processors. Call it now and then.
    pub fn settle(&mut self) {
        if self.ic1.abs() < 1e-15 {
            self.ic1 = 0.0;
        }
        if self.ic2.abs() < 1e-15 {
            self.ic2 = 0.0;
        }
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

/// Envelope with a click-free retrigger: the previous note's level decays quickly while the
/// new one rises.
#[derive(Clone, Copy, Default)]
pub struct Envelope {
    age: f32,
    carry: f32,
    velocity: f32,
    value: f32,
}

impl Envelope {
    pub fn trigger(&mut self, velocity: f32) {
        self.carry = self.value;
        self.velocity = velocity;
        self.age = 0.0;
    }

    /// Whether it has never been triggered, or has been silent for `seconds`.
    pub fn idle(&self, seconds: f32) -> bool {
        self.velocity == 0.0 || self.age > seconds
    }

    /// Seconds since the latest trigger.
    pub fn age(&self) -> f32 {
        self.age
    }

    #[inline]
    pub fn next(&mut self, attack: f32, decay: f32, step: f32) -> f32 {
        self.value = self.carry * (-self.age / 0.003).exp()
            + self.velocity * smooth(self.age / attack) * (-self.age / decay).exp();
        self.age += step;
        self.value
    }
}

/// An envelope for swells and stabs: it attacks along an S-curve from wherever it is, so a
/// retrigger never dips, holds at full level, then decays exponentially. Times are fixed at
/// the trigger, so a note keeps its shape whatever the controls do meanwhile.
#[derive(Clone, Copy, Default)]
pub struct Contour {
    value: f32,
    start: f32,
    age: f32,
    attack: f32,
    hold: f32,
    /// Factor per sample while decaying.
    decay: f32,
    step: f32,
}

impl Contour {
    /// Attack and hold in seconds; `release` is the time constant of the decay, in seconds.
    pub fn trigger(&mut self, attack: f32, hold: f32, release: f32, sample_rate: f32) {
        self.trigger_from(self.value, attack, hold, release, sample_rate);
    }

    /// Like `trigger`, from another level: for a new note whose loudness differs, so the
    /// product of both stays continuous. Above 1, the attack eases down to 1.
    pub fn trigger_from(
        &mut self,
        start: f32,
        attack: f32,
        hold: f32,
        release: f32,
        sample_rate: f32,
    ) {
        self.start = start;
        self.value = start;
        self.age = 0.0;
        self.attack = attack.max(1e-4);
        self.hold = hold.max(0.0);
        self.decay = (-1.0 / (release.max(1e-4) * sample_rate)).exp();
        self.step = 1.0 / sample_rate;
    }

    /// The level now, from 0 to 1.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The next sample; silent until the first trigger.
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.step == 0.0 {
            return 0.0;
        }
        self.value = if self.age < self.attack {
            lerp(self.start, 1.0, smooth(self.age / self.attack))
        } else if self.age < self.attack + self.hold {
            1.0
        } else {
            // Down to exactly zero at −180 dB, rather than to a subnormal it would never leave.
            let value = self.value * self.decay;
            if value < 1e-9 { 0.0 } else { value }
        };
        self.age += self.step;
        self.value
    }
}

/// Magnitude of a resonant low-pass at `frequency`: a fourth-order Butterworth response
/// (24 dB per octave) times an explicit resonance bump around `cutoff`, so the peak height
/// is set directly: `resonance` 0 is flat, and the response at the cutoff is
/// (1 + `resonance`) / √2. The bump is about half an octave wide. Applied to additive
/// partials, it filters without aliasing or phase.
#[inline]
pub fn resonant_lowpass(frequency: f32, cutoff: f32, resonance: f32) -> f32 {
    let x = frequency / cutoff;
    let x4 = x * x * x * x;
    let distance = (x - 1.0 / x.max(1e-6)) / 0.35;
    (1.0 + resonance / (1.0 + distance * distance)) / (1.0 + x4 * x4).sqrt()
}

/// The `resonance` that makes `resonant_lowpass` peak about `decibels` above the passband
/// at its cutoff.
pub fn resonance(decibels: f32) -> f32 {
    (10.0_f32.powf(decibels / 20.0) * std::f32::consts::SQRT_2 - 1.0).max(0.0)
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
    fn curves_pass_their_points_and_stay_within_them() {
        let bump = Curve(&[[0.0, 0.0], [0.1, 1.0], [0.5, 1.0], [1.0, 0.0]]);
        for [x, y] in bump.0 {
            assert_eq!(bump.at(*x), *y);
        }
        assert_eq!((bump.at(-1.0), bump.at(0.3), bump.at(2.0)), (0.0, 1.0, 0.0));
        assert_eq!(bump.at(0.75), 0.5);
        // Flat at the points: a small step away barely moves it.
        assert!(bump.at(0.505) > 0.999 && bump.at(0.005) < 0.01);
        assert!((0..=100).all(|i| (0.0..=1.0).contains(&bump.at(i as f32 / 100.0))));
    }

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
    fn contours_attack_from_where_they_are_hold_and_decay() {
        let rate = 1000.0;
        let mut contour = Contour::default();
        assert_eq!(contour.next(), 0.0, "silent until triggered");
        contour.trigger(0.01, 0.02, 0.1, rate);
        let values: Vec<f32> = (0..200).map(|_| contour.next()).collect();
        assert!(
            values[..10].windows(2).all(|pair| pair[1] >= pair[0]),
            "rises"
        );
        assert_eq!(values[15], 1.0, "holds");
        // One time constant after the hold: 1/e.
        assert!(
            (values[130] - (-1.0_f32).exp()).abs() < 0.02,
            "{}",
            values[130]
        );
        // A retrigger while decaying starts from the current level, without a dip.
        let before = contour.value();
        contour.trigger(0.05, 0.0, 0.1, rate);
        assert!(contour.next() >= before);
        // Long after, it is exactly silent.
        for _ in 0..20_000 {
            contour.next();
        }
        assert_eq!(contour.value(), 0.0);
    }

    #[test]
    fn resonant_lowpass_passes_peaks_and_rolls_off() {
        assert!((resonant_lowpass(10.0, 1000.0, 0.0) - 1.0).abs() < 1e-3);
        assert!(
            (resonant_lowpass(1000.0, 1000.0, 0.0) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5
        );
        // 24 dB per octave well above the cutoff.
        let octave = resonant_lowpass(8000.0, 1000.0, 0.0) / resonant_lowpass(4000.0, 1000.0, 0.0);
        assert!((20.0 * octave.log10() + 24.0).abs() < 0.5, "{octave}");
        // The resonance sets the height at the cutoff, and the peak stays near it.
        for decibels in [3.0, 10.0] {
            let at_cutoff = resonant_lowpass(1000.0, 1000.0, resonance(decibels));
            assert!((20.0 * at_cutoff.log10() - decibels).abs() < 1e-3);
            let peak = (500..2000)
                .map(|f| resonant_lowpass(f as f32, 1000.0, resonance(decibels)))
                .fold(0.0, f32::max);
            assert!(20.0 * peak.log10() < decibels + 0.6, "{decibels}: {peak}");
        }
        assert!(
            resonant_lowpass(4000.0, 1000.0, resonance(10.0)) < 0.02,
            "the bump stays narrow"
        );
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
