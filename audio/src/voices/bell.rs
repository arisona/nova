//! The bell: struck bars by modal synthesis, in pairs that beat like a gamelan's.
//!
//! Every note is a few inharmonic modes, the upper ones dying away faster, each with a twin
//! detuned by a few Hz, as a gamelan tunes its paired instruments apart so they shimmer
//! (ombak). Cold is glass: bright, clear modes and a shorter ring; warm is bronze: darker,
//! longer and wider beating. Soft notes are darker than hard ones. With other settings the
//! same voice is a gong: low, slow to speak and long. Notes ring on over each other; a note
//! that has to make room fades out over 3 ms.

use crate::dsp::{Rotor, lerp, smooth};

/// Modes per note, and notes ringing at once.
const MODES: usize = 4;
const NOTES: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Frequencies of the modes relative to the first.
    pub ratios: [f32; MODES],
    /// Levels of the modes when cold and warm.
    pub cold: [f32; MODES],
    pub warm: [f32; MODES],
    /// Decay time constant of the first mode in seconds, when cold and warm; mode k decays
    /// `1 + damping · k` times faster.
    pub decay: [f32; 2],
    pub damping: f32,
    /// How far each twin is detuned, in Hz, when cold and warm.
    pub beat: [f32; 2],
    /// Seconds the note takes to speak.
    pub attack: f32,
}

impl Settings {
    /// A bar of a metallophone, between glass and bronze.
    pub const BAR: Self = Self {
        ratios: [1.0, 2.76, 5.4, 8.93],
        cold: [1.0, 0.45, 0.22, 0.1],
        warm: [1.0, 0.18, 0.05, 0.015],
        decay: [0.7, 1.4],
        damping: 1.6,
        beat: [1.8, 3.2],
        attack: 0.0015,
    };

    /// A gong: nearly harmonic, slow to speak, ringing for seconds, beating slowly.
    pub const GONG: Self = Self {
        ratios: [1.0, 2.0, 2.41, 3.01],
        cold: [1.0, 0.3, 0.12, 0.08],
        warm: [1.0, 0.45, 0.2, 0.1],
        decay: [3.0, 4.5],
        damping: 0.8,
        beat: [0.8, 1.4],
        attack: 0.025,
    };
}

#[derive(Clone, Copy)]
struct Note {
    rotors: [[Rotor; 2]; MODES],
    levels: [f32; MODES],
    decays: [f32; MODES],
    pan: [f32; 2],
    age: f32,
    /// Gain while making room for the next note, and that note.
    gain: f32,
    pending: Option<(f32, f32, f32, f32)>,
}

impl Note {
    fn loudness(&self) -> f32 {
        self.levels[0] * self.gain
    }
}

pub struct Bell {
    sample_rate: f32,
    settings: Settings,
    notes: [Note; NOTES],
}

impl Bell {
    pub fn new(sample_rate: f32, settings: Settings) -> Self {
        let note = Note {
            rotors: [[Rotor::new(0.0); 2]; MODES],
            levels: [0.0; MODES],
            decays: [0.0; MODES],
            pan: [std::f32::consts::FRAC_1_SQRT_2; 2],
            age: 0.0,
            gain: 1.0,
            pending: None,
        };
        Self {
            sample_rate,
            settings,
            notes: [note; NOTES],
        }
    }

    /// Strikes a note at `frequency`, panned from −1 (left) to 1 (right). It takes the
    /// quietest of the ringing notes' place.
    pub fn trigger(&mut self, velocity: f32, frequency: f32, pan: f32, heat: f32) {
        let note = self
            .notes
            .iter_mut()
            .min_by(|a, b| a.loudness().total_cmp(&b.loudness()))
            .expect("notes");
        if note.loudness() > 1e-4 {
            note.pending = Some((velocity, frequency, pan, heat));
        } else {
            Self::start(
                note,
                &self.settings,
                self.sample_rate,
                velocity,
                frequency,
                pan,
                heat,
            );
        }
    }

    fn start(
        note: &mut Note,
        settings: &Settings,
        sample_rate: f32,
        velocity: f32,
        frequency: f32,
        pan: f32,
        heat: f32,
    ) {
        let warmth = smooth(heat);
        let beat = lerp(settings.beat[0], settings.beat[1], warmth);
        let decay = lerp(settings.decay[0], settings.decay[1], warmth);
        let top = 0.45 * sample_rate;
        for mode in 0..MODES {
            let ratio = settings.ratios[mode];
            for (twin, rotor) in note.rotors[mode].iter_mut().enumerate() {
                rotor.set_frequency((frequency + twin as f32 * beat) * ratio, sample_rate);
            }
            let level = lerp(settings.cold[mode], settings.warm[mode], warmth);
            // Soft notes are darker.
            let softness = velocity.powf(mode as f32 * 0.5);
            note.levels[mode] = if frequency * ratio < top {
                0.5 * velocity * level * softness
            } else {
                0.0
            };
            let time = decay / (1.0 + settings.damping * mode as f32);
            note.decays[mode] = (-1.0 / (time * sample_rate)).exp();
        }
        let angle = (0.5 + 0.5 * pan.clamp(-1.0, 1.0)) * std::f32::consts::FRAC_PI_2;
        note.pan = [angle.cos(), angle.sin()];
        note.age = 0.0;
        note.gain = 1.0;
    }

    /// Keeps the oscillators exact; once per block.
    pub fn normalize(&mut self) {
        for rotor in self
            .notes
            .iter_mut()
            .flat_map(|note| note.rotors.iter_mut().flatten())
        {
            rotor.normalize();
        }
    }

    #[inline]
    pub fn next(&mut self) -> [f32; 2] {
        let step = 1.0 / self.sample_rate;
        let fade = 1.0 / (0.003 * self.sample_rate);
        let mut output = [0.0; 2];
        for note in &mut self.notes {
            if note.pending.is_some() {
                note.gain -= fade;
                if note.gain <= 0.0 {
                    let (velocity, frequency, pan, heat) = note.pending.take().unwrap_or_default();
                    Self::start(
                        note,
                        &self.settings,
                        self.sample_rate,
                        velocity,
                        frequency,
                        pan,
                        heat,
                    );
                }
            }
            if note.levels[0] * note.gain < 1e-5 {
                continue;
            }
            let mut sum = 0.0;
            for mode in 0..MODES {
                let [one, two] = &mut note.rotors[mode];
                sum += (one.next() + two.next()) * note.levels[mode];
                note.levels[mode] *= note.decays[mode];
            }
            let attack = smooth(note.age / self.settings.attack);
            note.age += step;
            let sample = sum * attack * note.gain.max(0.0);
            output[0] += sample * note.pan[0];
            output[1] += sample * note.pan[1];
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Peak of the bell's output over `seconds`, after `skip` seconds.
    fn peak(bell: &mut Bell, rate: f32, skip: f32, seconds: f32) -> f32 {
        for _ in 0..(skip * rate) as usize {
            bell.next();
        }
        (0..(seconds * rate) as usize)
            .map(|_| bell.next()[0].abs())
            .fold(0.0, f32::max)
    }

    #[test]
    fn notes_ring_die_away_and_make_room() {
        let rate = 16_000.0;
        let mut bell = Bell::new(rate, Settings::BAR);
        assert_eq!(bell.next(), [0.0; 2]);
        bell.trigger(1.0, 440.0, 0.0, 0.5);
        let early = peak(&mut bell, rate, 0.0, 0.1);
        let late = peak(&mut bell, rate, 2.0, 0.1);
        assert!(early > 0.3 && late < 0.2 * early, "{early}, {late}");
        // More notes than places: the quietest make room.
        for index in 0..40 {
            bell.trigger(1.0, 300.0 + 37.0 * index as f32, 0.0, 0.5);
            for _ in 0..400 {
                let [left, right] = bell.next();
                assert!(left.abs() < 2.0 * NOTES as f32 && right.is_finite());
            }
        }
    }
}
