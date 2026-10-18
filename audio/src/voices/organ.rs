//! The organ: a combo organ after the Vox Continental, holding the chord.
//!
//! Every key sounds the footages of drawbars, from the 16' an octave below the tone through
//! the 8' (the tone itself) to the 1' three octaves above, all taken from one phasor, so they
//! stay locked in phase: the chord stands still and pure where the bed breathes and drifts.
//! Cold is a flute registration (8', 4' and a little 2') without vibrato; warm the reedy
//! combo organ, with the 16', the fifths and thirds of the mixture, and a vibrato. Each
//! key's registration is normalized by its K-weighted level, so Heat keeps the loudness, and
//! footages above `ceiling` roll off by 24 dB per octave, so the reed never buzzes
//! piercingly. Keys the chord keeps go on sounding; new ones speak and old ones fade over
//! the time the change asks for. Each key can lean forward or back, as the colors of the
//! light ask, while the chord keeps its level.

use crate::dsp::{Rotor, coefficient, k_weight, lerp, smooth};
use crate::harmony::{MAX_TONES, Tone};

/// Footages per key, as multiples of half the key's frequency: 16', 8', 4', 2 2/3', 2',
/// 1 3/5', 1 1/3' and 1'.
pub const FOOTAGES: usize = 8;
const MULTIPLES: [f32; FOOTAGES] = [1.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0];

/// Keys: the chord's tones, and as many again fading out while a new chord speaks.
const KEYS: usize = 2 * MAX_TONES;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Drawbar levels of the footages when cold and warm.
    pub cold: [f32; FOOTAGES],
    pub warm: [f32; FOOTAGES],
    /// Vibrato rate in Hz, and its depth in cents when cold and warm.
    pub vibrato_hz: f32,
    pub vibrato_cents: [f32; 2],
    /// Footages above this frequency in Hz roll off by 24 dB per octave.
    pub ceiling: f32,
    /// RMS level of the whole chord.
    pub level: f32,
}

impl Settings {
    /// A combo organ, from flute to reed.
    pub const COMBO: Self = Self {
        cold: [0.0, 1.0, 0.45, 0.0, 0.15, 0.0, 0.0, 0.0],
        warm: [0.45, 1.0, 0.4, 0.5, 0.25, 0.3, 0.12, 0.1],
        vibrato_hz: 6.0,
        vibrato_cents: [0.0, 14.0],
        ceiling: 2500.0,
        level: 0.1,
    };
}

#[derive(Clone, Copy)]
struct Key {
    /// Turns at half the key's frequency, the 16'.
    phasor: Rotor,
    frequency: f32,
    /// Level the key moves to, its gain now, and how fast it moves, per sample.
    target: f32,
    gain: f32,
    rate: f32,
    footages: [f32; FOOTAGES],
    pan: [f32; 2],
    /// Gain from the colors of the light.
    lean: f32,
}

impl Key {
    fn silent(&self) -> bool {
        self.target == 0.0 && self.gain < 1e-5
    }
}

pub struct Organ {
    sample_rate: f32,
    settings: Settings,
    keys: [Key; KEYS],
    /// Phase of the vibrato, in turns, and the gain that keeps the chord at its level.
    vibrato: f32,
    norm: f32,
}

impl Organ {
    pub fn new(sample_rate: f32, settings: Settings) -> Self {
        let keys = std::array::from_fn(|index| Key {
            // Divider organs leave the keys' phases apart.
            phasor: Rotor::new(index as f32 * 2.4),
            frequency: 0.0,
            target: 0.0,
            gain: 0.0,
            rate: 0.0,
            footages: [0.0; FOOTAGES],
            pan: [std::f32::consts::FRAC_1_SQRT_2; 2],
            lean: 1.0,
        });
        Self {
            sample_rate,
            settings,
            keys,
            vibrato: 0.0,
            norm: 0.0,
        }
    }

    /// Plays a chord: keys it keeps go on sounding at their new levels, new ones speak and
    /// old ones fall silent, over `seconds`.
    pub fn play(&mut self, tones: &[Tone], seconds: f32) {
        let rate = coefficient(seconds, self.sample_rate);
        let kept = |key: &Key, tone: &Tone| (key.frequency / tone.frequency - 1.0).abs() < 0.005;
        for key in &mut self.keys {
            if key.target > 0.0 && !tones.iter().any(|tone| kept(key, tone)) {
                key.target = 0.0;
                key.rate = rate;
            }
        }
        for tone in tones {
            let index = match self
                .keys
                .iter()
                .position(|key| key.target > 0.0 && kept(key, tone))
            {
                Some(index) => index,
                None => {
                    // A silent key, or else the quietest.
                    let index = self.keys.iter().position(Key::silent).unwrap_or_else(|| {
                        (0..KEYS)
                            .min_by(|&a, &b| self.keys[a].gain.total_cmp(&self.keys[b].gain))
                            .unwrap_or(0)
                    });
                    let key = &mut self.keys[index];
                    key.frequency = tone.frequency;
                    key.phasor
                        .set_frequency(0.5 * tone.frequency, self.sample_rate);
                    // Low keys a little to the left, high ones to the right.
                    let pan = ((tone.frequency / 400.0).log2() * 0.25).clamp(-0.4, 0.4);
                    let angle = (0.5 + 0.5 * pan) * std::f32::consts::FRAC_PI_2;
                    key.pan = [angle.cos(), angle.sin()];
                    key.lean = 1.0;
                    index
                }
            };
            let key = &mut self.keys[index];
            key.target = tone.level.max(1e-3);
            key.rate = rate;
        }
    }

    /// Leans the key that plays `frequency` forward or back by `gain`, as the colors of the
    /// light ask (see `bed::Voicing`).
    pub fn lean(&mut self, frequency: f32, gain: f32) {
        for key in &mut self.keys {
            if key.target > 0.0 && (key.frequency / frequency - 1.0).abs() < 0.005 {
                key.lean = gain;
            }
        }
    }

    /// Once per block: the registration, the vibrato and the chord's level.
    pub fn update_block(&mut self, heat: f32, block: usize) {
        let warmth = smooth(heat);
        let settings = &self.settings;
        self.vibrato =
            (self.vibrato + settings.vibrato_hz * block as f32 / self.sample_rate).fract();
        let cents = lerp(settings.vibrato_cents[0], settings.vibrato_cents[1], warmth)
            * (std::f32::consts::TAU * self.vibrato).sin();
        let bend = 2.0_f32.powf(cents / 1200.0);
        let top = 0.45 * self.sample_rate;
        let mut power = 0.0;
        for key in &mut self.keys {
            if key.silent() {
                continue;
            }
            let half = 0.5 * key.frequency;
            key.phasor.set_frequency(half * bend, self.sample_rate);
            key.phasor.normalize();
            let mut weighted = 0.0;
            for (footage, (level, multiple)) in key.footages.iter_mut().zip(
                settings
                    .cold
                    .iter()
                    .zip(&settings.warm)
                    .map(|(cold, warm)| lerp(*cold, *warm, warmth))
                    .zip(MULTIPLES),
            ) {
                let frequency = half * multiple;
                *footage = if frequency < top {
                    level / (1.0 + (frequency / settings.ceiling).powi(4))
                } else {
                    0.0
                };
                weighted += (*footage * k_weight(frequency)).powi(2);
            }
            // Each key as loud as a sine of its level, whatever the registration.
            let scale = (1.0 / weighted.max(1e-9)).sqrt();
            for footage in &mut key.footages {
                *footage *= scale;
            }
            power += 0.5 * key.target * key.target;
        }
        let norm = settings.level / power.max(1e-6).sqrt();
        self.norm = if self.norm == 0.0 {
            norm
        } else {
            self.norm + 0.05 * (norm - self.norm)
        };
    }

    #[inline]
    pub fn next(&mut self) -> [f32; 2] {
        let mut output = [0.0; 2];
        for key in &mut self.keys {
            key.gain += (key.target - key.gain) * key.rate;
            if key.silent() {
                continue;
            }
            key.phasor.next();
            let one = key.phasor.phasor();
            let two = multiply(one, one);
            let four = multiply(two, two);
            let eight = multiply(four, four);
            let powers = [
                one,
                two,
                four,
                multiply(four, two),
                eight,
                multiply(eight, two),
                multiply(eight, four),
                multiply(eight, eight),
            ];
            let sample = powers
                .iter()
                .zip(&key.footages)
                .map(|((_, sine), level)| sine * level)
                .sum::<f32>()
                * key.gain
                * key.lean
                * self.norm;
            output[0] += sample * key.pan[0];
            output[1] += sample * key.pan[1];
        }
        output
    }
}

/// The product of two phasors, as (cosine, sine): their angles add.
#[inline]
fn multiply((a, b): (f32, f32), (c, d): (f32, f32)) -> (f32, f32) {
    (a * c - b * d, a * d + b * c)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 16_000.0;

    fn chord(frequencies: &[f32]) -> Vec<Tone> {
        frequencies
            .iter()
            .map(|&frequency| Tone {
                frequency,
                level: 1.0,
                hue: None,
            })
            .collect()
    }

    /// RMS of the organ's output over `seconds`, and the largest step between samples.
    fn play(organ: &mut Organ, heat: f32, seconds: f32) -> (f32, f32) {
        let (mut sum, mut step, mut previous) = (0.0, 0.0_f32, 0.0);
        let frames = (seconds * RATE) as usize;
        for frame in 0..frames {
            if frame % 32 == 0 {
                organ.update_block(heat, 32);
            }
            let sample = organ.next()[0];
            sum += sample * sample;
            step = step.max((sample - previous).abs());
            previous = sample;
        }
        ((sum / frames as f32).sqrt(), step)
    }

    #[test]
    fn heat_keeps_the_loudness_and_changes_do_not_click() {
        let levels: Vec<f32> = [0.0, 0.5, 1.0]
            .iter()
            .map(|&heat| {
                let mut organ = Organ::new(RATE, Settings::COMBO);
                organ.play(&chord(&[220.0, 264.0, 330.0]), 0.02);
                play(&mut organ, heat, 0.5);
                play(&mut organ, heat, 1.0).0
            })
            .collect();
        let spread = 20.0 * (levels[0] / levels[2]).log10();
        assert!(levels[0] > 0.02 && spread.abs() < 3.0, "{levels:?}");
        // A new chord: one key stays, two fade out and two speak.
        let mut organ = Organ::new(RATE, Settings::COMBO);
        organ.play(&chord(&[220.0, 264.0, 330.0]), 0.02);
        let (_, held) = play(&mut organ, 1.0, 1.0);
        organ.play(&chord(&[220.0, 293.3, 352.0]), 0.05);
        let (level, change) = play(&mut organ, 1.0, 1.0);
        assert!(
            change < 1.5 * held && level > 0.5 * levels[2],
            "{held}, {change}"
        );
    }
}
