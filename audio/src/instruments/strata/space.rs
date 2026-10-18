//! Space: dub delay, reverb and distance.
//!
//! The pulse feeds the dub delay; everything feeds the reverb (see `voices/reverb.rs`), whose
//! input is high-passed so the bass stays dry and clean. Void moves the sound away: less and
//! darker dry signal above the bass, more reverb. The reverb follows the drone's dip. The
//! glow of the light brightens the hall, and light gathered in one place narrows the stereo
//! image above the bass.

use super::pulse::PulseOut;
use crate::BLOCK;
use crate::dsp::{Svf, lerp, smoothstep};
use crate::params::Params;
use crate::voices::{DubDelay, Reverb, dub_delay};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Decay time of the reverb (−60 dB), in seconds, and its damping in Hz.
    pub reverb_seconds: f32,
    pub reverb_damping: f32,
    /// Level of the reverb at Void 1; it keeps about the loudness of its input, and at Void
    /// 0.5 it is about as loud.
    pub reverb_level: f32,
    /// Level of the dub delay's echoes in the dry signal.
    pub echo_level: f32,
    /// How far the echoes dip with the drone (with the thumps), from 0 to 1.
    pub echo_duck: f32,
    /// How far the glow of the light moves the hall's damping, in octaves either way: up
    /// for lit voxels at full brightness, down for dim ones, not for those at half (see
    /// `Features::glow`); 0 ignores the glow.
    pub glow: f32,
    /// Width of the stereo image above the bass for light gathered in a single column, as
    /// a factor of the side signal; light spread out keeps it whole (see
    /// `Features::width`). 1 ignores the width.
    pub narrow: f32,
    pub dub: dub_delay::Settings,
}

impl Settings {
    pub const DEFAULT: Self = Self {
        reverb_seconds: 8.0,
        reverb_damping: 4000.0,
        reverb_level: 1.55,
        echo_level: 0.6,
        echo_duck: 0.0,
        glow: 1.0,
        narrow: 0.6,
        dub: dub_delay::Settings::DEFAULT,
    };
}

pub struct Space {
    sample_rate: f32,
    settings: Settings,
    dub: DubDelay,
    reverb: Reverb,
    /// Fourth-order Butterworth high-pass per channel, so the bass stays out of the reverb.
    reverb_input: [[Svf; 2]; 2],
    split: [Svf; 2],
    damping: [Svf; 2],
    dry: f32,
    dry_step: f32,
    wet: f32,
    wet_step: f32,
    /// The hall's damping in Hz now, and the side signal's gain above the bass.
    damping_hz: f32,
    side: f32,
    side_step: f32,
}

impl Space {
    pub fn new(sample_rate: f32, settings: Settings) -> Self {
        let mut reverb = Reverb::new(sample_rate);
        reverb.set_decay(settings.reverb_seconds);
        reverb.set_damping(settings.reverb_damping);
        Self {
            sample_rate,
            settings,
            dub: DubDelay::new(sample_rate, settings.dub),
            reverb,
            reverb_input: [[
                Svf::new(150.0, 0.541, sample_rate),
                Svf::new(150.0, 1.307, sample_rate),
            ]; 2],
            split: [Svf::new(120.0, 0.707, sample_rate); 2],
            damping: [Svf::new(18_000.0, 0.6, sample_rate); 2],
            dry: 1.0,
            dry_step: 0.0,
            wet: settings.reverb_level,
            wet_step: 0.0,
            damping_hz: settings.reverb_damping,
            side: 1.0,
            side_step: 0.0,
        }
    }

    /// Changes the level of the reverb (see `Settings::reverb_level`) from the next block on.
    pub fn set_reverb_level(&mut self, level: f32) {
        self.settings.reverb_level = level;
    }

    /// Lets the dub delay's echoes start on the left (0) or on the right (1).
    pub fn set_echo_start(&mut self, side: f32) {
        self.dub.set_start(side);
    }

    /// `echo` is the dub delay's time in 16ths, `boost` throws it (see `voices/dub_delay.rs`).
    pub fn update_block(&mut self, params: &Params, tempo: f32, echo: f32, boost: f32) {
        let distance = params.void;
        self.dry_step = (lerp(1.0, 0.45, distance) - self.dry) / BLOCK as f32;
        self.wet_step =
            (lerp(0.35, 1.0, distance) * self.settings.reverb_level - self.wet) / BLOCK as f32;
        let cutoff = 12_000.0 * (3000.0_f32 / 12_000.0).powf(distance);
        for filter in &mut self.damping {
            filter.set(cutoff, 0.6, self.sample_rate);
        }
        // The glow of what is lit, once something is.
        let light = &params.image;
        let lit = smoothstep(0.0, 0.05, light.density);
        let glow = (2.0 * light.glow() - 1.0).clamp(-1.0, 1.0) * lit;
        let damping = self.settings.reverb_damping * 2.0_f32.powf(self.settings.glow * glow);
        if (damping / self.damping_hz - 1.0).abs() > 0.002 {
            self.damping_hz = damping;
            self.reverb.set_damping(damping);
        }
        let side = lerp(self.settings.narrow, 1.0, smoothstep(0.1, 0.8, light.width));
        self.side_step = (side - self.side) / BLOCK as f32;
        self.dub.update_block(tempo, echo, params.heat, boost);
        self.reverb.update_block();
    }

    #[inline]
    pub fn process(&mut self, bed: [f32; 2], air: [f32; 2], pulse: PulseOut) -> [f32; 2] {
        self.dry += self.dry_step;
        self.wet += self.wet_step;
        let duck = 1.0 - self.settings.echo_duck * (1.0 - pulse.drone);
        let echoes = self.dub.process(pulse.delay_send).map(|echo| echo * duck);

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
        let wet = self.reverb.process(send);

        self.side += self.side_step;
        let (mut low, mut high) = ([0.0; 2], [0.0; 2]);
        for channel in 0..2 {
            let dry = bed[channel]
                + air[channel]
                + pulse.dry[channel]
                + self.settings.echo_level * echoes[channel];
            low[channel] = self.split[channel].lowpass(dry);
            high[channel] = self.damping[channel].lowpass(dry - low[channel]) * self.dry
                + wet[channel] * self.wet * pulse.drone;
        }
        // Narrower above the bass as the light gathers.
        let mid = 0.5 * (high[0] + high[1]);
        let side = 0.5 * (high[0] - high[1]) * self.side;
        [low[0] + mid + side, low[1] + mid - side]
    }
}
