//! Bayou: the chord on a guitar and a combo organ, between ambient electronica and the
//! rolling guitar and organ of sixties ballads (see Audio in the README).
//!
//! Plucked strings (`voices/pluck.rs`) play a pulse: fast strokes on one tone of the chord,
//! each picking its string again, with accents from the score's weights, and the tone moves
//! only now and then. Cold, a straight pulse with the accents on the beats; warmer, the
//! accents in 3 + 3 + 2 in each half bar, the tresillo; warm, a syncopated figure on which
//! the tone moves more; at the warmest the ballads' roll returns, up the chord and back
//! down, against the bar. Only the accents go into the dub delay, so the echoes play a
//! counter-rhythm. A bass string plays the score's bass line, the root on the bar and the
//! fifth answering. The organ (`voices/organ.rs`) holds the chord over the drone's bass
//! and plays the rhythm in its brightness: the score's stabs bring its upper footages in,
//! each for as many 16ths as the score writes, which Form stretches into long phrases at
//! the layers and cuts into short ones at the grain, while the chord sounds on, closing to
//! about half its brightness; sharp when cold, rounder when warm. As the rhythm grows, the
//! organ steps back behind the pulse and breathes from there. Its rotary speaker
//! turns over Form too: still at the layers, a chorale in the middle, a tremolo towards the
//! grain and, at the grain, faster than any cabinet turns, so its tremolo becomes a tone.
//! The rhythm (from Void) lets more strokes and more of the organ's short notes play, and as it
//! dissolves, the strings move into the dub delay. The ballads' gestures, the organ's swells
//! and vibrato and the amp's tremolo and breakup, come in only towards the warmest. A soft
//! kick and ticks join with a fuller rhythm. Visual activity plucks the strings harder, the
//! organ's keys and the strings' notes whose colors show in the light lean forward, and
//! sparse light lets the strings fill the echoes. Bed, air, pulse and space are the drone's.

use crate::dsp::{Curve, Rng, coefficient, fold, lerp, smooth, smoothstep};
use crate::harmony::{CROSSFADE_SECONDS, Harmony, MAX_TONES, Tone};
use crate::instruments::drone::Drone;
use crate::instruments::{pulse, space};
use crate::params::Params;
use crate::score::{Pattern, Patterns, Score, Throws};
use crate::signals::{BAR, Change, Now, PHRASE, Signals};
use crate::voices::bed::Voicing;
use crate::voices::{Organ, Pick, Pluck, bed, organ, pluck};

/// Bayou's rhythm. The strings from a rhythm of 0.15, growing from the beats to every 16th,
/// from straight (cold) to syncopated (warm), and the roll at the warmest; the organ's gate
/// from 0.05, from a trance gate on the grid to longer, pushed phrases, each of its notes
/// holding as many 16ths as written; the bass string from 0.1, the root on the bar, then the
/// fifth on the third beat and an octave leading to it; a soft half-time kick from 0.6 and
/// off-beat ticks from 0.5. The last bar of every eighth is a fill.
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
    stab: Patterns {
        from: 0.05,
        list: &[
            // A long note, then the trance gate on the eighths.
            Pattern::with_notes(
                "9... .... 6... .... 8.5. 7.5. 8.5. 7.5.",
                "9... .... 8... .... 1.1. 1.1. 2.1. 1.1.",
            ),
            // Two long notes, then short ones off the beat.
            Pattern::with_notes(
                "9... .... ..6. .... 8.5. .7.. 6..5 ..4.",
                "6... .... ..5. .... 1.1. .2.. 1..2 ..1.",
            ),
            // Long, breathing phrases, pushed over the bar.
            Pattern::with_notes(
                "9... .... .... ..6. 7... ..5. .... 6...",
                "9... .... .... ..3. 7... ..2. .... 4...",
            ),
        ],
    },
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
            // A straight pulse, the tone moving every second bar.
            Pattern::with_notes(
                "9353 7353 8353 7353 9353 7353 8353 7353 9353 7353 8353 7353 9353 7353 8353 7353",
                "4444 4444 4444 4444 4444 4444 4444 4444 5555 5555 5555 5555 5555 5555 5555 3333",
            ),
            // The accents in 3 + 3 + 2 in each half bar, the tresillo.
            Pattern::with_notes(
                "9347 3463 8347 3464 9347 3463 8347 3464",
                "4444 4444 4444 4444 4444 4444 4445 5555",
            ),
            // A syncopated pulse on which the tone moves more.
            Pattern::with_notes(
                "9.35 .7.3 5.8. 3.6. 9.35 .7.3 5.8. .76.",
                "4.44 .4.4 4.5. 5.5. 4.44 .4.4 6.5. .43.",
            ),
            // The ballads' roll of six 16ths, up the chord and back down, against the bar.
            Pattern::with_notes("947 856", "123 432"),
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
    /// The range the organ's chord is folded into, and the lowest octave of the strings: the
    /// chord's tones are folded into it, and higher degrees climb from there.
    pub organ_register: [f32; 2],
    pub string_register: [f32; 2],
    /// The octave the bass string's root is folded into.
    pub bass_register: [f32; 2],
    /// Levels of the organ, the strings and the bass string.
    pub levels: [f32; 3],
    /// How far the organ steps back behind the pulse as the rhythm grows, in dB.
    pub recede: f32,
    /// How long the organ's notes hold over Form, as a factor of what the score writes:
    /// tied into long phrases at the layers, as written in the middle, short at the grain.
    pub hold: Curve,
    /// The brightness gate's ramps in seconds, and how much of the upper footages it keeps
    /// when closed, when cold (sharp) and warm (rounder, closing faster as it goes): about
    /// half, so the organ only breathes in brightness and never goes away.
    pub ramp: [f32; 2],
    pub floor: [f32; 2],
    /// Speed of the organ's rotary speaker over Form, in turns per second.
    pub rotary: Curve,
    /// How much of the ballads' gestures plays over Heat: none in the middle, all at the
    /// warmest, where the roll returns. At full strength the organ swells by `swell` dB into
    /// every phrase start and back, and the amp has a tremolo of `tremolo` (from 0 to 1) on
    /// the eighths and breaks up with a drive of `drive` into a soft clip.
    pub ballads: Curve,
    pub swell: f32,
    pub tremolo: f32,
    pub drive: f32,
    /// Send of the accented strokes to the dub delay when cold and warm, and of all of them
    /// once the rhythm has dissolved.
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
        string_register: [110.0, 220.0],
        bass_register: [70.0, 140.0],
        levels: [0.8, 0.25, 0.3],
        recede: 6.0,
        hold: Curve::new(&[[0.0, 4.0], [0.5, 1.0], [1.0, 0.3]]),
        ramp: [0.006, 0.03],
        floor: [0.5, 0.6],
        rotary: Curve::new(&[[0.15, 0.0], [0.5, 1.2], [0.75, 6.7], [1.0, 18.0]]),
        ballads: Curve::new(&[[0.6, 0.0], [1.0, 1.0]]),
        swell: 3.0,
        tremolo: 0.35,
        drive: 2.2,
        send: [0.45, 0.55],
        dissolved_send: 0.5,
        reverb: [0.4, 0.3],
        drone: 0.3,
        strike: 0.25,
        voicing: 3.0,
        sparse: 0.25,
        level: 2.4,
    };
}

pub struct Bayou {
    sample_rate: f32,
    drone: Drone,
    organ: Organ,
    strings: Pluck,
    bass_string: Pluck,
    preset: Preset,
    /// The chord's tones in the lowest octave of the strings, from low to high, and how far
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
    /// Samples per 16th, how much of the ballads' gestures plays, and the accents' share of
    /// the dub delay, falling over half a 16th after each.
    sixteenth: f32,
    ballads: f32,
    /// The organ's gain as it steps back behind the pulse.
    organ_gain: f32,
    accent: f32,
    accent_decay: f32,
    /// The brightness gate: samples it holds open, how far it opens, how far open it is, and
    /// per block its depth (from the rhythm), what it keeps when closed and its ramp per
    /// sample.
    held: f32,
    opening: f32,
    openness: f32,
    depth: f32,
    floor: f32,
    ramp: f32,
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
            sample_rate,
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
            sixteenth: 0.125 * sample_rate,
            ballads: 0.0,
            organ_gain: 1.0,
            accent: 0.0,
            accent_decay: 0.0,
            held: 0.0,
            opening: 1.0,
            openness: 1.0,
            depth: 0.0,
            floor: 0.0,
            ramp: 1.0,
            phrase: 0.0,
            eighth: 0.0,
            plucks: 0,
        };
        bayou.tune(harmony, 0.05);
        bayou
    }

    /// Folds the chord into the strings' lowest octave and the organ's range, the root into
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
        self.count = fold_into(self.preset.string_register, &mut self.tones);
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
        let warmth = smooth(params.heat);
        self.sixteenth = 15.0 / signals.tempo.max(1.0) * self.sample_rate;
        self.ballads = self.preset.ballads.at(params.heat);
        let behind = self.preset.recede * smoothstep(0.2, 0.6, params.rhythm);
        self.organ_gain = 10.0_f32.powf(-behind / 20.0);
        self.accent_decay = (-1.0 / (0.5 * self.sixteenth)).exp();
        // The gate comes in with the rhythm and, like the pulse, rests at Flow 0.
        self.depth =
            smoothstep(0.0, 0.2, params.rhythm) * smoothstep(0.0, 0.15, params.flow_setting);
        self.floor = lerp(self.preset.floor[0], self.preset.floor[1], warmth);
        let ramp = lerp(self.preset.ramp[0], self.preset.ramp[1], warmth);
        self.ramp = coefficient(ramp, self.sample_rate);
        let hues = &params.light.hues;
        let voicing = Voicing::new(&self.tones[..self.count], hues, self.preset.voicing);
        for (lean, tone) in self.leans.iter_mut().zip(&self.tones[..self.count]) {
            *lean = voicing.gain(tone);
        }
        let voicing = Voicing::new(&self.chord[..self.chord_count], hues, self.preset.voicing);
        for tone in &self.chord[..self.chord_count] {
            self.organ.lean(tone.frequency, voicing.gain(tone));
        }
        self.organ.update_block(
            params.heat,
            self.preset.rotary.at(params.form),
            crate::BLOCK,
        );
        self.strings.settle();
        self.bass_string.settle();
        self.drone.update_block(params, signals, 0.0, 0.0);
    }

    #[inline]
    pub fn next(&mut self, now: &Now) -> [f32; 2] {
        if let Some(step) = now.step {
            let hits = &now.hits;
            let [low, high] = self.preset.string_register;
            let strike = 1.0 + self.preset.strike * (2.0 * self.liveliness - 1.0);
            if hits.melody > 0.0 {
                // Strokes of weight 7 and more are accents.
                self.accent = self.accent.max(smoothstep(0.8, 0.96, hits.melody));
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
            if hits.stab > 0.0 {
                // A note of the gate, which never cuts short one still holding.
                let hold = hits.hold.max(1) as f32 * self.preset.hold.at(self.form);
                self.held = self.held.max(hold * self.sixteenth);
                self.opening = hits.stab;
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
        // The brightness gate, closing faster as it goes when warm.
        let warmth = smooth(self.heat);
        let target = if self.held > 0.0 {
            self.held -= 1.0;
            self.opening
        } else {
            0.0
        };
        self.openness += (target - self.openness) * self.ramp;
        let open = self.openness;
        let bright = open * lerp(1.0, open, warmth);
        let brightness = 1.0 - self.depth * (1.0 - lerp(self.floor, 1.0, bright));
        // Towards the warmest, the organ swells over the last bar into the phrase start and
        // back over the first, around its level.
        let bump = if phrase > 0.75 {
            smooth((phrase - 0.75) / 0.25)
        } else {
            1.0 - smooth(phrase / 0.25)
        };
        let swell = 10.0_f32.powf(self.preset.swell * self.ballads * (bump - 0.5) / 20.0);
        let [organ_level, string_level, bass_level] = self.preset.levels;
        let organ = self
            .organ
            .next(brightness)
            .map(|sample| sample * organ_level * self.organ_gain * swell);
        // Towards the warmest, the amp: a tremolo on the eighths and a soft breakup.
        let tremolo = 1.0
            - self.preset.tremolo
                * self.ballads
                * (0.5 - 0.5 * (std::f32::consts::TAU * eighth).cos());
        let drive = lerp(1.0, self.preset.drive, self.ballads);
        let pulse_strings = self.strings.next();
        let bass = self.bass_string.next();
        let strings: [f32; 2] = std::array::from_fn(|channel| {
            let clean = pulse_strings[channel] * string_level + bass[channel] * bass_level;
            (drive * clean).tanh() / drive * tremolo
        });
        // The accents go into the dub delay, and as the rhythm dissolves, every stroke.
        let dry = 1.0 - 0.5 * self.dissolve;
        let send = lerp(self.preset.send[0], self.preset.send[1], warmth) * self.accent
            + self.preset.dissolved_send * self.dissolve
            + self.preset.sparse * self.sparseness;
        self.accent *= self.accent_decay;
        let [string_reverb, organ_reverb] = self.preset.reverb;
        for channel in 0..2 {
            pulse.dry[channel] += strings[channel] * dry + organ[channel];
            pulse.reverb_send[channel] +=
                string_reverb * strings[channel] + organ_reverb * organ[channel];
        }
        pulse.delay_send += send * 0.5 * (strings[0] + strings[1]);
        drone.space.process(bed, air, pulse)
    }

    /// Steps played so far: thumps, ticks and strokes of the strings.
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
        let [low, high] = Preset::DEFAULT.string_register;
        assert!(bayou.count >= 2 && bayou.chord_count >= 2);
        let tones: Vec<f32> = (1..=8).map(|degree| bayou.degree(degree).0).collect();
        assert!(tones.windows(2).all(|pair| pair[1] > pair[0]), "{tones:?}");
        assert!(tones[0] >= low && tones[0] < high);
    }

    /// How far the gate is open with stabs on every eighth, each holding a 16th as written,
    /// at `form`: the lowest at the end of a 16th, and the highest.
    fn gate_range(form: f32) -> (f32, f32) {
        let harmony = Harmony::from_palette(&palette());
        let mut bayou = Bayou::new(16_000.0, &Preset::DEFAULT, &harmony, 1);
        let mut signals = Signals::new(harmony);
        signals.tempo = 120.0;
        let params = Params {
            form,
            rhythm: 0.8,
            ..Params::default()
        };
        bayou.update_block(&params, &mut signals);
        let sixteenth = bayou.sixteenth as usize;
        let (mut lowest, mut highest) = (1.0_f32, 0.0_f32);
        for step in 0..32 {
            let mut now = Now {
                step: Some(step),
                ..Now::default()
            };
            if step % 2 == 0 {
                now.hits.stab = 1.0;
                now.hits.hold = 1;
            }
            for sample in 0..sixteenth {
                bayou.next(&now);
                now.step = None;
                if step >= 4 {
                    highest = highest.max(bayou.openness);
                    if sample == sixteenth - 1 {
                        lowest = lowest.min(bayou.openness);
                    }
                }
            }
        }
        (lowest, highest)
    }

    #[test]
    fn form_ties_the_gate_into_phrases_or_cuts_it_short() {
        let (tied, _) = gate_range(0.0);
        let (cut, open) = gate_range(1.0);
        assert!(tied > 0.9, "{tied}");
        assert!(cut < 0.1 && open > 0.8, "{cut}, {open}");
    }
}
