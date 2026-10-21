//! The organ: a combo organ after the Vox Continental, holding the chord.
//!
//! Every key sounds the footages of drawbars, from the 16' an octave below the tone through
//! the 8' (the tone itself) to the 1' three octaves above, all taken from one phasor, so they
//! stay locked in phase: the chord stands still and pure where the bed breathes and drifts.
//! Cold is a flute registration (8', 4' and a little 2'); warm the reedy combo organ, with
//! the 16' and the fifths and thirds of the mixture, and towards the warmest a vibrato. Each
//! key's registration is normalized by its K-weighted level, so Heat keeps the loudness, and
//! footages above `ceiling` roll off by 24 dB per octave, so the reed never buzzes
//! piercingly. Keys the chord keeps go on sounding; new ones speak and old ones fade over
//! the time the change asks for. Each key can lean forward or back, as the colors of the
//! light ask, while the chord keeps its level. The instrument can take the footages above
//! the 8' back, as a gate of brightness, while the chord sounds on.
//!
//! It plays through a rotary speaker: a horn turns the footages above the 8', a drum the
//! others, each moving them in level, in pitch (by Doppler) and between the sides, and each
//! taking its own time to reach a new speed, the horn quicker than the drum, so they beat
//! against each other on the way. It moves the chord gently, never far from its level:
//! stopped, the chord stands still; slow is a chorale, fast a tremolo, and faster than any
//! cabinet turns, the movement in level becomes a roughness of its own, while the Doppler
//! is held to a slight vibrato.

use crate::dsp::{Curve, DelayLine, Rotor, coefficient, k_weight, lerp, smooth};
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
    /// Vibrato rate in Hz, and its depth in cents over Heat.
    pub vibrato_hz: f32,
    pub vibrato_cents: Curve,
    /// Footages above this frequency in Hz roll off by 24 dB per octave.
    pub ceiling: f32,
    /// RMS level of the whole chord.
    pub level: f32,
    /// The rotary speaker's horn and drum: how deep they move the level, from 0 to 1, their
    /// largest Doppler delay in seconds, and the seconds they take to reach a new speed. The
    /// drum turns at `drum` times the horn's speed; the Doppler never bends the pitch further
    /// than `swing`, as a share of the frequency, however fast they turn.
    pub rotary: [f32; 2],
    pub doppler: [f32; 2],
    pub inertia: [f32; 2],
    pub drum: f32,
    pub swing: f32,
}

impl Settings {
    /// A combo organ, from flute to reed.
    pub const COMBO: Self = Self {
        cold: [0.0, 1.0, 0.45, 0.0, 0.15, 0.0, 0.0, 0.0],
        warm: [0.45, 1.0, 0.4, 0.5, 0.25, 0.3, 0.12, 0.1],
        vibrato_hz: 6.0,
        vibrato_cents: Curve::new(&[[0.6, 0.0], [1.0, 14.0]]),
        ceiling: 2500.0,
        level: 0.1,
        rotary: [0.15, 0.06],
        doppler: [0.00015, 0.00005],
        inertia: [0.8, 3.5],
        drum: 0.85,
        swing: 0.006,
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

/// A turning part of the rotary speaker, horn or drum, with its delay lines for the left
/// and the right.
struct Turning {
    /// Turns so far, from 0 to 1, and the speed in turns per second.
    angle: f32,
    speed: f32,
    /// Doppler delay in samples, at most and now.
    reach: f32,
    doppler: f32,
    lines: [DelayLine; 2],
}

impl Turning {
    fn new(angle: f32, reach: f32, sample_rate: f32) -> Self {
        let lines = [0, 1].map(|_| DelayLine::new(reach + 4.0 / sample_rate, sample_rate));
        Self {
            angle,
            speed: 0.0,
            reach: reach * sample_rate,
            doppler: reach * sample_rate,
            lines,
        }
    }

    /// Moves towards `speed` by `share` of the way, holding the Doppler to `swing`.
    fn turn_towards(&mut self, speed: f32, share: f32, swing: f32, sample_rate: f32) {
        self.speed += (speed - self.speed) * share;
        let held = swing * sample_rate / (std::f32::consts::TAU * self.speed.max(1e-3));
        self.doppler = self.reach.min(held);
    }

    /// The left and right sides as microphones on opposite sides of the cabinet hear them:
    /// louder as it points their way, higher as it turns towards them.
    #[inline]
    fn next(&mut self, input: [f32; 2], depth: f32, sample_rate: f32) -> [f32; 2] {
        self.angle = (self.angle + self.speed / sample_rate).fract();
        let (sin, cos) = (std::f32::consts::TAU * self.angle).sin_cos();
        let middle = self.reach + 2.0;
        let mut output = [0.0; 2];
        for (side, sign) in [1.0, -1.0].into_iter().enumerate() {
            self.lines[side].write(input[side]);
            let delayed = self.lines[side].read(middle - sign * self.doppler * sin);
            output[side] = delayed * (1.0 + sign * depth * cos);
        }
        output
    }
}

pub struct Organ {
    sample_rate: f32,
    settings: Settings,
    keys: [Key; KEYS],
    /// The rotary speaker's horn and drum.
    horn: Turning,
    drum: Turning,
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
            horn: Turning::new(0.0, settings.doppler[0], sample_rate),
            drum: Turning::new(0.3, settings.doppler[1], sample_rate),
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

    /// Once per block: the registration, the vibrato, the chord's level, and the rotary
    /// speaker's horn turning towards `speed` turns per second.
    pub fn update_block(&mut self, heat: f32, speed: f32, block: usize) {
        let warmth = smooth(heat);
        let settings = &self.settings;
        let rate = self.sample_rate / block as f32;
        let targets = [speed, speed * settings.drum];
        for ((turning, inertia), target) in [&mut self.horn, &mut self.drum]
            .into_iter()
            .zip(settings.inertia)
            .zip(targets)
        {
            let share = coefficient(inertia, rate);
            turning.turn_towards(target, share, settings.swing, self.sample_rate);
        }
        self.vibrato =
            (self.vibrato + settings.vibrato_hz * block as f32 / self.sample_rate).fract();
        let cents = settings.vibrato_cents.at(heat) * (std::f32::consts::TAU * self.vibrato).sin();
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

    /// The next sample, with `brightness` scaling the footages above the 8'.
    #[inline]
    pub fn next(&mut self, brightness: f32) -> [f32; 2] {
        let mut low_out = [0.0; 2];
        let mut high_out = [0.0; 2];
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
            let [low, high] = [0..2, 2..FOOTAGES].map(|range| {
                powers[range.clone()]
                    .iter()
                    .zip(&key.footages[range])
                    .map(|((_, sine), level)| sine * level)
                    .sum::<f32>()
            });
            let gain = key.gain * key.lean * self.norm;
            for side in 0..2 {
                low_out[side] += low * gain * key.pan[side];
                high_out[side] += high * gain * key.pan[side];
            }
        }
        let [horn, drum] = self.settings.rotary;
        let high = high_out.map(|sample| sample * brightness);
        let high = self.horn.next(high, horn, self.sample_rate);
        let low = self.drum.next(low_out, drum, self.sample_rate);
        [low[0] + high[0], low[1] + high[1]]
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
                organ.update_block(heat, 0.0, 32);
            }
            let sample = organ.next(1.0)[0];
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

    /// How far the level of the left side moves over 40 ms windows, as the ratio of the
    /// loudest to the quietest, for one key with the horn turning at `speed`, once it has
    /// got there.
    fn movement(speed: f32) -> f32 {
        let mut organ = Organ::new(RATE, Settings::COMBO);
        organ.play(&chord(&[220.0]), 0.02);
        let window = (0.04 * RATE) as usize;
        let (mut levels, mut sum) = (Vec::new(), 0.0);
        for frame in 0..(12.0 * RATE) as usize {
            if frame % 32 == 0 {
                organ.update_block(0.5, speed, 32);
            }
            let sample = organ.next(1.0)[0];
            if frame >= (10.0 * RATE) as usize {
                sum += sample * sample;
                if (frame + 1) % window == 0 {
                    levels.push(sum);
                    sum = 0.0;
                }
            }
        }
        let loudest = levels.iter().copied().fold(0.0, f32::max);
        let quietest = levels.iter().copied().fold(f32::INFINITY, f32::min);
        (loudest / quietest).sqrt()
    }

    #[test]
    fn the_rotary_speaker_stands_still_or_turns() {
        let (still, turning) = (movement(0.0), movement(6.0));
        assert!(still < 1.05 && turning > 1.15, "{still}, {turning}");
    }
}
