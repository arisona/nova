//! The instrument: drone bed, air, pulse and space (see `doc/audio_concept.md`).
//!
//! Work in progress: the sketches render it offline, driven by the real content, until it
//! replaces the live synth. Like the synth, it allocates only on construction.

mod bed;
mod pulse;
mod space;

use super::dsp::{Rng, coefficient, smoothstep};
use super::features::{Features, LAYERS};
use crate::palettes::{MAX_COLORS, PALETTES, Palette};

/// Samples between control updates.
const BLOCK: usize = 32;

/// Seconds over which the output fades in after construction.
const FADE_IN_SECONDS: f32 = 1.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    pub volume: f32,
    /// Position in `PALETTES`.
    pub palette: usize,
    /// Heat, Flow, Form and Void are the effective values, after the tides.
    pub heat: f32,
    pub flow: f32,
    pub form: f32,
    pub void: f32,
    /// The Flow slider without its tide, for the tempo, so the tempo does not wobble.
    pub flow_setting: f32,
}

impl Controls {
    fn sanitized(self) -> Self {
        fn level(value: f32) -> f32 {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        Self {
            volume: level(self.volume),
            palette: self.palette.min(PALETTES.len() - 1),
            heat: level(self.heat),
            flow: level(self.flow),
            form: level(self.form),
            void: level(self.void),
            flow_setting: level(self.flow_setting),
        }
    }
}

/// How the pulse places its steps: everything on one 16-step bar, or loops of 16, 12 and
/// 7 steps that phase against each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rhythm {
    Euclidean,
    Polymeter,
}

/// Smoothed controls and features, handed to the layers once per block.
#[derive(Clone, Copy, Debug)]
struct Params {
    heat: f32,
    flow: f32,
    form: f32,
    void: f32,
    flow_setting: f32,
    /// Visual activity, mapped to [0, 1].
    activity: f32,
    /// Share of the light per layer, bottom first.
    layers: [f32; LAYERS],
}

impl Params {
    /// Weight of a frequency band from the light in the matching layer: ten bands from
    /// 40 Hz to 10 kHz, low at the bottom and high at the top of the display.
    fn band_weight(&self, frequency: f32) -> f32 {
        let position = ((frequency / 40.0).ln() / 250.0_f32.ln() * (LAYERS - 1) as f32)
            .clamp(0.0, (LAYERS - 1) as f32);
        let below = position as usize;
        let above = (below + 1).min(LAYERS - 1);
        let fraction = position - below as f32;
        let share = self.layers[below] + (self.layers[above] - self.layers[below]) * fraction;
        (1.0 + 0.5 * (share * LAYERS as f32 - 1.0)).clamp(0.3, 2.2)
    }
}

/// Maximum number of tones in a harmony: the root and one per palette color.
const MAX_TONES: usize = 1 + MAX_COLORS;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tone {
    frequency: f32,
    level: f32,
}

/// The tonal world of a palette.
///
/// The chroma-weighted mean hue picks the root on the circle of fifths, between C1 and B1
/// (33 to 62 Hz). Every color adds a tone: its hue distance from the mean gives the
/// interval in steps on the circle of fifths (close hues give fifths and fourths, contrasting
/// hues thirds and sixths, all in just intonation), its lightness the octave, and its chroma
/// the level. Whites add an octave.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Harmony {
    tones: [Tone; MAX_TONES],
    count: usize,
}

/// Just ratios by steps on the circle of fifths from the root, from −4 to 4.
const FIFTHS: [f32; 9] = [
    8.0 / 5.0,
    6.0 / 5.0,
    7.0 / 4.0,
    4.0 / 3.0,
    1.0,
    3.0 / 2.0,
    9.0 / 8.0,
    5.0 / 3.0,
    5.0 / 4.0,
];

impl Harmony {
    fn from_palette(palette: &Palette) -> Self {
        let (mut x, mut y) = (0.0_f32, 0.0_f32);
        for color in palette.colors {
            let (_, chroma, hue) = color.oklch();
            x += chroma * hue.to_radians().cos();
            y += chroma * hue.to_radians().sin();
        }
        let mean = if x.hypot(y) > 1e-3 {
            y.atan2(x).to_degrees().rem_euclid(360.0)
        } else {
            0.0
        };
        // Neighbouring hues lie a fifth apart: 30° per step around the circle of fifths.
        let pitch_class = ((mean / 30.0).round() as i32 * 7).rem_euclid(12);
        let root = 440.0 * 2.0_f32.powf((24 + pitch_class - 69) as f32 / 12.0);
        let mut tones = [Tone::default(); MAX_TONES];
        tones[0] = Tone {
            frequency: root,
            level: 1.0,
        };
        let mut count = 1;
        for color in palette.colors.iter().take(MAX_COLORS) {
            let (lightness, chroma, hue) = color.oklch();
            let ratio = if chroma < 0.03 {
                2.0
            } else {
                let distance = (hue - mean + 540.0).rem_euclid(360.0) - 180.0;
                FIFTHS[(((distance / 40.0).round() as i32).clamp(-4, 4) + 4) as usize]
            };
            let octave = (1 + (lightness * 3.2) as i32).clamp(1, 4);
            let mut frequency = root * ratio * 2.0_f32.powi(octave);
            // Colors that land on the same tone move up an octave rather than double it.
            for _ in 0..2 {
                if tones[..count]
                    .iter()
                    .any(|tone| (tone.frequency / frequency - 1.0).abs() < 0.01)
                {
                    frequency *= 2.0;
                }
            }
            tones[count] = Tone {
                frequency,
                level: (0.3 + 2.0 * chroma).min(0.9),
            };
            count += 1;
        }
        Self { tones, count }
    }

    fn tones(&self) -> &[Tone] {
        &self.tones[..self.count]
    }

    fn root(&self) -> f32 {
        self.tones[0].frequency
    }
}

pub struct Instrument {
    sample_rate: f32,
    target: Controls,
    features: Features,
    params: Params,
    /// Raw visual activity, smoothed slowly and quickly, for the level and for onsets.
    activity_slow: f32,
    activity_fast: f32,
    palette: usize,
    bed: bed::Bed,
    air: bed::Air,
    pulse: pulse::Pulse,
    space: space::Space,
    gain: f32,
    fade_in: f32,
    position: usize,
    /// Whether the controls have been smoothed since the start; the first block takes
    /// them as they are.
    primed: bool,
}

impl Instrument {
    pub fn new(sample_rate: f32, seed: u64, rhythm: Rhythm) -> Self {
        assert!(sample_rate.is_finite() && sample_rate >= 8000.0);
        let mut rng = Rng::new(seed);
        let harmony = Harmony::from_palette(&PALETTES[0]);
        let params = Params {
            heat: 0.5,
            flow: 0.5,
            form: 0.5,
            void: 0.5,
            flow_setting: 0.5,
            activity: 0.0,
            layers: Features::default().layers,
        };
        Self {
            sample_rate,
            target: Controls {
                heat: 0.5,
                flow: 0.5,
                form: 0.5,
                void: 0.5,
                flow_setting: 0.5,
                ..Default::default()
            },
            features: Features::default(),
            params,
            activity_slow: 0.0,
            activity_fast: 0.0,
            palette: 0,
            bed: bed::Bed::new(sample_rate, &harmony, Rng::new(rng.next_u64())),
            air: bed::Air::new(sample_rate, &harmony, Rng::new(rng.next_u64())),
            pulse: pulse::Pulse::new(sample_rate, rhythm, &harmony, Rng::new(rng.next_u64())),
            space: space::Space::new(sample_rate),
            gain: 0.0,
            fade_in: 0.0,
            position: 0,
            primed: false,
        }
    }

    pub fn set_controls(&mut self, controls: Controls) {
        self.target = controls.sanitized();
    }

    pub fn set_features(&mut self, features: Features) {
        if features.activity.is_finite() && features.layers.iter().all(|layer| layer.is_finite()) {
            self.features = features;
        }
    }

    fn update_block(&mut self) {
        let block_rate = self.sample_rate / BLOCK as f32;
        let controls = if self.primed {
            coefficient(0.08, block_rate)
        } else {
            self.primed = true;
            self.params.layers = self.features.layers;
            self.palette = self.target.palette;
            let harmony = Harmony::from_palette(&PALETTES[self.palette]);
            self.bed.load(&harmony);
            self.air.load(&harmony);
            self.pulse.adopt(&harmony);
            1.0
        };
        let params = &mut self.params;
        params.heat += (self.target.heat - params.heat) * controls;
        params.flow += (self.target.flow - params.flow) * controls;
        params.form += (self.target.form - params.form) * controls;
        params.void += (self.target.void - params.void) * controls;
        params.flow_setting += (self.target.flow_setting - params.flow_setting) * controls;

        let layers = coefficient(2.5, block_rate);
        for (smoothed, &layer) in params.layers.iter_mut().zip(&self.features.layers) {
            *smoothed += (layer - *smoothed) * layers;
        }
        let activity = self.features.activity.min(2.0);
        self.activity_slow += (activity - self.activity_slow) * coefficient(2.0, block_rate);
        self.activity_fast += (activity - self.activity_fast) * coefficient(0.15, block_rate);
        params.activity = 1.0 - (-self.activity_slow / 0.06).exp();
        if self.activity_fast > 1.8 * self.activity_slow + 0.03 {
            self.pulse.request_accent();
        }

        if self.target.palette != self.palette && !self.bed.fading() {
            self.palette = self.target.palette;
            let harmony = Harmony::from_palette(&PALETTES[self.palette]);
            self.bed.change(&harmony);
            self.air.change(&harmony);
            self.pulse.change(&harmony);
        }

        let params = self.params;
        self.bed.update_block(&params);
        self.air.update_block(&params);
        self.pulse.update_block(&params);
        self.space.update_block(&params, self.pulse.tempo());
    }

    pub fn next_frame(&mut self) -> [f32; 2] {
        if self.position == 0 {
            self.update_block();
        }
        self.position = (self.position + 1) % BLOCK;

        let bed = self.bed.next();
        let air = self.air.next();
        let pulse = self.pulse.next();
        let mixed = self.space.process(bed, air, pulse);

        let ramp = 1.0 / (0.02 * self.sample_rate);
        let target_gain = self.target.volume * self.target.volume;
        self.gain += (target_gain - self.gain).clamp(-ramp, ramp);
        self.fade_in = (self.fade_in + 1.0 / (FADE_IN_SECONDS * self.sample_rate)).min(1.0);
        let gain = self.gain * smoothstep(0.0, 1.0, self.fade_in);
        mixed.map(|sample| {
            if sample.is_finite() {
                sample * gain
            } else {
                0.0
            }
        })
    }

    /// Steps the pulse has played, per voice: thumps, ticks and stabs.
    #[cfg(test)]
    pub fn pulse_counts(&self) -> [u64; 3] {
        self.pulse.counts()
    }

    /// Current tempo in beats per minute.
    #[cfg(test)]
    pub fn tempo(&self) -> f32 {
        self.pulse.tempo()
    }
}

#[cfg(test)]
mod tests {
    use super::super::dsp::Svf;
    use super::*;

    const RATE: f32 = 16_000.0;

    fn controls() -> Controls {
        Controls {
            volume: 1.0,
            palette: 0,
            heat: 0.5,
            flow: 0.5,
            form: 0.5,
            void: 0.5,
            flow_setting: 0.5,
        }
    }

    fn rms(instrument: &mut Instrument, seconds: f32) -> f32 {
        let frames = (seconds * RATE) as usize;
        let mut sum = 0.0;
        for _ in 0..frames {
            let [left, right] = instrument.next_frame();
            sum += left * left + right * right;
        }
        (sum / (2 * frames) as f32).sqrt()
    }

    #[test]
    fn harmonies_stay_in_range_and_in_just_intonation() {
        for palette in PALETTES {
            let harmony = Harmony::from_palette(palette);
            assert_eq!(harmony.count, 1 + palette.colors.len());
            assert!((32.0..62.0).contains(&harmony.root()), "{}", palette.name);
            for tone in &harmony.tones()[1..] {
                assert!(tone.frequency > harmony.root() * 1.9, "{}", palette.name);
                assert!(tone.frequency < harmony.root() * 128.0, "{}", palette.name);
                assert!((0.3..=0.9).contains(&tone.level));
                let ratio = tone.frequency / harmony.root();
                let octaves = ratio / 2.0_f32.powi(ratio.log2().floor() as i32);
                assert!(
                    FIFTHS
                        .iter()
                        .chain([2.0].iter())
                        .any(|just| (just - octaves).abs() < 1e-4 || (octaves - 1.0).abs() < 1e-4),
                    "{}: {ratio}",
                    palette.name
                );
            }
        }
    }

    #[test]
    fn output_is_finite_and_bounded_under_control_changes() {
        let mut instrument = Instrument::new(RATE, 7, Rhythm::Euclidean);
        let mut peak = 0.0_f32;
        let mut rng = Rng::new(3);
        for frame in 0..(RATE as usize * 12) {
            if frame % 4000 == 0 {
                instrument.set_controls(Controls {
                    volume: 1.0,
                    palette: rng.below(PALETTES.len()),
                    heat: rng.unit().round(),
                    flow: rng.unit(),
                    form: rng.unit().round(),
                    void: rng.unit().round(),
                    flow_setting: 1.0,
                });
                let mut layers = [0.0; LAYERS];
                layers[rng.below(LAYERS)] = 1.0;
                instrument.set_features(Features {
                    layers,
                    activity: rng.unit(),
                });
            }
            for sample in instrument.next_frame() {
                assert!(sample.is_finite());
                peak = peak.max(sample.abs());
            }
        }
        assert!(peak > 0.05 && peak < 0.92, "peak {peak}");
    }

    #[test]
    fn mute_silences_everything_including_tails() {
        let mut instrument = Instrument::new(RATE, 1, Rhythm::Euclidean);
        instrument.set_controls(controls());
        assert!(rms(&mut instrument, 4.0) > 0.01);
        instrument.set_controls(Controls {
            volume: 0.0,
            ..controls()
        });
        rms(&mut instrument, 0.05);
        for _ in 0..8000 {
            assert_eq!(instrument.next_frame(), [0.0; 2]);
        }
    }

    #[test]
    fn invalid_controls_are_silent_and_finite() {
        let mut instrument = Instrument::new(RATE, 1, Rhythm::Polymeter);
        instrument.set_controls(Controls {
            volume: f32::NAN,
            palette: usize::MAX,
            heat: f32::INFINITY,
            flow: -1.0,
            form: 2.0,
            void: f32::NEG_INFINITY,
            flow_setting: f32::NAN,
        });
        instrument.set_features(Features {
            layers: [f32::NAN; LAYERS],
            activity: f32::INFINITY,
        });
        for _ in 0..8000 {
            assert_eq!(instrument.next_frame(), [0.0; 2]);
        }
    }

    #[test]
    fn form_and_flow_gate_the_pulse_but_never_the_drone() {
        for (form, flow, pulses) in [(0.0, 0.5, false), (0.8, 0.0, false), (0.8, 0.5, true)] {
            let mut instrument = Instrument::new(RATE, 5, Rhythm::Euclidean);
            instrument.set_controls(Controls {
                form,
                flow,
                flow_setting: flow,
                ..controls()
            });
            let level = rms(&mut instrument, 6.0);
            let counts = instrument.pulse_counts();
            assert!(level > 0.01, "form {form}, flow {flow}: {level}");
            assert_eq!(
                counts[0] > 0,
                pulses,
                "form {form}, flow {flow}: {counts:?}"
            );
        }
    }

    #[test]
    fn tempo_follows_the_flow_setting_and_stays_smooth() {
        let mut instrument = Instrument::new(RATE, 5, Rhythm::Euclidean);
        instrument.set_controls(Controls {
            flow_setting: 0.0,
            ..controls()
        });
        rms(&mut instrument, 0.1);
        let slow = instrument.tempo();
        instrument.set_controls(Controls {
            flow_setting: 1.0,
            ..controls()
        });
        let mut previous = slow;
        for _ in 0..300 {
            rms(&mut instrument, 0.05);
            assert!((instrument.tempo() - previous).abs() < 1.0);
            previous = instrument.tempo();
        }
        assert!(slow < 66.0 && previous > 120.0, "{slow} → {previous}");
    }

    #[test]
    fn void_thins_the_sound_but_never_silences_it() {
        let mut levels = Vec::new();
        for void in [0.0, 0.5, 1.0] {
            let mut instrument = Instrument::new(RATE, 9, Rhythm::Euclidean);
            instrument.set_controls(Controls {
                form: 0.0,
                void,
                ..controls()
            });
            rms(&mut instrument, 2.0);
            levels.push(rms(&mut instrument, 2.0));
        }
        assert!(levels[2] > levels[1] * 0.4, "{levels:?}");
        assert!(levels[0] < levels[1] * 2.5, "{levels:?}");
    }

    #[test]
    fn bass_is_mono_and_palette_changes_do_not_click() {
        let mut instrument = Instrument::new(RATE, 11, Rhythm::Euclidean);
        instrument.set_controls(Controls {
            form: 0.0,
            heat: 1.0,
            ..controls()
        });
        rms(&mut instrument, 2.0);
        // What a subwoofer sums: below about 80 Hz.
        let mut mid_filter = [Svf::new(80.0, 0.541, RATE), Svf::new(80.0, 1.307, RATE)];
        let mut side_filter = [Svf::new(80.0, 0.541, RATE), Svf::new(80.0, 1.307, RATE)];
        let (mut mid, mut side) = (0.0, 0.0);
        // A click is a sample step far above the steps the steady drone makes anyway:
        // compare the largest step of every 0.1 s window with the typical one.
        let mut previous = instrument.next_frame();
        let mut steps = vec![0.0_f32; 90];
        for frame in 0..(RATE as usize * 9) {
            if frame % (RATE as usize * 3) == 0 {
                instrument.set_controls(Controls {
                    form: 0.0,
                    heat: 1.0,
                    palette: frame / (RATE as usize * 3) + 1,
                    ..controls()
                });
            }
            let [left, right] = instrument.next_frame();
            let lowpass = |filters: &mut [Svf; 2], value: f32| {
                filters
                    .iter_mut()
                    .fold(value, |value, filter| filter.lowpass(value))
            };
            let low_mid = lowpass(&mut mid_filter, 0.5 * (left + right));
            let low_side = lowpass(&mut side_filter, 0.5 * (left - right));
            mid += low_mid * low_mid;
            side += low_side * low_side;
            let window = &mut steps[frame * 10 / RATE as usize];
            *window = window
                .max((left - previous[0]).abs())
                .max((right - previous[1]).abs());
            previous = [left, right];
        }
        assert!(side < mid * 0.01, "side {side}, mid {mid}");
        let largest = steps.iter().copied().fold(0.0, f32::max);
        steps.sort_by(f32::total_cmp);
        let typical = steps[steps.len() / 2];
        assert!(
            largest < 2.5 * typical,
            "largest step {largest}, typical {typical}"
        );
    }
}
