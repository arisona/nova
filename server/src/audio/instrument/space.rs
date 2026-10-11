//! Space and master: dub delay, reverb, distance, bass management and limiting.
//!
//! The dub delay is a tempo-synced ping-pong (a dotted eighth) with a band-pass, gentle
//! saturation and tape flutter in its loop; warmer means more feedback and darker repeats.
//! The reverb is a long, modulated FDN; its input is high-passed so the bass stays dry and
//! clean. Void moves the sound away: less and darker dry signal above the bass, more reverb.
//! The master keeps everything below 120 Hz mono (subwoofers sum their channels), removes
//! infrasound and limits peaks to about −1 dBFS with a short look-ahead.

use fundsp::prelude32::{AudioUnit, limiter_stereo, lowpole_hz, reverb2_stereo};

use super::pulse::PulseOut;
use super::{BLOCK, Params};
use crate::audio::dsp::{DelayLine, OnePole, Svf, coefficient, lerp, smooth};

/// The limiter holds peaks at about 0.91 of its input scale; scaling by this keeps them
/// at about −1 dBFS.
const CEILING: f32 = 0.98;

/// The reverb's steady-state gain is about −5 dB; at Void 0.5 it sits about 9 dB below
/// the dry signal.
const REVERB_LEVEL: f32 = 1.0;
const ECHO_LEVEL: f32 = 0.6;

struct DubDelay {
    sample_rate: f32,
    lines: [DelayLine; 2],
    low_cut: [OnePole; 2],
    high_cut: [Svf; 2],
    time: f32,
    feedback: f32,
    flutter: f32,
    flutter_step: f32,
    seconds: f32,
}

impl DubDelay {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            lines: [
                DelayLine::new(1.5, sample_rate),
                DelayLine::new(1.5, sample_rate),
            ],
            low_cut: [OnePole::new(250.0, sample_rate); 2],
            high_cut: [Svf::new(3000.0, 0.7, sample_rate); 2],
            time: 0.0,
            feedback: 0.5,
            flutter: 0.0,
            flutter_step: 0.0,
            seconds: 0.0,
        }
    }

    fn update_block(&mut self, tempo: f32, heat: f32) {
        let block_seconds = BLOCK as f32 / self.sample_rate;
        self.seconds += block_seconds;
        // A dotted eighth: three 16ths.
        let target = 3.0 * 15.0 / tempo * self.sample_rate;
        if self.time == 0.0 {
            self.time = target;
        }
        self.time += (target - self.time) * coefficient(1.5, 1.0 / block_seconds);
        let warmth = smooth(heat);
        self.feedback = lerp(0.42, 0.66, warmth);
        for filter in &mut self.high_cut {
            filter.set(lerp(3500.0, 1600.0, warmth), 0.7, self.sample_rate);
        }
        let flutter = 0.0004
            * self.sample_rate
            * (std::f32::consts::TAU * 0.7 * self.seconds).sin()
            * (0.5 + 0.5 * warmth);
        self.flutter_step = (flutter - self.flutter) / BLOCK as f32;
    }

    fn shape(&mut self, input: f32, channel: usize) -> f32 {
        let band = self.high_cut[channel].lowpass(input - self.low_cut[channel].tick(input));
        (1.3 * band).tanh() / 1.3
    }

    #[inline]
    fn process(&mut self, input: f32) -> [f32; 2] {
        self.flutter += self.flutter_step;
        let time = self.time + self.flutter;
        let left = self.lines[0].read(time);
        let right = self.lines[1].read(time);
        let into_right = self.shape(left, 0);
        let into_left = self.shape(right, 1);
        self.lines[0].write(input + into_left * self.feedback);
        self.lines[1].write(into_right * self.feedback);
        [left, right]
    }
}

pub struct Space {
    sample_rate: f32,
    dub: DubDelay,
    reverb: Box<dyn AudioUnit>,
    /// Fourth-order Butterworth high-pass per channel, so the bass stays out of the reverb.
    reverb_input: [[Svf; 2]; 2],
    split: [Svf; 2],
    damping: [Svf; 2],
    /// Fourth-order Butterworth high-pass for the side signal.
    side: [Svf; 2],
    infrasonic: [Svf; 2],
    limiter: Box<dyn AudioUnit>,
    dry: f32,
    dry_step: f32,
    wet: f32,
    wet_step: f32,
}

impl Space {
    pub fn new(sample_rate: f32) -> Self {
        let mut reverb: Box<dyn AudioUnit> =
            Box::new(reverb2_stereo(28.0, 8.0, 0.9, 0.6, lowpole_hz(5000.0)));
        let mut limiter: Box<dyn AudioUnit> = Box::new(limiter_stereo(0.003, 0.25));
        for unit in [&mut reverb, &mut limiter] {
            unit.set_sample_rate(sample_rate as f64);
            unit.allocate();
        }
        Self {
            sample_rate,
            dub: DubDelay::new(sample_rate),
            reverb,
            reverb_input: [[
                Svf::new(150.0, 0.541, sample_rate),
                Svf::new(150.0, 1.307, sample_rate),
            ]; 2],
            split: [Svf::new(120.0, 0.707, sample_rate); 2],
            damping: [Svf::new(18_000.0, 0.6, sample_rate); 2],
            side: [
                Svf::new(120.0, 0.541, sample_rate),
                Svf::new(120.0, 1.307, sample_rate),
            ],
            infrasonic: [Svf::new(18.0, 0.707, sample_rate); 2],
            limiter,
            dry: 1.0,
            dry_step: 0.0,
            wet: REVERB_LEVEL,
            wet_step: 0.0,
        }
    }

    pub fn update_block(&mut self, params: &Params, tempo: f32) {
        let distance = params.void;
        self.dry_step = (lerp(1.0, 0.45, distance) - self.dry) / BLOCK as f32;
        self.wet_step = (lerp(0.35, 1.0, distance) * REVERB_LEVEL - self.wet) / BLOCK as f32;
        let cutoff = 18_000.0 * (4000.0_f32 / 18_000.0).powf(distance);
        for filter in &mut self.damping {
            filter.set(cutoff, 0.6, self.sample_rate);
        }
        self.dub.update_block(tempo, params.heat);
    }

    #[inline]
    pub fn process(&mut self, bed: [f32; 2], air: [f32; 2], pulse: PulseOut) -> [f32; 2] {
        self.dry += self.dry_step;
        self.wet += self.wet_step;
        let echoes = self.dub.process(pulse.delay_send);

        let mut send = [0.0; 2];
        for channel in 0..2 {
            let input = bed[channel]
                + 1.5 * air[channel]
                + pulse.reverb_send[channel]
                + 0.5 * echoes[channel];
            send[channel] = self.reverb_input[channel]
                .iter_mut()
                .fold(input, |input, filter| filter.highpass(input));
        }
        let mut wet = [0.0; 2];
        self.reverb.tick(&send, &mut wet);

        let mut output = [0.0; 2];
        for channel in 0..2 {
            let dry =
                bed[channel] + air[channel] + pulse.dry[channel] + ECHO_LEVEL * echoes[channel];
            let low = self.split[channel].lowpass(dry);
            let high = self.damping[channel].lowpass(dry - low) * self.dry;
            output[channel] = low + high + wet[channel] * self.wet;
        }

        let mid = 0.5 * (output[0] + output[1]);
        let side = self
            .side
            .iter_mut()
            .fold(0.5 * (output[0] - output[1]), |side, filter| {
                filter.highpass(side)
            });
        let stereo = [mid + side, mid - side];
        let mut input = [0.0; 2];
        for channel in 0..2 {
            input[channel] = self.infrasonic[channel].highpass(stereo[channel]) / CEILING;
        }
        let mut limited = [0.0; 2];
        self.limiter.tick(&input, &mut limited);
        limited.map(|sample| sample * CEILING)
    }
}
