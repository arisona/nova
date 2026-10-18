//! Shoal: the chord played by bells, as a gamelan would (see `doc/audio_concept.md`).
//!
//! Struck bars in beating pairs (see `voices/bell.rs`) play the palette's chord, its degrees
//! written in the score. Cold, they run an arpeggio of 12 steps against the bar; warmer,
//! the even and odd 16ths interlock from the left and the right like a gamelan's paired
//! parts (kotekan); warm, a syncopated figure over two bars. The rhythm (from Void) doubles
//! the elaboration, from one note per beat to eighths and 16ths, as a gamelan does between
//! its levels of density. A gong closes every phrase, and its fifth marks the half. The
//! bed's drone sinks below the bells as they grow; with a fuller rhythm a soft kick and
//! ticks join. Bed, air,
//! pulse and space are shared with Strata.

use crate::dsp::{Rng, lerp, smooth, smoothstep};
use crate::harmony::{Harmony, MAX_TONES};
use crate::instruments::strata::pulse::{self, Pulse};
use crate::instruments::strata::space::{self, Space};
use crate::params::Params;
use crate::score::{Pattern, Patterns, Score, Throws};
use crate::signals::{Change, Now, Signals};
use crate::voices::{Air, Bed, Bell, Strike, bed, bell};

/// Shoal's rhythm. Bells from a rhythm of 0.2, from straight (cold) to syncopated (warm); a gong
/// from 0.1 on every phrase, its fifth at the half from 0.4; a soft half-time kick from 0.6
/// and off-beat ticks from 0.55. The last bar of every eighth is a fill.
pub const SCORE: Score = Score {
    kick: Patterns {
        from: 0.6,
        list: &[
            Pattern::new("9... .... 6... ...."),
            Pattern::new("9... .... 6... ..3."),
        ],
    },
    tick: Patterns {
        from: 0.55,
        list: &[
            Pattern::new("..9. ..6. ..8. ..5."),
            Pattern::new("..9. .36. ..8. .35."),
        ],
    },
    stab: Patterns::NONE,
    bass: Patterns {
        from: 0.1,
        list: &[Pattern::with_notes(
            "9... .... .... .... .... .... .... .... 6... .... .... .... .... .... .... ....",
            "1... .... .... .... .... .... .... .... 5... .... .... .... .... .... .... ....",
        )],
    },
    gate: Patterns::NONE,
    bell: Patterns {
        from: 0.2,
        list: &[
            // An arpeggio rising over three beats, against the bar.
            Pattern::with_notes("9353 7353 8353", "1234 2345 3456"),
            // Interlocking parts circling the chord.
            Pattern::with_notes("9464 8464 9464 8464", "1213 3234 2123 4345"),
            // A syncopated figure, the second bar answering the first.
            Pattern::with_notes(
                "4.95 3.8. 4.96 3.7. 5.9. 3.85 4.9. 6..7",
                "1.32 1.4. 2.53 1.4. 3.54 2.43 1.3. 5..2",
            ),
        ],
    },
    echo: &[3.0, 4.0],
    ghost: 0.15,
    drop: 0.1,
    throws: Throws::NONE,
    fills: 8,
    breaks: 0,
};

/// Everything that sets Shoal's character, as data.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub score: Score,
    pub bed: bed::Settings,
    /// Kick and ticks, with the drone's dip and the bass accent.
    pub pulse: pulse::Settings,
    pub space: space::Settings,
    pub bell: bell::Settings,
    pub gong: bell::Settings,
    /// The lowest octave of the bells: the chord's tones are folded into it, and higher
    /// degrees climb from there.
    pub register: [f32; 2],
    /// Levels of the bells and the gong.
    pub levels: [f32; 2],
    /// Send of the bells to the dub delay when cold and warm, and to the reverb.
    pub send: [f32; 2],
    pub reverb: f32,
    /// Level of the drone's chord below the bells and once they play fully.
    pub drone: [f32; 2],
}

impl Preset {
    pub const DEFAULT: Self = Self {
        score: SCORE,
        bed: bed::Settings::DEFAULT,
        pulse: pulse::Settings {
            thump_level: 0.16,
            tick_level: 0.08,
            stab_level: 0.0,
            chord_gate: 0.0,
            ..pulse::Settings::DEFAULT
        },
        space: space::Settings::DEFAULT,
        bell: bell::Settings::BAR,
        gong: bell::Settings::GONG,
        register: [300.0, 600.0],
        levels: [0.08, 0.16],
        send: [0.3, 0.45],
        reverb: 0.35,
        drone: [1.0, 0.45],
    };
}

pub struct Shoal {
    bed: Bed,
    air: Air,
    pulse: Pulse,
    space: Space,
    bells: Bell,
    gong: Bell,
    preset: Preset,
    /// The chord's tones in the lowest octave of the bells, from low to high.
    tones: [f32; MAX_TONES],
    count: usize,
    root: f32,
    heat: f32,
    form: f32,
    drone: f32,
    rings: u64,
}

impl Shoal {
    /// Starts from `harmony` until the conductor loads the first one.
    pub fn new(sample_rate: f32, preset: &Preset, harmony: &Harmony, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut shoal = Self {
            bed: Bed::new(sample_rate, preset.bed, harmony, Rng::new(rng.next_u64())),
            air: Air::new(sample_rate, harmony, Rng::new(rng.next_u64())),
            pulse: Pulse::new(sample_rate, preset.pulse, harmony, Rng::new(rng.next_u64())),
            space: Space::new(sample_rate, preset.space),
            bells: Bell::new(sample_rate, preset.bell),
            gong: Bell::new(sample_rate, preset.gong),
            preset: *preset,
            tones: [0.0; MAX_TONES],
            count: 0,
            root: 0.0,
            heat: 0.5,
            form: 0.5,
            drone: 1.0,
            rings: 0,
        };
        shoal.tune(harmony);
        shoal
    }

    /// Folds the chord into the bells' lowest octave, and the root into the gong's.
    fn tune(&mut self, harmony: &Harmony) {
        let [low, high] = self.preset.register;
        let fold = |mut frequency: f32, low: f32, high: f32| {
            while frequency < low {
                frequency *= 2.0;
            }
            while frequency >= high {
                frequency *= 0.5;
            }
            frequency
        };
        self.count = 0;
        for tone in harmony.tones() {
            let frequency = fold(tone.frequency, low, high);
            let taken = self.tones[..self.count]
                .iter()
                .any(|&other| (frequency / other - 1.0).abs() < 0.01);
            if !taken {
                self.tones[self.count] = frequency;
                self.count += 1;
            }
        }
        self.tones[..self.count].sort_by(f32::total_cmp);
        self.root = fold(harmony.root(), 65.0, 130.0);
    }

    /// The frequency of a degree of the chord, from 1: the tones climb by octaves.
    fn degree(&self, degree: u8) -> f32 {
        let index = degree.max(1) as usize - 1;
        let count = self.count.max(1);
        self.tones[index % count] * (1 << (index / count)) as f32
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
                self.tune(harmony);
            }
            Some(Change::Palette) => {
                self.bed.change(harmony);
                self.air.change(harmony);
                self.pulse.change(harmony);
                self.tune(harmony);
            }
            Some(Change::Bass) => {
                self.bed.set_bass(harmony.root());
                self.air.change(harmony);
                self.pulse.change(harmony);
                self.tune(harmony);
            }
            Some(Change::Voice) => self.bed.move_voice(),
            None => {}
        }
        self.heat = params.heat;
        self.form = params.form;
        let [below, under] = self.preset.drone;
        self.drone = lerp(below, under, smoothstep(0.2, 0.6, params.rhythm));
        self.bells.normalize();
        self.gong.normalize();
        self.bed.update_block(params, signals);
        self.air.update_block(params, 0.0);
        self.pulse.update_block(params, signals);
        self.space
            .update_block(params, signals.tempo, signals.echo, 0.0);
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        if let Some(step) = now.step {
            let hits = &now.hits;
            if hits.bell > 0.0 {
                // Even and odd 16ths interlock from either side.
                let pan = if step.is_multiple_of(2) { -0.45 } else { 0.45 };
                self.bells.trigger(Strike {
                    velocity: hits.bell,
                    frequency: self.degree(hits.degree),
                    pan,
                    heat: self.heat,
                    form: self.form,
                });
                self.rings += 1;
            }
            if hits.bass > 0.0 {
                self.gong.trigger(Strike {
                    velocity: hits.bass,
                    frequency: self.root * hits.note,
                    pan: 0.0,
                    heat: self.heat,
                    form: self.form,
                });
            }
        }
        let mut pulse = self.pulse.next(now);
        let parts = self.bed.next_parts(1.0);
        let chord = self.drone * pulse.drone;
        let bed = [
            parts.bass[0] + parts.chord[0] * chord,
            parts.bass[1] + parts.chord[1] * chord,
        ];
        let air = self.air.next().map(|sample| sample * pulse.drone);
        let [bells_level, gong_level] = self.preset.levels;
        let bells = self.bells.next().map(|sample| sample * bells_level);
        let gong = self.gong.next().map(|sample| sample * gong_level);
        let warmth = smooth(self.heat);
        let send = lerp(self.preset.send[0], self.preset.send[1], warmth);
        for channel in 0..2 {
            pulse.dry[channel] += bells[channel] + gong[channel];
            pulse.reverb_send[channel] += self.preset.reverb * bells[channel] + 0.5 * gong[channel];
        }
        pulse.delay_send += send * 0.5 * (bells[0] + bells[1]);
        self.space.process(bed, air, pulse)
    }

    /// Steps played so far: thumps, ticks and bells.
    pub fn steps(&self) -> [u64; 3] {
        let [thumps, ticks, _] = self.pulse.counts();
        [thumps, ticks, self.rings]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::tests::palette;

    #[test]
    fn degrees_climb_the_chord_by_octaves() {
        let harmony = Harmony::from_palette(&palette());
        let shoal = Shoal::new(16_000.0, &Preset::DEFAULT, &harmony, 1);
        let [low, high] = Preset::DEFAULT.register;
        assert!(shoal.count >= 2);
        let tones: Vec<f32> = (1..=9).map(|degree| shoal.degree(degree)).collect();
        assert!(tones.windows(2).all(|pair| pair[1] > pair[0]), "{tones:?}");
        assert!(tones[0] >= low && tones[0] < high);
        assert_eq!(shoal.degree(1 + shoal.count as u8), 2.0 * tones[0]);
    }
}
