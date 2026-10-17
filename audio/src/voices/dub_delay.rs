//! The dub delay: a tempo-synced ping-pong (a dotted eighth) with a band-pass, gentle
//! saturation and tape flutter in its loop; warmer means more feedback and darker repeats.
//! A boost throws it: more feedback, darker and more saturated repeats, as when a dub
//! engineer pushes the echo for a moment. The echoes start on the left unless told to start
//! on the right; the input glides between the two over a few milliseconds.

use crate::BLOCK;
use crate::dsp::{DelayLine, OnePole, Svf, coefficient, lerp, smooth};

/// Feedback never goes beyond this, however hard the delay is thrown.
const MAX_FEEDBACK: f32 = 0.88;

/// Seconds the input takes to move from one side to the other.
const START_SECONDS: f32 = 0.01;

/// Each value when cold and warm.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub feedback: [f32; 2],
    /// Corners of the band-pass in the loop, in Hz.
    pub high_cut: [f32; 2],
    pub low_cut: [f32; 2],
    /// Drive of the saturation in the loop.
    pub drive: [f32; 2],
}

impl Settings {
    pub const DEFAULT: Self = Self {
        feedback: [0.42, 0.66],
        high_cut: [3500.0, 1600.0],
        low_cut: [250.0, 250.0],
        drive: [1.3, 1.3],
    };
}

pub struct DubDelay {
    sample_rate: f32,
    settings: Settings,
    lines: [DelayLine; 2],
    low_cut: [OnePole; 2],
    high_cut: [Svf; 2],
    time: f32,
    feedback: f32,
    drive: f32,
    flutter: f32,
    flutter_step: f32,
    seconds: f32,
    /// Where the input goes in, from 0 (the left line) to 1 (the right), and where it is
    /// going.
    start: f32,
    start_target: f32,
}

impl DubDelay {
    pub fn new(sample_rate: f32, settings: Settings) -> Self {
        Self {
            sample_rate,
            settings,
            lines: [
                DelayLine::new(1.5, sample_rate),
                DelayLine::new(1.5, sample_rate),
            ],
            low_cut: [OnePole::new(settings.low_cut[0], sample_rate); 2],
            high_cut: [Svf::new(3000.0, 0.7, sample_rate); 2],
            time: 0.0,
            feedback: 0.5,
            drive: 1.0,
            flutter: 0.0,
            flutter_step: 0.0,
            seconds: 0.0,
            start: 0.0,
            start_target: 0.0,
        }
    }

    /// Lets the echoes start on the left (0) or on the right (1).
    pub fn set_start(&mut self, side: f32) {
        self.start_target = side.clamp(0.0, 1.0);
    }

    /// `boost` from 0 throws the delay: it adds feedback (up to `MAX_FEEDBACK`), lowers the
    /// high cut by two octaves per unit and drives the loop harder.
    pub fn update_block(&mut self, tempo: f32, heat: f32, boost: f32) {
        let settings = &self.settings;
        let block_seconds = BLOCK as f32 / self.sample_rate;
        self.seconds += block_seconds;
        // A dotted eighth: three 16ths.
        let target = 3.0 * 15.0 / tempo * self.sample_rate;
        if self.time == 0.0 {
            self.time = target;
        }
        self.time += (target - self.time) * coefficient(1.5, 1.0 / block_seconds);
        let warmth = smooth(heat);
        let boost = boost.max(0.0);
        self.feedback =
            (lerp(settings.feedback[0], settings.feedback[1], warmth) + boost).min(MAX_FEEDBACK);
        self.drive = lerp(settings.drive[0], settings.drive[1], warmth) * (1.0 + boost);
        let high_cut =
            lerp(settings.high_cut[0], settings.high_cut[1], warmth) * 2.0_f32.powf(-2.0 * boost);
        for filter in &mut self.high_cut {
            filter.set(high_cut, 0.7, self.sample_rate);
        }
        let low_cut = lerp(settings.low_cut[0], settings.low_cut[1], warmth);
        for filter in &mut self.low_cut {
            filter.set(low_cut, self.sample_rate);
        }
        let flutter = 0.0004
            * self.sample_rate
            * (std::f32::consts::TAU * 0.7 * self.seconds).sin()
            * (0.5 + 0.5 * warmth);
        self.flutter_step = (flutter - self.flutter) / BLOCK as f32;
    }

    fn shape(&mut self, input: f32, channel: usize) -> f32 {
        let band = self.high_cut[channel].lowpass(input - self.low_cut[channel].tick(input));
        (self.drive * band).tanh() / self.drive
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> [f32; 2] {
        self.flutter += self.flutter_step;
        let glide = 1.0 / (START_SECONDS * self.sample_rate);
        self.start += (self.start_target - self.start).clamp(-glide, glide);
        let time = self.time + self.flutter;
        let left = self.lines[0].read(time);
        let right = self.lines[1].read(time);
        let into_right = self.shape(left, 0);
        let into_left = self.shape(right, 1);
        self.lines[0].write(input * (1.0 - self.start) + into_left * self.feedback);
        self.lines[1].write(input * self.start + into_right * self.feedback);
        [left, right]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Rng;

    /// RMS of the delay's output over `seconds` after a burst of noise, at a boost.
    fn tail(heat: f32, boost: f32, seconds: f32) -> f32 {
        let rate = 16_000.0;
        let mut delay = DubDelay::new(rate, Settings::DEFAULT);
        let mut rng = Rng::new(1);
        let mut sum = 0.0;
        let frames = (seconds * rate) as usize;
        for frame in 0..frames {
            if frame % BLOCK == 0 {
                delay.update_block(100.0, heat, boost);
            }
            let input = if frame < 1600 {
                rng.range(-1.0, 1.0)
            } else {
                0.0
            };
            let [left, right] = delay.process(input);
            assert!(left.is_finite() && right.abs() < 4.0);
            if frame >= frames - 16_000 {
                sum += left * left + right * right;
            }
        }
        (sum / 32_000.0).sqrt()
    }

    #[test]
    fn a_boost_throws_the_echoes_longer_but_they_still_die_away() {
        let plain = tail(1.0, 0.0, 6.0);
        let thrown = tail(1.0, 0.3, 6.0);
        assert!(thrown > 4.0 * plain, "{plain} vs {thrown}");
        for (heat, boost) in [(0.0, 0.46), (1.0, 0.3), (1.0, 2.0)] {
            assert!(tail(heat, boost, 40.0) < 1e-3, "feedback stays below 1");
        }
    }

    #[test]
    fn echoes_start_on_the_side_asked_for() {
        let rate = 16_000.0;
        for (side, first) in [(0.0, 0), (1.0, 1)] {
            let mut delay = DubDelay::new(rate, Settings::DEFAULT);
            delay.set_start(side);
            // Energy per channel until the first echo has passed: at 100 BPM, a dotted
            // eighth (0.45 s) after the click, the second one as long again.
            let mut energy = [0.0_f32; 2];
            for frame in 0..(0.8 * rate) as usize {
                if frame % BLOCK == 0 {
                    delay.update_block(100.0, 0.5, 0.0);
                }
                let input = if (1000..1100).contains(&frame) {
                    1.0
                } else {
                    0.0
                };
                let output = delay.process(input);
                if frame < 1100 + (0.6 * rate) as usize {
                    for (sum, sample) in energy.iter_mut().zip(output) {
                        *sum += sample * sample;
                    }
                }
            }
            assert!(
                energy[first] > 0.0 && energy[1 - first] == 0.0,
                "side {side}: {energy:?}"
            );
        }
    }
}
