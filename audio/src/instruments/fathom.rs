//! Fathom: the bed's chord played as dub stabs (see `doc/audio_concept.md`).
//!
//! The chord of the bed, folded into a compact register, is played as off-beat stabs
//! through a resonant low-pass that opens and closes with every hit (see `fathom/chord.rs`).
//! Only the stab goes into the dub delay, so the echoes and a dark hall carry the groove;
//! a deep sub bass and a soft kick stay underneath. At low Form it sinks into a dark,
//! filtered drone. Bed, air, kick, ticks and space are shared with Strata.

pub mod chord;

use crate::dsp::{Rng, Svf};
use crate::harmony::Harmony;
use crate::instruments::strata::pulse::{self, Pulse, Rhythm};
use crate::instruments::strata::space::{self, Space};
use crate::params::Params;
use crate::signals::{Change, Now, Signals};
use crate::voices::{Air, Bed, bed, dub_delay};
use chord::Chord;

pub use chord::Throws;

/// Everything that sets Fathom's character, as data.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub bed: bed::Settings,
    /// Kick and ticks, with the drone's dip and the bass accent; its stabs and chord gate
    /// stay off.
    pub pulse: pulse::Settings,
    pub chord: chord::Settings,
    pub space: space::Settings,
    /// Level of the air.
    pub air: f32,
    /// Level of the reverb for the drone and for stabs (see `space::Settings::reverb_level`):
    /// the drone sinks into the hall, stabs stand out from it.
    pub reverb: [f32; 2],
}

/// The register the chord is folded into, in Hz.
const REGISTER: [f32; 2] = [180.0, 720.0];

impl Preset {
    pub const DEFAULT: Self = Self {
        bed: bed::Settings {
            // Pulse-like and glassy when cold, saw-like when warm; the low-pass shapes it.
            cold: [1.0, 0.08, 0.34, 0.06, 0.2, 0.05, 0.14, 0.04],
            warm: [1.0, 0.5, 0.33, 0.25, 0.2, 0.16, 0.13, 0.11],
            // A deep, nearly sine sub bass.
            bass_cold: [1.0, 0.1, 0.2, 0.05, 0.08, 0.02, 0.03, 0.01],
            bass_warm: [1.0, 0.35, 0.15, 0.08, 0.04, 0.02, 0.01, 0.0],
            bass_rolloff: [300.0, 220.0],
            rolloff: 6000.0,
            // Crisp attacks.
            breath: [0.03, 0.12],
            detune_cents: 6.0,
            sweep: [4.0, 8.0],
            // The bass moves like a dub bassline.
            glide_seconds: 1.5,
            voice_seconds: 3.0,
            voice_range: REGISTER,
            bass_accent: [0.8, 1.6],
            register: Some(REGISTER),
            // The filter answers the height of the light instead (see `chord::Settings`).
            height_tilt: 0.0,
            ..bed::Settings::DEFAULT
        },
        pulse: pulse::Settings {
            // Kick from Form 0.4, half time and then four to the floor, never three, which
            // would land on the stabs; ticks from Form 0.5; no stabs of its own.
            starts: [0.4, 0.5, f32::INFINITY],
            thump_counts: [1, 2, 4, 4],
            thump_level: 0.22,
            tick_level: 0.12,
            stab_level: 0.0,
            chord_gate: 0.0,
            rhythm: Rhythm::Euclidean,
            ..pulse::Settings::DEFAULT
        },
        chord: chord::Settings::DEFAULT,
        space: space::Settings {
            // A darker hall than Strata's.
            reverb_seconds: 7.0,
            reverb_damping: 3000.0,
            echo_level: 0.75,
            echo_duck: 0.4,
            dub: dub_delay::Settings {
                feedback: [0.5, 0.7],
                high_cut: [3200.0, 1400.0],
                low_cut: [320.0, 220.0],
                drive: [1.2, 1.8],
            },
            ..space::Settings::DEFAULT
        },
        air: 0.5,
        reverb: [1.55, 0.6],
    };

    /// This preset with other throws.
    pub const fn with_throws(mut self, throws: Throws) -> Self {
        self.chord.throws = throws;
        self
    }
}

pub struct Fathom {
    bed: Bed,
    air: Air,
    pulse: Pulse,
    chord: Chord,
    space: Space,
    air_level: f32,
    reverb: [f32; 2],
    /// Fourth-order Butterworth high-pass on the send, so the delay stays out of the bass.
    send_filter: [Svf; 2],
}

impl Fathom {
    /// Starts from `harmony` until the conductor loads the first one.
    pub fn new(sample_rate: f32, preset: &Preset, harmony: &Harmony, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let register = preset.bed.register.unwrap_or(REGISTER);
        Self {
            bed: Bed::new(sample_rate, preset.bed, harmony, Rng::new(rng.next_u64())),
            air: Air::new(sample_rate, harmony, Rng::new(rng.next_u64())),
            pulse: Pulse::new(sample_rate, preset.pulse, harmony, Rng::new(rng.next_u64())),
            chord: Chord::new(
                sample_rate,
                preset.chord,
                register,
                harmony,
                Rng::new(rng.next_u64()),
            ),
            space: Space::new(sample_rate, preset.space),
            air_level: preset.air,
            reverb: preset.reverb,
            send_filter: [
                Svf::new(160.0, 0.541, sample_rate),
                Svf::new(160.0, 1.307, sample_rate),
            ],
        }
    }

    pub fn update_block(&mut self, params: &Params, signals: &mut Signals) {
        // The pulse as it is after the previous block, for the bed and anyone else.
        self.pulse.publish(signals);
        let harmony = &signals.harmony;
        match signals.change {
            Some(Change::Load) => {
                self.bed.load(harmony);
                self.air.load(harmony);
                self.pulse.adopt(harmony);
                self.chord.set_harmony(harmony, true);
            }
            Some(Change::Palette) => {
                self.bed.change(harmony);
                self.air.change(harmony);
                self.pulse.change(harmony);
                self.chord.set_harmony(harmony, false);
                self.chord.arm_section();
            }
            Some(Change::Bass) => {
                self.bed.set_bass(harmony.root());
                self.air.change(harmony);
                self.pulse.change(harmony);
                self.chord.arm_section();
            }
            Some(Change::Voice) => self.bed.move_voice(),
            None => {}
        }
        self.chord.update_block(params, signals);
        self.bed.set_filter(Some(self.chord.filter()));
        for filter in &mut self.send_filter {
            filter.settle();
        }
        self.bed.update_block(params, signals);
        self.air.update_block(params);
        self.pulse.update_block(params, signals);
        let [drone, stabs] = self.reverb;
        self.space
            .set_reverb_level(drone * (stabs / drone).powf(self.chord.morph()));
        self.space.set_echo_start(self.chord.echo_start());
        self.space
            .update_block(params, signals.tempo, self.chord.boost());
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        let mut pulse = self.pulse.next(now);
        let stab = self.chord.next(now);
        let parts = self.bed.next_parts(1.0);
        // Between stabs, the chord follows the drone's dip; the stab itself does not.
        let gain = stab.residual * pulse.drone + stab.dry;
        let bed = [
            parts.bass[0] + parts.chord[0] * gain,
            parts.bass[1] + parts.chord[1] * gain,
        ];
        let air = self
            .air
            .next()
            .map(|sample| sample * pulse.drone * self.air_level);
        // Only the stab goes into the delay.
        let send = 0.5 * (parts.chord[0] + parts.chord[1]) * stab.send;
        pulse.delay_send += self
            .send_filter
            .iter_mut()
            .fold(send, |send, filter| filter.highpass(send));
        self.space.process(bed, air, pulse)
    }

    /// Steps played so far: thumps, ticks and chord stabs.
    pub fn steps(&self) -> [u64; 3] {
        let [thumps, ticks, _] = self.pulse.counts();
        [thumps, ticks, self.chord.count()]
    }
}

#[cfg(test)]
mod tests {
    use crate::dsp::Svf;
    use crate::engine::Engine;
    use crate::harmony::tests::palettes;
    use crate::instruments::Preset;
    use nova_core::Controls;

    const RATE: f32 = 16_000.0;

    fn engine(controls: Controls) -> Engine {
        let preset = Preset::Fathom(super::Preset::DEFAULT);
        let mut engine = Engine::with_palettes(RATE, 3, palettes(), &preset);
        engine.set_controls(controls);
        engine
    }

    /// Levels in dB of the chord's range (300 Hz to 3 kHz) per 10 ms, after `settle`
    /// seconds, over `seconds`, calling `change` at the start of every second.
    fn levels(
        engine: &mut Engine,
        settle: f32,
        seconds: f32,
        mut change: impl FnMut(&mut Engine, usize),
    ) -> Vec<f32> {
        let mut high = [Svf::new(300.0, 0.541, RATE), Svf::new(300.0, 1.307, RATE)];
        let mut low = [Svf::new(3000.0, 0.541, RATE), Svf::new(3000.0, 1.307, RATE)];
        let window = (RATE * 0.01) as usize;
        let (mut levels, mut sum) = (Vec::new(), 0.0);
        let first = (settle * RATE) as usize;
        for frame in 0..first + (seconds * RATE) as usize {
            if frame >= first && (frame - first).is_multiple_of(RATE as usize) {
                change(engine, (frame - first) / RATE as usize);
            }
            let [left, right] = engine.next_frame();
            let band = low.iter_mut().fold(
                high.iter_mut()
                    .fold(0.5 * (left + right), |value, filter| filter.highpass(value)),
                |value, filter| filter.lowpass(value),
            );
            if frame >= first {
                sum += band * band;
                if (frame - first + 1).is_multiple_of(window) {
                    levels.push(10.0 * (sum / window as f32 + 1e-12).log10());
                    sum = 0.0;
                }
            }
        }
        levels
    }

    /// Mean level of `levels[range]` in dB.
    fn mean(levels: &[f32]) -> f32 {
        let power = levels
            .iter()
            .map(|level| 10.0_f32.powf(level / 10.0))
            .sum::<f32>();
        10.0 * (power / levels.len() as f32).log10()
    }

    #[test]
    fn palette_changes_glide_the_filter() {
        for heat in [0.0, 1.0] {
            let controls = Controls {
                form: 0.0,
                heat,
                volume: 1.0,
                ..Controls::default()
            };
            let mut engine = engine(controls);
            // A new palette every 6 s, once the previous crossfade is done.
            let levels = levels(&mut engine, 3.0, 24.0, |engine, second| {
                if second % 6 == 0 {
                    engine.set_controls(Controls {
                        palette: 1 + second / 6,
                        ..controls
                    });
                }
            });
            // The old chord fades out over seconds; its filter must not jump at the change.
            for change in [600, 1200, 1800] {
                let before = mean(&levels[change - 20..change]);
                let after = mean(&levels[change..change + 20]);
                assert!(
                    (after - before).abs() < 3.0,
                    "heat {heat}, at {} s: {before} dB, then {after} dB",
                    change / 100
                );
            }
        }
    }

    #[test]
    fn swells_stay_near_the_drone() {
        for heat in [0.0, 1.0] {
            let loudness = |form: f32| {
                let mut engine = engine(Controls {
                    form,
                    heat,
                    volume: 1.0,
                    ..Controls::default()
                });
                mean(&levels(&mut engine, 3.0, 12.0, |_, _| {}))
            };
            let (drone, swells) = (loudness(0.05), loudness(0.22));
            assert!(
                (swells - drone).abs() < 3.0,
                "heat {heat}: drone {drone} dB, swells {swells} dB"
            );
        }
    }
}
