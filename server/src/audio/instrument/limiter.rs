//! A look-ahead peak limiter that never lets a sample through above its ceiling.
//!
//! For every sample it takes the gain that would bring the louder channel down to the
//! ceiling, holds the smallest such gain for the look-ahead time, lets it rise again slowly,
//! and averages it over the look-ahead time. The signal is delayed by the same time, so the
//! gain glides down before a peak and has arrived when the peak goes out.

pub struct Limiter {
    ceiling: f32,
    lookahead: usize,
    delayed: [Vec<f32>; 2],
    /// Gains of the last `lookahead` samples, after the release, and their sum.
    gains: Vec<f32>,
    sum: f64,
    position: usize,
    /// Required gains with the sample they were required for, increasing from the front, so
    /// the front is the smallest within the look-ahead time. A ring buffer.
    window: Vec<(f32, u64)>,
    front: usize,
    count: usize,
    released: f32,
    release: f32,
    time: u64,
}

impl Limiter {
    /// `ceiling` is the largest sample value let through; look-ahead and release are in
    /// seconds.
    pub fn new(sample_rate: f32, ceiling: f32, lookahead: f32, release: f32) -> Self {
        let lookahead = ((lookahead * sample_rate) as usize).max(1);
        Self {
            ceiling,
            lookahead,
            delayed: [vec![0.0; lookahead], vec![0.0; lookahead]],
            gains: vec![1.0; lookahead],
            sum: lookahead as f64,
            position: 0,
            window: vec![(1.0, 0); lookahead + 2],
            front: 0,
            count: 0,
            released: 1.0,
            release: 1.0 - (-1.0 / (release * sample_rate)).exp(),
            time: 0,
        }
    }

    #[inline]
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        let peak = input[0].abs().max(input[1].abs());
        let required = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };

        // The smallest required gain over the last `lookahead` + 1 samples.
        let length = self.window.len();
        while self.count > 0 && self.window[(self.front + self.count - 1) % length].0 >= required {
            self.count -= 1;
        }
        self.window[(self.front + self.count) % length] = (required, self.time);
        self.count += 1;
        while self.window[self.front].1 + (self.lookahead as u64) < self.time {
            self.front = (self.front + 1) % length;
            self.count -= 1;
        }
        let held = self.window[self.front].0;

        // Down at once, up slowly.
        self.released = if held < self.released {
            held
        } else {
            self.released + (held - self.released) * self.release
        };
        self.sum += (self.released - self.gains[self.position]) as f64;
        self.gains[self.position] = self.released;
        let gain = (self.sum / self.lookahead as f64) as f32;

        let mut output = [0.0; 2];
        for (channel, delayed) in self.delayed.iter_mut().enumerate() {
            output[channel] = delayed[self.position] * gain;
            delayed[self.position] = input[channel];
        }
        self.position = (self.position + 1) % self.lookahead;
        self.time += 1;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::dsp::Rng;

    const RATE: f32 = 48_000.0;

    #[test]
    fn peaks_never_exceed_the_ceiling() {
        let mut limiter = Limiter::new(RATE, 0.9, 0.003, 0.25);
        let mut rng = Rng::new(4);
        for frame in 0..(RATE as usize * 4) {
            // Bursts of up to 12 dB over the ceiling, and single spikes.
            let level = if (frame / 7000) % 2 == 0 { 3.5 } else { 0.5 };
            let spike = if frame % 9973 == 0 { 8.0 } else { 1.0 };
            let input = [rng.range(-level, level) * spike, rng.range(-level, level)];
            for sample in limiter.process(input) {
                assert!(sample.abs() <= 0.9 + 1e-5, "{sample} at {frame}");
            }
        }
    }

    #[test]
    fn quiet_signals_pass_unchanged_and_gain_recovers() {
        let mut limiter = Limiter::new(RATE, 0.9, 0.003, 0.25);
        let delay = (0.003 * RATE) as usize;
        let sine = |frame: usize| 0.5 * (frame as f32 * 0.01).sin();
        for frame in 0..(RATE as usize) {
            let output = limiter.process([sine(frame); 2])[0];
            if frame >= delay {
                assert!((output - sine(frame - delay)).abs() < 1e-6);
            }
        }
        limiter.process([4.0; 2]);
        let mut level_after = |seconds: f32| {
            let mut output = 0.0;
            for _ in 0..(seconds * RATE) as usize {
                output = limiter.process([0.5; 2])[0];
            }
            output
        };
        assert!(level_after(0.1) < 0.45, "held down after a peak");
        assert!(level_after(2.0) > 0.499, "and released");
    }
}
