//! Bayou: the chord on a combo organ and a guitar, after the organ and the rolling guitar
//! of sixties ballads (see Audio in the README).
//!
//! The organ (`voices/organ.rs`) holds the palette's chord over the drone's bass and swells
//! into every phrase start. Plucked strings (`voices/pluck.rs`) roll through the chord's
//! degrees as the score writes them: cold, a roll of six 16ths against the bar; warmer,
//! 3 + 3 + 2 in each half bar, the tresillo; warm, a syncopated figure over two bars. A bass
//! string plays the score's bass line, the root on the bar and the fifth answering. The
//! strings go through an amp, with a tremolo on the eighths and a little breakup when warm.
//! The rhythm (from Void) lets the roll grow from one note per roll to all of it, and as the
//! rhythm dissolves, the strings move into the dub delay. A soft kick and ticks join with a
//! fuller rhythm. Visual activity plucks the strings harder, the organ's keys and the roll's
//! notes whose colors show in the light lean forward, and sparse light lets the strings fill
//! the echoes. Bed, air, pulse and space are the drone's.

use crate::dsp::{Rng, fold, lerp, smooth};
use crate::harmony::{CROSSFADE_SECONDS, Harmony, MAX_TONES, Tone};
use crate::instruments::drone::Drone;
use crate::instruments::{pulse, space};
use crate::params::Params;
use crate::score::{Pattern, Patterns, Score, Throws};
use crate::signals::{BAR, Change, Now, PHRASE, Signals};
use crate::voices::bed::Voicing;
use crate::voices::{Organ, Pick, Pluck, bed, organ, pluck};

/// Bayou's rhythm. The roll from a rhythm of 0.15, from straight (cold) to syncopated
/// (warm); the bass string from 0.1, the root on the bar, then the fifth on the third beat
/// and an octave leading to it; a soft half-time kick from 0.6 and off-beat ticks from 0.5.
/// The last bar of every eighth is a fill.
pub const SCORE: Score = Score {
    kick: Patterns {
        from: 0.6,
        list: &[
            Pattern::new("9... .... 6... ...."),
            Pattern::new("9... .... 6... ..3."),
        ],
    },
    tick: Patterns {
        from: 0.5,
        list: &[
            Pattern::new("..9. ..6. ..8. ..5."),
            Pattern::new("..9. .36. ..8. .35."),
        ],
    },
    stab: Patterns::NONE,
    stabs: BAR,
    bass: Patterns {
        from: 0.1,
        list: &[
            Pattern::with_notes("9... .... .... ....", "1... .... .... ...."),
            Pattern::with_notes("9... .... 7... ....", "1... .... 5... ...."),
            Pattern::with_notes("9... ..4. 7... ....", "1... ..8. 5... ...."),
        ],
    },
    basses: &[],
    gate: Patterns::NONE,
    melody: Patterns {
        from: 0.15,
        list: &[
            // A roll of six 16ths, up the chord and back down, against the bar.
            Pattern::with_notes("947 856", "123 432"),
            // 3 + 3 + 2 in each half bar, the tresillo.
            Pattern::with_notes("947 857 63 947 857 64", "123 432 31 124 543 42"),
            // A syncopated figure, the second bar answering the first.
            Pattern::with_notes(
                "9.6. 48.5 .7.3 6.4. 9.5. 38.6 .7.4 5..6",
                "1.3. 24.3 .5.2 3.1. 1.4. 35.4 .3.2 4..2",
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

/// Everything that sets Bayou's character, as data.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub score: Score,
    pub bed: bed::Settings,
    /// Kick and ticks, with the drone's dip and the bass accent.
    pub pulse: pulse::Settings,
    pub space: space::Settings,
    pub organ: organ::Settings,
    pub strings: pluck::Settings,
    /// The range the organ's chord is folded into, and the lowest octave of the roll: the
    /// chord's tones are folded into it, and higher degrees climb from there.
    pub organ_register: [f32; 2],
    pub roll_register: [f32; 2],
    /// The octave the bass string's root is folded into.
    pub bass_register: [f32; 2],
    /// Levels of the organ, the roll and the bass string.
    pub levels: [f32; 3],
    /// How far the organ swells into every phrase start and back, in dB.
    pub swell: f32,
    /// Depth of the amp's tremolo when warm, from 0 to 1, and its breakup when cold and
    /// warm, as the drive into a soft clip.
    pub tremolo: f32,
    pub drive: [f32; 2],
    /// Send of the strings to the dub delay when cold and warm, and how much more once the
    /// rhythm has dissolved.
    pub send: [f32; 2],
    pub dissolved_send: f32,
    /// Sends of the strings and the organ to the reverb.
    pub reverb: [f32; 2],
    /// Level of the bed's chord under the organ.
    pub drone: f32,
    /// How much harder the strings are plucked with visual activity: their velocity changes
    /// by up to this share either way of calm and busy light; 0 ignores the activity.
    pub strike: f32,
    /// How far the colors of the light move the organ's keys and the roll's notes, in dB,
    /// as in the bed (see `bed::Settings::voicing`); 0 ignores the colors.
    pub voicing: f32,
    /// How much more of the strings goes into the dub delay as the light thins out from
    /// half lit to dark (see `Features::sparseness`); 0 ignores the density.
    pub sparse: f32,
    /// Output level in dB, so the instruments meet at the same loudness (see the sketches'
    /// report).
    pub level: f32,
}

impl Preset {
    pub const DEFAULT: Self = Self {
        score: SCORE,
        bed: bed::Settings::DEFAULT,
        pulse: pulse::Settings {
            thump_level: 0.14,
            tick_level: 0.07,
            stab_level: 0.0,
            chord_gate: 0.0,
            ..pulse::Settings::DEFAULT
        },
        space: space::Settings::DEFAULT,
        organ: organ::Settings::COMBO,
        strings: pluck::Settings::GUITAR,
        organ_register: [180.0, 720.0],
        roll_register: [110.0, 220.0],
        bass_register: [70.0, 140.0],
        levels: [0.8, 0.25, 0.3],
        swell: 3.0,
        tremolo: 0.35,
        drive: [1.0, 2.2],
        send: [0.2, 0.3],
        dissolved_send: 0.5,
        reverb: [0.4, 0.3],
        drone: 0.3,
        strike: 0.25,
        voicing: 3.0,
        sparse: 0.25,
        level: 1.2,
    };
}

pub struct Bayou {
    drone: Drone,
    organ: Organ,
    strings: Pluck,
    bass_string: Pluck,
    preset: Preset,
    /// The chord's tones in the lowest octave of the roll, from low to high, and how far
    /// the colors of the light lean each.
    tones: [Tone; MAX_TONES],
    leans: [f32; MAX_TONES],
    count: usize,
    /// The chord for the organ, and the root for the bass string.
    chord: [Tone; MAX_TONES],
    chord_count: usize,
    root: f32,
    heat: f32,
    form: f32,
    dissolve: f32,
    liveliness: f32,
    sparseness: f32,
    /// Where in the phrase the music is, from 0 to 1, and in the current eighth.
    phrase: f32,
    eighth: f32,
    plucks: u64,
}

impl Bayou {
    /// Starts from `harmony` until the conductor loads the first one.
    pub fn new(sample_rate: f32, preset: &Preset, harmony: &Harmony, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut bayou = Self {
            drone: Drone::new(
                sample_rate,
                preset.bed,
                preset.pulse,
                preset.space,
                harmony,
                &mut rng,
            ),
            organ: Organ::new(sample_rate, preset.organ),
            strings: Pluck::new(sample_rate, preset.strings, Rng::new(rng.next_u64())),
            bass_string: Pluck::new(sample_rate, preset.strings, Rng::new(rng.next_u64())),
            preset: *preset,
            tones: [Tone::default(); MAX_TONES],
            leans: [1.0; MAX_TONES],
            count: 0,
            chord: [Tone::default(); MAX_TONES],
            chord_count: 0,
            root: 0.0,
            heat: 0.5,
            form: 0.5,
            dissolve: 0.0,
            liveliness: 0.5,
            sparseness: 0.0,
            phrase: 0.0,
            eighth: 0.0,
            plucks: 0,
        };
        bayou.tune(harmony, 0.05);
        bayou
    }

    /// Folds the chord into the roll's lowest octave and the organ's range, the root into
    /// the bass string's octave, and plays the organ's chord over `seconds`.
    fn tune(&mut self, harmony: &Harmony, seconds: f32) {
        let fold_into = |register: [f32; 2], tones: &mut [Tone; MAX_TONES]| {
            let mut count = 0;
            for tone in harmony.tones() {
                let frequency = fold(tone.frequency, register[0], register[1]);
                let taken = tones[..count]
                    .iter()
                    .any(|other| (frequency / other.frequency - 1.0).abs() < 0.01);
                if !taken {
                    tones[count] = Tone { frequency, ..*tone };
                    count += 1;
                }
            }
            tones[..count].sort_by(|a, b| a.frequency.total_cmp(&b.frequency));
            count
        };
        self.count = fold_into(self.preset.roll_register, &mut self.tones);
        self.chord_count = fold_into(self.preset.organ_register, &mut self.chord);
        let [low, high] = self.preset.bass_register;
        self.root = fold(harmony.root(), low, high);
        self.organ.play(&self.chord[..self.chord_count], seconds);
    }

    /// The frequency of a degree of the chord, from 1, and how far the light leans it: the
    /// tones climb by octaves.
    fn degree(&self, degree: u8) -> (f32, f32) {
        let index = degree.max(1) as usize - 1;
        let count = self.count.max(1);
        let frequency = self.tones[index % count].frequency * (1 << (index / count)) as f32;
        (frequency, self.leans[index % count])
    }

    pub fn update_block(&mut self, params: &Params, signals: &mut Signals) {
        match self.drone.follow(signals) {
            Some(Change::Load) => self.tune(&signals.harmony, 0.05),
            Some(Change::Palette) => self.tune(&signals.harmony, 0.5 * CROSSFADE_SECONDS),
            Some(Change::Bass) => self.tune(&signals.harmony, 0.3),
            _ => {}
        }
        self.heat = params.heat;
        self.form = params.form;
        self.dissolve = params.dissolve;
        self.liveliness = params.liveliness;
        self.sparseness = params.light.sparseness();
        let hues = &params.light.hues;
        let voicing = Voicing::new(&self.tones[..self.count], hues, self.preset.voicing);
        for (lean, tone) in self.leans.iter_mut().zip(&self.tones[..self.count]) {
            *lean = voicing.gain(tone);
        }
        let voicing = Voicing::new(&self.chord[..self.chord_count], hues, self.preset.voicing);
        for tone in &self.chord[..self.chord_count] {
            self.organ.lean(tone.frequency, voicing.gain(tone));
        }
        self.organ.update_block(params.heat, crate::BLOCK);
        self.strings.settle();
        self.bass_string.settle();
        self.drone.update_block(params, signals, 0.0, 0.0);
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        if let Some(step) = now.step {
            let hits = &now.hits;
            let [low, high] = self.preset.roll_register;
            let strike = 1.0 + self.preset.strike * (2.0 * self.liveliness - 1.0);
            if hits.melody > 0.0 {
                let (frequency, lean) = self.degree(hits.degree);
                // Low strings a little to the left, high ones to the right.
                let pan = ((frequency / (low * high).sqrt()).log2() * 0.35).clamp(-0.5, 0.5);
                self.strings.trigger(Pick {
                    velocity: hits.melody * strike * lean,
                    frequency,
                    pan,
                    heat: self.heat,
                    form: self.form,
                });
                self.plucks += 1;
            }
            if hits.bass > 0.0 {
                self.bass_string.trigger(Pick {
                    velocity: hits.bass * strike,
                    frequency: self.root * hits.note,
                    pan: 0.0,
                    heat: self.heat,
                    form: self.form,
                });
            }
            let steps = (BAR * PHRASE) as u64;
            self.phrase = (step % steps) as f32 / steps as f32;
            self.eighth = (step % 2) as f32 * 0.5;
        }
        let phrase = self.phrase + now.progress / (BAR * PHRASE) as f32;
        let eighth = self.eighth + 0.5 * now.progress;
        let drone = &mut self.drone;
        let mut pulse = drone.pulse.next(now);
        let parts = drone.bed.next_parts(1.0);
        let chord = self.preset.drone * pulse.drone;
        let bed = [
            parts.bass[0] + parts.chord[0] * chord,
            parts.bass[1] + parts.chord[1] * chord,
        ];
        let air = drone.air.next().map(|sample| sample * pulse.drone);
        // The organ swells over the last bar into the phrase start and back over the first,
        // around its level.
        let bump = if phrase > 0.75 {
            smooth((phrase - 0.75) / 0.25)
        } else {
            1.0 - smooth(phrase / 0.25)
        };
        let swell = 10.0_f32.powf(self.preset.swell * (bump - 0.5) / 20.0);
        let [organ_level, roll_level, bass_level] = self.preset.levels;
        let organ = self.organ.next().map(|sample| sample * organ_level * swell);
        // The amp: a tremolo on the eighths and a soft breakup, when warm.
        let warmth = smooth(self.heat);
        let tremolo = 1.0
            - self.preset.tremolo * warmth * (0.5 - 0.5 * (std::f32::consts::TAU * eighth).cos());
        let drive = lerp(self.preset.drive[0], self.preset.drive[1], warmth);
        let roll = self.strings.next();
        let bass = self.bass_string.next();
        let strings: [f32; 2] = std::array::from_fn(|channel| {
            let clean = roll[channel] * roll_level + bass[channel] * bass_level;
            (drive * clean).tanh() / drive * tremolo
        });
        // As the rhythm dissolves, the strings move into their echoes.
        let dry = 1.0 - 0.5 * self.dissolve;
        let send = lerp(self.preset.send[0], self.preset.send[1], warmth)
            + self.preset.dissolved_send * self.dissolve
            + self.preset.sparse * self.sparseness;
        let [string_reverb, organ_reverb] = self.preset.reverb;
        for channel in 0..2 {
            pulse.dry[channel] += strings[channel] * dry + organ[channel];
            pulse.reverb_send[channel] +=
                string_reverb * strings[channel] + organ_reverb * organ[channel];
        }
        pulse.delay_send += send * 0.5 * (strings[0] + strings[1]);
        drone.space.process(bed, air, pulse)
    }

    /// Steps played so far: thumps, ticks and plucks of the roll.
    pub fn steps(&self) -> [u64; 3] {
        let [thumps, ticks, _] = self.drone.pulse.counts();
        [thumps, ticks, self.plucks]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::tests::palette;

    #[test]
    fn degrees_climb_the_chord_by_octaves() {
        let harmony = Harmony::from_palette(&palette());
        let bayou = Bayou::new(16_000.0, &Preset::DEFAULT, &harmony, 1);
        let [low, high] = Preset::DEFAULT.roll_register;
        assert!(bayou.count >= 2 && bayou.chord_count >= 2);
        let tones: Vec<f32> = (1..=8).map(|degree| bayou.degree(degree).0).collect();
        assert!(tones.windows(2).all(|pair| pair[1] > pair[0]), "{tones:?}");
        assert!(tones[0] >= low && tones[0] < high);
    }
}
