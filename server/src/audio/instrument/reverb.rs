//! A long, smooth, modulated reverb: a feedback delay network (FDN) after a diffuser.
//!
//! The stereo input is spread over `CHANNELS` channels. Each diffusion step delays every
//! channel by a different time, flips some polarities and mixes all channels with a Hadamard
//! matrix, which turns every sound into a dense cloud within about 0.15 s: the early sound.
//! The cloud feeds `CHANNELS` delay lines of 65 to 190 ms that feed back into each other
//! through a Householder reflection, each with a gain for the decay time and a low-pass for
//! damping. Slow, independent modulation of the feedback delays keeps long tails from
//! ringing. Delays and polarities are seeded, so every instance sounds the same.
//!
//! `CHANNELS` and `DIFFUSION_STEPS` trade quality for cost: more channels give a denser,
//! smoother tail, more steps a softer onset; the cost grows with both.

use crate::audio::dsp::{DelayLine, OnePole, Rng, Rotor};

/// Channels of the diffuser and the network; a power of two.
const CHANNELS: usize = 16;

const DIFFUSION_STEPS: usize = 4;

/// Longest delay of the first diffusion step; every further step doubles it.
const DIFFUSION_SECONDS: f32 = 0.011;

/// Range of the feedback delays, which sets the size of the space.
const SHORTEST_SECONDS: f32 = 0.065;
const LONGEST_SECONDS: f32 = 0.19;

/// Depth and range of rates of the modulation of the feedback delays.
const MODULATION_SECONDS: f32 = 0.0012;
const MODULATION_RATES: [f32; 2] = [0.07, 0.31];

/// Level of the early sound next to the tail.
const EARLY_LEVEL: f32 = 0.35;

const SEED: u64 = 0x4e6f_7661;

/// Mixes all channels with the normalized Hadamard matrix (fast Walsh–Hadamard transform),
/// keeping the energy.
fn hadamard(values: &mut [f32; CHANNELS]) {
    let mut half = 1;
    while half < CHANNELS {
        for start in (0..CHANNELS).step_by(2 * half) {
            for index in start..start + half {
                let (a, b) = (values[index], values[index + half]);
                values[index] = a + b;
                values[index + half] = a - b;
            }
        }
        half *= 2;
    }
    let scale = 1.0 / (CHANNELS as f32).sqrt();
    for value in values {
        *value *= scale;
    }
}

struct DiffusionStep {
    lines: [DelayLine; CHANNELS],
    delays: [usize; CHANNELS],
    polarities: [f32; CHANNELS],
}

impl DiffusionStep {
    /// Spreads the delays evenly but randomly up to `seconds`, one slot per channel.
    fn new(seconds: f32, sample_rate: f32, rng: &mut Rng) -> Self {
        let delays: [usize; CHANNELS] = std::array::from_fn(|channel| {
            let slot = (channel as f32 + rng.unit()) / CHANNELS as f32;
            ((slot * seconds * sample_rate) as usize).max(1)
        });
        Self {
            lines: std::array::from_fn(|channel| {
                DelayLine::new(delays[channel] as f32 / sample_rate, sample_rate)
            }),
            delays,
            polarities: std::array::from_fn(|_| if rng.unit() < 0.5 { -1.0 } else { 1.0 }),
        }
    }

    #[inline]
    fn process(&mut self, values: &mut [f32; CHANNELS]) {
        for (channel, value) in values.iter_mut().enumerate() {
            let line = &mut self.lines[channel];
            let delayed = line.tap(self.delays[channel]);
            line.write(*value);
            *value = delayed * self.polarities[channel];
        }
        hadamard(values);
    }
}

struct FeedbackLine {
    line: DelayLine,
    /// Delay in samples, before modulation.
    delay: f32,
    modulation: Rotor,
    damping: OnePole,
    /// Gain per pass, for the decay time.
    gain: f32,
}

pub struct Reverb {
    sample_rate: f32,
    diffusion: [DiffusionStep; DIFFUSION_STEPS],
    lines: [FeedbackLine; CHANNELS],
    depth: f32,
    /// Scales the input to the network, so its loudness does not depend on the decay time.
    input_gain: f32,
}

impl Reverb {
    /// A reverb with a decay time of 4 s and damping at 6 kHz.
    pub fn new(sample_rate: f32) -> Self {
        let mut rng = Rng::new(SEED);
        let diffusion = std::array::from_fn(|step| {
            DiffusionStep::new(
                DIFFUSION_SECONDS * (1 << step) as f32,
                sample_rate,
                &mut rng,
            )
        });
        let lines = std::array::from_fn(|channel| {
            // Spread exponentially, so the delays share no common pattern.
            let slot = (channel as f32 + rng.unit()) / CHANNELS as f32;
            let seconds = SHORTEST_SECONDS * (LONGEST_SECONDS / SHORTEST_SECONDS).powf(slot);
            let mut modulation = Rotor::new(rng.range(0.0, std::f32::consts::TAU));
            modulation.set_frequency(
                rng.range(MODULATION_RATES[0], MODULATION_RATES[1]),
                sample_rate,
            );
            FeedbackLine {
                line: DelayLine::new(seconds + MODULATION_SECONDS, sample_rate),
                delay: seconds * sample_rate,
                modulation,
                damping: OnePole::default(),
                gain: 0.0,
            }
        });
        let mut reverb = Self {
            sample_rate,
            diffusion,
            lines,
            depth: MODULATION_SECONDS * sample_rate,
            input_gain: 0.0,
        };
        reverb.set_decay(4.0);
        reverb.set_damping(6000.0);
        reverb
    }

    /// The time in which the tail decays by 60 dB, in seconds.
    pub fn set_decay(&mut self, seconds: f32) {
        let seconds = seconds.max(0.1);
        let mut power = 0.0;
        for line in &mut self.lines {
            line.gain = 10.0_f32.powf(-3.0 * line.delay / self.sample_rate / seconds);
            power += line.gain * line.gain;
        }
        // Energy builds up by 1 / (1 − g²) over the passes.
        self.input_gain = (1.0 - power / CHANNELS as f32).sqrt();
    }

    /// Cutoff of the damping in the feedback loop, in Hz: lower is darker, and high
    /// frequencies die away faster.
    pub fn set_damping(&mut self, frequency: f32) {
        for line in &mut self.lines {
            line.damping.set(frequency, self.sample_rate);
        }
    }

    /// Call every few dozen samples, to keep the modulation exact.
    pub fn update_block(&mut self) {
        for line in &mut self.lines {
            line.modulation.normalize();
        }
    }

    #[inline]
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        let spread = (2.0 / CHANNELS as f32).sqrt();
        let mut early: [f32; CHANNELS] = std::array::from_fn(|channel| input[channel % 2] * spread);
        for step in &mut self.diffusion {
            step.process(&mut early);
        }

        let mut delayed = [0.0; CHANNELS];
        for (value, line) in delayed.iter_mut().zip(&mut self.lines) {
            let delay = line.delay + line.modulation.next() * self.depth;
            *value = line.damping.tick(line.line.read(delay));
        }
        // A Householder reflection mixes every line into every other, keeping the energy.
        let reflection = delayed.iter().sum::<f32>() * (2.0 / CHANNELS as f32);
        for ((line, &value), &early) in self.lines.iter_mut().zip(&delayed).zip(&early) {
            line.line
                .write(early * self.input_gain + (value - reflection) * line.gain);
        }

        let mut output = [0.0; 2];
        for channel in 0..CHANNELS {
            output[channel % 2] += delayed[channel] + EARLY_LEVEL * early[channel];
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// Mean power of both output channels over `seconds`, with noise or silence as input.
    fn power(reverb: &mut Reverb, rng: &mut Rng, seconds: f32, noise: bool) -> f32 {
        let frames = (seconds * RATE) as usize;
        let mut sum = 0.0;
        for frame in 0..frames {
            if frame % 32 == 0 {
                reverb.update_block();
            }
            let input = if noise {
                [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)]
            } else {
                [0.0; 2]
            };
            let [left, right] = reverb.process(input);
            assert!(left.is_finite() && right.is_finite());
            sum += left * left + right * right;
        }
        sum / (2 * frames) as f32
    }

    fn decibels(ratio: f32) -> f32 {
        10.0 * ratio.log10()
    }

    #[test]
    fn hadamard_mixing_keeps_energy() {
        let mut values: [f32; CHANNELS] = std::array::from_fn(|channel| channel as f32 - 3.0);
        let energy: f32 = values.iter().map(|value| value * value).sum();
        hadamard(&mut values);
        let mixed: f32 = values.iter().map(|value| value * value).sum();
        assert!((mixed / energy - 1.0).abs() < 1e-5);
    }

    #[test]
    fn loudness_does_not_depend_on_the_decay_time() {
        let mut levels = Vec::new();
        for decay in [2.0, 8.0] {
            let mut reverb = Reverb::new(RATE);
            reverb.set_decay(decay);
            reverb.set_damping(20_000.0);
            let mut rng = Rng::new(1);
            power(&mut reverb, &mut rng, decay, true);
            // White noise has a power of 1/3.
            levels.push(decibels(power(&mut reverb, &mut rng, 2.0, true) * 3.0));
        }
        assert!(
            levels.iter().all(|level| level.abs() < 4.0),
            "{levels:?} dB"
        );
        assert!((levels[0] - levels[1]).abs() < 3.0, "{levels:?} dB");
    }

    #[test]
    fn tail_decays_at_the_set_time() {
        let mut reverb = Reverb::new(RATE);
        reverb.set_decay(2.0);
        reverb.set_damping(20_000.0);
        let mut rng = Rng::new(2);
        power(&mut reverb, &mut rng, 2.0, true);
        power(&mut reverb, &mut rng, 0.2, false);
        let early = power(&mut reverb, &mut rng, 0.2, false);
        power(&mut reverb, &mut rng, 0.8, false);
        let late = power(&mut reverb, &mut rng, 0.2, false);
        // 1 s apart at 60 dB per 2 s.
        let drop = decibels(early / late);
        assert!((24.0..36.0).contains(&drop), "{drop} dB");
    }

    #[test]
    fn mono_input_comes_out_wide() {
        let mut reverb = Reverb::new(RATE);
        let mut rng = Rng::new(3);
        let (mut left_power, mut right_power, mut product) = (0.0, 0.0, 0.0);
        for frame in 0..(RATE as usize * 4) {
            let value = rng.range(-1.0, 1.0);
            let [left, right] = reverb.process([value; 2]);
            if frame >= RATE as usize {
                left_power += left * left;
                right_power += right * right;
                product += left * right;
            }
        }
        let correlation = product / (left_power * right_power).sqrt();
        assert!(correlation.abs() < 0.3, "correlation {correlation}");
    }
}
