//! The engine: shared musical time, the instrument, and the master.
//!
//! It plays from the controls as set, the tide clock and the features of the light, as the
//! bus carries them (`nova_core::Bus`). Once per block it moves Heat, Flow, Form and Void by
//! the global tides, as the display does, smooths them and the light, lets the conductor
//! move time and harmony, and lets the instrument read and write the signals; every sample
//! goes from the instrument through the master. It plays in the audio callback, so it
//! allocates only on construction.

mod conductor;
mod master;

pub use conductor::Conductor;

use nova_core::{Controls, Features, tides};

use crate::BLOCK;
use crate::dsp::coefficient;
use crate::harmony::{Harmony, Palette};
use crate::instruments::{Instrument, Preset};
use crate::params::Params;
use crate::signals::Signals;
use master::Master;

/// Range of visual activity the sound follows, on a logarithmic scale: calm content (about
/// 0.005) still differs, busy content (about 0.2) does not all sound the same.
const ACTIVITY_RANGE: [f32; 2] = [0.003, 0.3];

/// A quick rise of the activity above this multiple of its slow level, plus a margin, is an
/// onset, which adds a stab; onsets are at least `ONSET_COOLDOWN` seconds apart.
const ONSET_RATIO: f32 = 1.8;
const ONSET_MARGIN: f32 = 0.03;
const ONSET_COOLDOWN: f32 = 8.0;

/// Visual activity, followed slowly (2 s) for the sound and quickly (0.15 s) for onsets.
pub struct Activity {
    slow: f32,
    fast: f32,
    cooldown: f32,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            slow: 0.0,
            fast: 0.0,
            cooldown: ONSET_COOLDOWN,
        }
    }
}

impl Activity {
    /// Follows the activity of a frame `seconds` long; returns the level for the sound, from
    /// 0 to 1, and whether this is an onset.
    pub fn update(&mut self, activity: f32, seconds: f32) -> (f32, bool) {
        let activity = activity.min(2.0);
        self.slow += (activity - self.slow) * coefficient(2.0, 1.0 / seconds);
        self.fast += (activity - self.fast) * coefficient(0.15, 1.0 / seconds);
        self.cooldown -= seconds;
        let onset = self.cooldown <= 0.0 && self.fast > ONSET_RATIO * self.slow + ONSET_MARGIN;
        if onset {
            self.cooldown = ONSET_COOLDOWN;
        }
        let [low, high] = ACTIVITY_RANGE;
        let level = ((self.slow.max(low) / low).ln() / (high / low).ln()).min(1.0);
        (level, onset)
    }
}

pub struct Engine {
    sample_rate: f32,
    palettes: Vec<Palette>,
    /// As set, and the tide clock.
    controls: Controls,
    clock: f64,
    light: Features,
    params: Params,
    activity: Activity,
    conductor: Conductor,
    signals: Signals,
    instrument: Instrument,
    master: Master,
    position: usize,
    /// Whether the controls have been smoothed since the start; the first block takes
    /// them as they are.
    primed: bool,
}

impl Engine {
    /// An engine that plays `preset`; `seed` varies the details. It starts silent, until
    /// the controls set the volume.
    pub fn new(sample_rate: f32, seed: u64, preset: &Preset) -> Self {
        Self::with_palettes(sample_rate, seed, Palette::all(), preset)
    }

    /// An engine with other palettes than Nova's, for tests.
    pub(crate) fn with_palettes(
        sample_rate: f32,
        seed: u64,
        palettes: Vec<Palette>,
        preset: &Preset,
    ) -> Self {
        assert!(sample_rate.is_finite() && sample_rate >= 8000.0);
        assert!(palettes.len() >= nova_core::PALETTES.len());
        let harmony = Harmony::from_palette(&palettes[0]);
        Self {
            sample_rate,
            palettes,
            controls: Controls::default(),
            clock: 0.0,
            light: Features::default(),
            params: Params::default(),
            activity: Activity::default(),
            conductor: Conductor::new(sample_rate, harmony, preset.score(), seed),
            signals: Signals::new(harmony),
            instrument: Instrument::new(preset, sample_rate, &harmony, seed),
            master: Master::new(sample_rate),
            position: 0,
            primed: false,
        }
    }

    /// The controls as set; Volume applies after the complete mix.
    pub fn set_controls(&mut self, controls: Controls) {
        self.controls = controls.sanitized();
        self.master.set_volume(self.controls.volume);
    }

    /// The tide clock, in seconds.
    pub fn set_clock(&mut self, seconds: f64) {
        self.clock = if seconds.is_finite() { seconds } else { 0.0 };
    }

    /// The features of the light.
    pub fn set_light(&mut self, features: Features) {
        if features.is_finite() {
            self.light = features;
        }
    }

    fn update_block(&mut self) {
        let block_rate = self.sample_rate / BLOCK as f32;
        let controls = if self.primed {
            coefficient(0.08, block_rate)
        } else {
            self.primed = true;
            self.params.image = self.light;
            1.0
        };
        let target = tides::effective(&self.controls, self.clock);
        let params = &mut self.params;
        params.heat += (target.heat - params.heat) * controls;
        params.flow += (target.flow - params.flow) * controls;
        params.form += (target.form - params.form) * controls;
        params.void += (target.void - params.void) * controls;
        // The tempo follows the Flow slider without its tide, so it does not wobble.
        params.flow_setting += (self.controls.flow - params.flow_setting) * controls;

        let image = coefficient(2.5, block_rate);
        let (smoothed, features) = (&mut params.image, &self.light);
        for (smoothed, target) in [
            (&mut smoothed.level, features.level),
            (&mut smoothed.balance, features.balance),
            (&mut smoothed.width, features.width),
            (&mut smoothed.height, features.height),
            (&mut smoothed.density, features.density),
            (&mut smoothed.activity, features.activity),
        ] {
            *smoothed += (target - *smoothed) * image;
        }
        for (smoothed, &target) in smoothed.hues.iter_mut().zip(&features.hues) {
            *smoothed += (target - *smoothed) * image;
        }
        let (activity, onset) = self.activity.update(self.light.activity, 1.0 / block_rate);
        params.activity = activity;
        self.signals.onset = onset;

        let params = self.params;
        self.conductor.update_block(
            &params,
            &self.controls,
            self.clock,
            &self.palettes,
            &mut self.signals,
        );
        self.instrument.update_block(&params, &mut self.signals);
    }

    pub fn next_frame(&mut self) -> [f32; 2] {
        if self.position == 0 {
            self.update_block();
        }
        self.position = (self.position + 1) % BLOCK;
        let now = self.conductor.next();
        let mix = self.instrument.next(&now);
        self.master.process(mix)
    }

    /// The signals as of the latest block: tempo, harmony, the drone's dip and so on.
    pub fn signals(&self) -> &Signals {
        &self.signals
    }

    /// Steps the instrument's pulse has played, per voice: thumps, ticks and stabs.
    pub fn steps(&self) -> [u64; 3] {
        self.instrument.steps()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{Rng, Svf};
    use crate::harmony::tests::palettes;
    use crate::instruments::INSTRUMENTS;
    use crate::instruments::strata;
    use nova_core::{HUES, PALETTES};

    const RATE: f32 = 16_000.0;

    /// Every instrument, and Strata with its other rhythm.
    fn presets() -> Vec<(&'static str, Preset)> {
        let mut presets = INSTRUMENTS.to_vec();
        presets.push((
            "Strata (polymeter)",
            Preset::Strata(strata::Preset::POLYMETER),
        ));
        presets
    }

    /// Every control in the middle, the volume up.
    fn loud() -> Controls {
        Controls {
            volume: 1.0,
            ..Controls::default()
        }
    }

    fn engine(seed: u64, preset: &Preset) -> Engine {
        let mut engine = Engine::with_palettes(RATE, seed, palettes(), preset);
        engine.set_controls(loud());
        engine
    }

    fn rms(engine: &mut Engine, seconds: f32) -> f32 {
        let frames = (seconds * RATE) as usize;
        let mut sum = 0.0;
        for _ in 0..frames {
            let [left, right] = engine.next_frame();
            sum += left * left + right * right;
        }
        (sum / (2 * frames) as f32).sqrt()
    }

    #[test]
    fn output_is_finite_and_bounded_under_control_changes() {
        for (name, preset) in presets() {
            let mut engine = engine(7, &preset);
            let mut peak = 0.0_f32;
            let mut rng = Rng::new(3);
            for frame in 0..(RATE as usize * 12) {
                if frame % 4000 == 0 {
                    engine.set_controls(Controls {
                        palette: rng.below(PALETTES.len()),
                        heat: rng.unit().round(),
                        flow: rng.unit(),
                        form: rng.unit().round(),
                        void: rng.unit().round(),
                        ..loud()
                    });
                    engine.set_clock(rng.range(0.0, 1000.0) as f64);
                    let mut hues = [0.0; HUES];
                    hues[rng.below(HUES)] = 1.0;
                    engine.set_light(Features {
                        level: rng.unit(),
                        balance: rng.range(-1.0, 1.0),
                        width: rng.unit(),
                        height: rng.unit(),
                        density: rng.unit(),
                        activity: rng.unit(),
                        hues,
                    });
                }
                for sample in engine.next_frame() {
                    assert!(sample.is_finite());
                    peak = peak.max(sample.abs());
                }
            }
            assert!(peak > 0.05 && peak < 0.92, "{name}: peak {peak}");
        }
    }

    #[test]
    fn mute_silences_everything_including_tails() {
        for (name, preset) in presets() {
            let mut engine = engine(1, &preset);
            assert!(rms(&mut engine, 4.0) > 0.01, "{name}");
            engine.set_controls(Controls::default());
            rms(&mut engine, 0.05);
            for _ in 0..8000 {
                assert_eq!(engine.next_frame(), [0.0; 2], "{name}");
            }
        }
    }

    #[test]
    fn invalid_controls_are_silent_and_finite() {
        for (name, preset) in presets() {
            let mut engine = engine(1, &preset);
            engine.set_controls(Controls {
                brightness: f32::NAN,
                volume: f32::NAN,
                palette: usize::MAX,
                heat: f32::INFINITY,
                flow: -1.0,
                form: 2.0,
                void: f32::NEG_INFINITY,
            });
            engine.set_clock(f64::NAN);
            engine.set_light(Features {
                height: f32::NAN,
                activity: f32::INFINITY,
                hues: [f32::NAN; HUES],
                ..Features::default()
            });
            for _ in 0..8000 {
                assert_eq!(engine.next_frame(), [0.0; 2], "{name}");
            }
        }
    }

    #[test]
    fn form_and_flow_gate_the_pulse_but_never_the_drone() {
        for (name, preset) in presets() {
            for (form, flow, pulses) in [(0.0, 0.5, false), (0.8, 0.0, false), (0.8, 0.5, true)] {
                let mut engine = engine(5, &preset);
                engine.set_controls(Controls {
                    form,
                    flow,
                    ..loud()
                });
                let level = rms(&mut engine, 6.0);
                let steps = engine.steps();
                assert!(level > 0.01, "{name}, form {form}, flow {flow}: {level}");
                if pulses {
                    assert!(
                        steps.iter().all(|&count| count > 0),
                        "{name}, form {form}, flow {flow}: {steps:?}"
                    );
                } else {
                    assert_eq!(steps, [0; 3], "{name}, form {form}, flow {flow}");
                }
            }
        }
    }

    #[test]
    fn tempo_follows_the_flow_setting_and_stays_smooth() {
        let mut engine = engine(5, &Preset::default());
        engine.set_controls(Controls {
            flow: 0.0,
            ..loud()
        });
        rms(&mut engine, 0.1);
        let slow = engine.signals().tempo;
        engine.set_controls(Controls {
            flow: 1.0,
            ..loud()
        });
        let mut previous = slow;
        for _ in 0..300 {
            rms(&mut engine, 0.05);
            assert!((engine.signals().tempo - previous).abs() < 1.0);
            previous = engine.signals().tempo;
        }
        assert!(slow < 82.0 && previous > 108.0, "{slow} → {previous}");
    }

    #[test]
    fn void_thins_the_sound_but_never_silences_it() {
        for (name, preset) in presets() {
            for form in [0.0, 0.8] {
                let mut levels = Vec::new();
                for void in [0.0, 0.5, 1.0] {
                    let mut engine = engine(9, &preset);
                    engine.set_controls(Controls {
                        form,
                        void,
                        ..loud()
                    });
                    rms(&mut engine, 2.0);
                    levels.push(rms(&mut engine, 4.0));
                }
                assert!(levels[2] > levels[1] * 0.4, "{name}, {form}: {levels:?}");
                assert!(levels[0] < levels[1] * 2.5, "{name}, {form}: {levels:?}");
            }
        }
    }

    #[test]
    fn heat_keeps_the_loudness() {
        for (name, preset) in presets() {
            for form in [0.0, 0.8] {
                let levels: Vec<f32> = [0.0, 1.0]
                    .iter()
                    .map(|&heat| {
                        let mut engine = engine(4, &preset);
                        engine.set_controls(Controls {
                            form,
                            heat,
                            ..loud()
                        });
                        rms(&mut engine, 2.0);
                        rms(&mut engine, 8.0)
                    })
                    .collect();
                let ratio = 20.0 * (levels[1] / levels[0]).log10();
                // Plain RMS, led by the bass and the kick, which warmth lengthens.
                assert!(ratio.abs() < 5.0, "{name}, form {form}: {ratio} dB");
            }
        }
    }

    #[test]
    fn bass_is_mono_and_palette_changes_do_not_click() {
        for (name, preset) in presets() {
            let mut engine = engine(11, &preset);
            engine.set_controls(Controls {
                form: 0.0,
                heat: 1.0,
                ..loud()
            });
            rms(&mut engine, 2.0);
            // What a subwoofer sums: below about 80 Hz.
            let mut mid_filter = [Svf::new(80.0, 0.541, RATE), Svf::new(80.0, 1.307, RATE)];
            let mut side_filter = [Svf::new(80.0, 0.541, RATE), Svf::new(80.0, 1.307, RATE)];
            let (mut mid, mut side) = (0.0, 0.0);
            // A click is a sample step far above the steps the steady drone makes anyway:
            // compare the largest step of every 0.1 s window with the typical one.
            let mut previous = engine.next_frame();
            let mut steps = vec![0.0_f32; 90];
            for frame in 0..(RATE as usize * 9) {
                if frame % (RATE as usize * 3) == 0 {
                    engine.set_controls(Controls {
                        form: 0.0,
                        heat: 1.0,
                        palette: frame / (RATE as usize * 3) + 1,
                        ..loud()
                    });
                }
                let [left, right] = engine.next_frame();
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
            assert!(side < mid * 0.01, "{name}: side {side}, mid {mid}");
            let largest = steps.iter().copied().fold(0.0, f32::max);
            steps.sort_by(f32::total_cmp);
            let typical = steps[steps.len() / 2];
            assert!(
                largest < 2.5 * typical,
                "{name}: largest step {largest}, typical {typical}"
            );
        }
    }
}
