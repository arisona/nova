//! Fathom: the bed's chord played as dub stabs (see Audio in the README).
//!
//! The chord of the bed, folded into a compact register, is played as off-beat stabs
//! through a resonant low-pass that opens and closes with every hit (see `fathom/chord.rs`).
//! Only the stab goes into the dub delay, so the echoes and a dark hall carry the groove;
//! a deep sub bass and a soft kick stay underneath. Without rhythm (Void near 0 or 1) it
//! sinks into a dark, filtered drone. Bed, air, kick, ticks and space are shared with Strata.

pub mod chord;

use crate::BLOCK;
use crate::dsp::{Rng, Svf, coefficient, fold, lerp, smooth};
use crate::harmony::Harmony;
use crate::instruments::drone::Drone;
use crate::instruments::{pulse, space};
use crate::params::Params;
use crate::score::{Pattern, Patterns, Score};
use crate::signals::{Change, Now, Signals};
use crate::voices::{Sub, bed, dub_delay};
use chord::Chord;

pub use crate::score::Throws;

/// Fathom's rhythm, from straight (cold) to syncopated (warm); while the bass is away, one
/// pattern warmer. Stabs on the off-beat eighths when cold (even 16ths, so swing never moves
/// them, and never the downbeat, which belongs to the kick): swells on the second off-beat of
/// every second bar from a rhythm of 0.15, which turn into stabs, then on the second off-beat of
/// every bar, growing to all four, the skank. Warmer, the third stab pushes into the fourth
/// beat, and then the second bar answers the first, with a stab right after the kick and one
/// leading into the next bar. The kick from a rhythm of 0.4, half time and then four to the floor,
/// never three, which would land on the stabs; warmer, a ghost kick before the bar, and at
/// the warmest a one drop, on the third beat only, until the kick fills in again. Ticks from
/// 0.5, growing into off-beat hats; when warm, a shaker of 16ths. A bass line from 0.45:
/// a sub on the off-beats between the kicks when cold, with octaves and fifths warmer, a dub
/// riff on the one when warm. The echoes are dotted eighths at home, a quarter and a dotted
/// quarter while the bass is away. The last bar of every eighth is a fill, and the last
/// phrase of every eighth a break.
pub const SCORE: Score = Score {
    kick: Patterns {
        from: 0.4,
        list: &[
            Pattern::new("9... 5... 7... 5..."),
            Pattern::new("9... 5... 7... 5..3"),
            Pattern::new("4... 3... 9... 5..3"),
        ],
    },
    tick: Patterns {
        from: 0.5,
        list: &[
            Pattern::new("5.72 4.81 5.72 4.91"),
            Pattern::new("1.93 2.83 1.95 2.74"),
        ],
    },
    stab: Patterns {
        from: 0.15,
        list: &[
            Pattern::new("..3. ..9. ..5. ..6. ..3. ..8. ..5. ..6."),
            Pattern::new("..3. ..9. ...5 ..6. ..3. ..8. ...5 ..6."),
            Pattern::new("..3. ..9. ...5 ..6. .3.. ..8. ..5. ...6"),
        ],
    },
    bass: Patterns {
        from: 0.45,
        list: &[
            Pattern::new("..9. ..6. ..8. ..5."),
            Pattern::with_notes(
                "..9. ..6. ..8. ..5. ..9. ..6. ..8. 5.4.",
                "..1. ..1. ..1. ..1. ..1. ..1. ..1. 8.5.",
            ),
            Pattern::with_notes(
                "9..4 ..6. 7... ..5. 9..4 ..6. 7.3. .5..",
                "1..1 ..8. 5... ..1. 1..1 ..8. 5.4. .1..",
            ),
        ],
    },
    // One stab a bar, so a groove of them does not tire: the strongest step of each bar,
    // with the echoes of two heads following it. 4 plays every off-beat, the skank.
    stabs: 1,
    basses: &[
        // Climbing: the fifth on the third off-beat, the octave and the seventh leading on.
        &[
            Pattern::with_notes("..9. ..6. ..8. ..5.", "..1. ..1. ..5. ..1."),
            Pattern::with_notes(
                "..9. ..6. ..8. ..5. ..9. ..6. ..8. .54.",
                "..1. ..1. ..5. ..1. ..1. ..1. ..5. .87.",
            ),
            Pattern::with_notes(
                "9... ..6. .7.. ..5. 9... ..6. .7.5 ..4.",
                "1... ..5. .8.. ..7. 1... ..3. .4.5 ..1.",
            ),
        ],
        // Spacious: two notes a bar, the fifth answering the root.
        &[
            Pattern::with_notes("..9. .... ..8. ....", "..1. .... ..5. ...."),
            Pattern::with_notes(
                "..9. .... ..8. ..5. ..9. .... ..8. ....",
                "..1. .... ..8. ..5. ..1. .... ..4. ....",
            ),
            Pattern::with_notes(
                "9... .... 7..5 .... 9... ..6. 7... ....",
                "1... .... 8..5 .... 1... ..7. 5... ....",
            ),
        ],
    ],
    gate: Patterns::NONE,
    bell: Patterns::NONE,
    echo: &[3.0, 4.0, 3.0, 6.0],
    ghost: 0.2,
    drop: 0.12,
    throws: Throws::SHORT,
    fills: 8,
    breaks: 8,
};

/// Everything that sets Fathom's character, as data.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub score: Score,
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
    /// Peak level of the bass line, and the level of the bed's bass while the line plays:
    /// the sustained bass gives way to it.
    pub line: [f32; 2],
    /// How long a note of the bass line holds before it fades, in 16ths, when cold and warm.
    pub line_hold: [f32; 2],
    /// Output level in dB, so the instruments meet at the same loudness (see the sketches'
    /// report).
    pub level: f32,
}

/// The register the chord is folded into, in Hz.
const REGISTER: [f32; 2] = [180.0, 720.0];

impl Preset {
    pub const DEFAULT: Self = Self {
        score: SCORE,
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
            // The chord plays the stabs; no gate.
            thump_level: 0.22,
            tick_level: 0.12,
            stab_level: 0.0,
            chord_gate: 0.0,
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
                // A second head two 16ths further: the echoes fall irregularly.
                second_head: [2.0, 0.6],
            },
            ..space::Settings::DEFAULT
        },
        air: 0.5,
        reverb: [1.55, 0.6],
        line: [0.16, 0.3],
        line_hold: [1.2, 2.2],
        level: 0.0,
    };

    /// This preset with other throws.
    pub const fn with_throws(mut self, throws: Throws) -> Self {
        self.score.throws = throws;
        self
    }
}

pub struct Fathom {
    sample_rate: f32,
    drone: Drone,
    chord: Chord,
    air_level: f32,
    reverb: [f32; 2],
    /// Fourth-order Butterworth high-pass on the send, so the delay stays out of the bass.
    send_filter: [Svf; 2],
    sub: Sub,
    line: [f32; 2],
    line_hold: [f32; 2],
    /// The root, folded into 40 to 80 Hz, the tempo and Heat, for the notes of the line.
    root: f32,
    tempo: f32,
    heat: f32,
    /// Seconds since the line's latest note, and how far the line has taken over from the
    /// bed's bass, from 0 to 1.
    since_line: f32,
    taken: f32,
}

impl Fathom {
    /// Starts from `harmony` until the conductor loads the first one.
    pub fn new(sample_rate: f32, preset: &Preset, harmony: &Harmony, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let register = preset.bed.register.unwrap_or(REGISTER);
        Self {
            sample_rate,
            drone: Drone::new(
                sample_rate,
                preset.bed,
                preset.pulse,
                preset.space,
                harmony,
                &mut rng,
            ),
            chord: Chord::new(
                sample_rate,
                preset.chord,
                register,
                harmony,
                Rng::new(rng.next_u64()),
            ),
            air_level: preset.air,
            reverb: preset.reverb,
            send_filter: [
                Svf::new(160.0, 0.541, sample_rate),
                Svf::new(160.0, 1.307, sample_rate),
            ],
            sub: Sub::new(sample_rate),
            line: preset.line,
            line_hold: preset.line_hold,
            root: fold(harmony.root(), 40.0, 80.0),
            tempo: 100.0,
            heat: 0.5,
            since_line: f32::INFINITY,
            taken: 0.0,
        }
    }

    pub fn update_block(&mut self, params: &Params, signals: &mut Signals) {
        match self.drone.follow(signals) {
            Some(Change::Load) => self.chord.set_harmony(&signals.harmony, true),
            Some(Change::Palette) => self.chord.set_harmony(&signals.harmony, false),
            _ => {}
        }
        self.root = fold(signals.harmony.root(), 40.0, 80.0);
        self.tempo = signals.tempo.max(1.0);
        self.heat = params.heat;
        // The line takes over from the bed's bass over a bar, and gives it back a bar after
        // its latest note.
        let block_seconds = BLOCK as f32 / self.sample_rate;
        let bar_seconds = 240.0 / self.tempo;
        self.since_line += block_seconds;
        let target = (self.since_line < bar_seconds) as u8 as f32;
        self.taken += (target - self.taken) * coefficient(bar_seconds, 1.0 / block_seconds);
        self.chord.update_block(params, signals);
        self.drone.bed.set_filter(Some(self.chord.filter()));
        for filter in &mut self.send_filter {
            filter.settle();
        }
        let [drone, stabs] = self.reverb;
        let space = &mut self.drone.space;
        space.set_reverb_level(drone * (stabs / drone).powf(self.chord.morph()));
        space.set_echo_start(self.chord.echo_start());
        self.drone
            .update_block(params, signals, 0.0, self.chord.boost());
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        let drone = &mut self.drone;
        let mut pulse = drone.pulse.next(now);
        let stab = self.chord.next(now);
        let parts = drone.bed.next_parts(1.0);
        if now.hits.bass > 0.0 {
            let warmth = smooth(self.heat);
            let hold = lerp(self.line_hold[0], self.line_hold[1], warmth) * 15.0 / self.tempo;
            let frequency = self.root * now.hits.note;
            self.sub.trigger(now.hits.bass, frequency, hold, self.heat);
            self.since_line = 0.0;
        }
        let [line, residual] = self.line;
        let low = self.sub.next() * line;
        let bass = lerp(1.0, residual, self.taken);
        // Between stabs, the chord follows the drone's dip; the stab itself does not.
        let gain = stab.residual * pulse.drone + stab.dry;
        let bed = [
            parts.bass[0] * bass + low + parts.chord[0] * gain,
            parts.bass[1] * bass + low + parts.chord[1] * gain,
        ];
        let air = drone
            .air
            .next()
            .map(|sample| sample * pulse.drone * self.air_level);
        // Only the stab goes into the delay.
        let send = 0.5 * (parts.chord[0] + parts.chord[1]) * stab.send;
        pulse.delay_send += self
            .send_filter
            .iter_mut()
            .fold(send, |send, filter| filter.highpass(send));
        drone.space.process(bed, air, pulse)
    }

    /// Steps played so far: thumps, ticks and chord stabs.
    pub fn steps(&self) -> [u64; 3] {
        let [thumps, ticks, _] = self.drone.pulse.counts();
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
        let mut engine = Engine::with_palettes(RATE, 3, palettes(), &[preset], 0);
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
                void: 0.0,
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
            let loudness = |void: f32| {
                let mut engine = engine(Controls {
                    void,
                    heat,
                    volume: 1.0,
                    ..Controls::default()
                });
                mean(&levels(&mut engine, 3.0, 12.0, |_, _| {}))
            };
            // Rhythms of about 0.05 and 0.22, on the way up.
            let (drone, swells) = (loudness(0.015), loudness(0.035));
            assert!(
                (swells - drone).abs() < 3.0,
                "heat {heat}: drone {drone} dB, swells {swells} dB"
            );
        }
    }
}
