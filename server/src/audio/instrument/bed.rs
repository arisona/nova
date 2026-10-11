//! The drone bed and the air above it.
//!
//! The bed is an additive partial bank: every tone of the harmony is a set of partials, each
//! with a slightly detuned twin it beats against, a slow random amplitude drift and a place
//! in the stereo field (centred in the bass). The root is a synth bass: centred, steadier,
//! with its own spectrum and level. Heat crossfades from cold to warm: cold is
//! thin and glassy (pure fundamentals, weak low harmonics, stretched upper partials, upper
//! tones emphasized), warm is round (strong low harmonics, rolled-off highs, lower tones
//! emphasized, wider beating and a gentle wow). Warmth also keeps the chord from sounding
//! like an organ: breath (narrow-band noise modulating each tone's amplitude, which spreads
//! its partials into breathy bands), a slowly drifting brightness per tone, slightly detuned
//! upper partials, deeper movement, and tones that fade back now and then. Void keeps only
//! the strongest share of
//! partials, ranked like Void by rank in the visuals; the root's fundamental always stays. A
//! K-weighted level estimate keeps the loudness constant whatever Heat, Void or the palette
//! do.
//!
//! The air is noise through narrow resonators tuned to the harmony, strongest when cold.

use super::{BLOCK, Harmony, MAX_TONES, Params, Tone};
use crate::audio::dsp::{Drift, Rng, Rotor, Svf, coefficient, k_weight, lerp, smooth, smoothstep};

const PARTIALS: usize = 8;

/// Partial levels when cold and warm.
const COLD: [f32; PARTIALS] = [1.0, 0.06, 0.03, 0.05, 0.08, 0.03, 0.06, 0.04];
const WARM: [f32; PARTIALS] = [1.0, 0.62, 0.42, 0.28, 0.16, 0.08, 0.04, 0.02];

/// Partial levels of the bass (the root) when cold and warm: hollow and square-like when
/// cold, full and saw-like when warm.
const BASS_COLD: [f32; PARTIALS] = [1.0, 0.12, 0.38, 0.08, 0.2, 0.05, 0.1, 0.03];
const BASS_WARM: [f32; PARTIALS] = [1.0, 0.6, 0.4, 0.28, 0.18, 0.11, 0.07, 0.04];

/// The bass's harmonics roll off above this frequency when cold and warm.
const BASS_ROLLOFF: [f32; 2] = [420.0, 300.0];

/// K-weighted RMS levels of the bass and of the chord above it.
const BASS_LEVEL: f32 = 0.06;
const LEVEL: f32 = 0.08;

/// Partials roll off above this frequency, like a gentle 12 dB per octave low-pass.
const ROLLOFF: f32 = 2400.0;

/// Seconds over which a new harmony replaces the old one.
const CROSSFADE_SECONDS: f32 = 5.0;

/// Depth of the breath when cold and warm, and its bandwidth in Hz.
const BREATH: [f32; 2] = [0.06, 0.3];
const BREATH_HZ: f32 = 40.0;

/// Largest detuning of the chord's upper partials when warm, in cents.
const DETUNE_CENTS: f32 = 8.0;

/// Range of the drifting brightness of the chord tones when warm, in harmonics: partials
/// above it fade.
const SWEEP: [f32; 2] = [1.5, 6.0];

/// Seconds the bass takes to glide to a new note.
const GLIDE_SECONDS: f32 = 8.0;

/// Seconds a chord tone takes to fade out before it changes its octave, and to fade back in.
const VOICE_SECONDS: f32 = 6.0;

/// The bass's accent on thumps at full coupling (see `pulse.rs`): added level, and added
/// level of the top harmonic, rising from the fundamental, like an opening filter.
const BASS_ACCENT: [f32; 2] = [0.8, 2.4];

/// Chord tones change their octave within this range, in Hz.
const VOICE_RANGE: [f32; 2] = [120.0, 1600.0];

#[derive(Clone, Copy)]
struct Partial {
    oscillators: [Rotor; 2],
    /// Offset of the twin in Hz, before scaling by Heat and frequency.
    beat: f32,
    pan: [f32; 2],
    drift: Drift,
    drift_rate: f32,
    /// Detuning when warm, from −1 to 1 of `DETUNE_CENTS`.
    detune: f32,
    gains: [f32; 2],
    steps: [f32; 2],
    mask: f32,
    frequency: f32,
    /// Level before Void, for ranking.
    strength: f32,
    /// Level after Void and the crossfade, before loudness normalization.
    level: f32,
}

impl Partial {
    fn new(rng: &mut Rng) -> Self {
        Self {
            oscillators: [
                Rotor::new(rng.range(0.0, std::f32::consts::TAU)),
                Rotor::new(rng.range(0.0, std::f32::consts::TAU)),
            ],
            beat: rng.range(0.03, 0.22) * if rng.unit() < 0.5 { -1.0 } else { 1.0 },
            pan: [std::f32::consts::FRAC_1_SQRT_2; 2],
            drift: Drift::new(rng),
            drift_rate: rng.range(0.5, 1.5),
            detune: rng.range(-1.0, 1.0),
            gains: [0.0; 2],
            steps: [0.0; 2],
            mask: 1.0,
            frequency: 0.0,
            strength: 0.0,
            level: 0.0,
        }
    }
}

/// The partials of one harmony; two sets crossfade when the palette changes.
struct Set {
    tones: [Tone; MAX_TONES],
    count: usize,
    partials: [[Partial; PARTIALS]; MAX_TONES],
    tone_drifts: [Drift; MAX_TONES],
    sweeps: [Drift; MAX_TONES],
    /// Fourth-order Butterworth low-pass per tone, which turns noise into the breath.
    breaths: [[Svf; 2]; MAX_TONES],
    /// The bass's glide: from, to, and progress from 0 to 1.
    glide: [f32; 3],
    /// Octave of each tone, as a factor of its frequency in the harmony.
    octaves: [f32; MAX_TONES],
    /// The chord tone changing its octave, with its progress from 0 to 2: out, then in.
    voice: Option<(usize, f32)>,
    next_voice: usize,
    level: f32,
}

impl Set {
    fn new(sample_rate: f32, rng: &mut Rng) -> Self {
        Self {
            tones: [Tone::default(); MAX_TONES],
            count: 0,
            partials: std::array::from_fn(|_| std::array::from_fn(|_| Partial::new(rng))),
            tone_drifts: std::array::from_fn(|_| Drift::new(rng)),
            sweeps: std::array::from_fn(|_| Drift::new(rng)),
            breaths: [[
                Svf::new(BREATH_HZ, 0.541, sample_rate),
                Svf::new(BREATH_HZ, 1.307, sample_rate),
            ]; MAX_TONES],
            glide: [0.0, 0.0, 1.0],
            octaves: [1.0; MAX_TONES],
            voice: None,
            next_voice: 1,
            level: 0.0,
        }
    }

    fn load(&mut self, harmony: &Harmony, rng: &mut Rng) {
        self.count = harmony.count;
        self.tones = harmony.tones;
        self.glide = [harmony.root(), harmony.root(), 1.0];
        self.octaves = [1.0; MAX_TONES];
        self.voice = None;
        self.next_voice = 1;
        for (tone_index, (tone, partials)) in
            harmony.tones().iter().zip(&mut self.partials).enumerate()
        {
            for (index, partial) in partials.iter_mut().enumerate() {
                *partial = Partial::new(rng);
                // The bass stays centred; partials spread wider the higher they are.
                let frequency = tone.frequency * (index + 1) as f32;
                let width = if tone_index == 0 {
                    0.0
                } else {
                    ((frequency - 160.0) / 400.0).clamp(0.0, 1.0)
                };
                let angle =
                    (0.5 + 0.4 * width * rng.range(-1.0, 1.0)) * std::f32::consts::FRAC_PI_2;
                partial.pan = [angle.cos(), angle.sin()];
            }
        }
    }
}

/// Level of a partial's twin: the bass beats only gently, so the drone keeps its floor.
fn partial_twin(twin: f32, frequency: f32) -> f32 {
    if frequency < 150.0 {
        twin.min(0.3)
    } else {
        twin
    }
}

pub struct Bed {
    sample_rate: f32,
    sets: [Set; 2],
    current: usize,
    fade: f32,
    rng: Rng,
    seconds: f32,
    wow: Drift,
    /// Of the bass and of the chord.
    normalization: [f32; 2],
    order: [u8; MAX_TONES * PARTIALS],
    breath: f32,
    breath_step: f32,
    bass_accent: f32,
    /// Scales the low-passed noise to an RMS of 1.
    breath_scale: f32,
}

impl Bed {
    pub fn new(sample_rate: f32, harmony: &Harmony, mut rng: Rng) -> Self {
        let mut sets = [
            Set::new(sample_rate, &mut rng),
            Set::new(sample_rate, &mut rng),
        ];
        sets[0].load(harmony, &mut rng);
        sets[0].level = 1.0;
        Self {
            sample_rate,
            sets,
            current: 0,
            fade: 1.0,
            wow: Drift::new(&mut rng),
            rng,
            seconds: 0.0,
            normalization: [0.0; 2],
            order: std::array::from_fn(|index| index as u8),
            breath: 0.0,
            breath_step: 0.0,
            bass_accent: 0.0,
            // Uniform noise has a variance of 1/3; the low-pass passes 1.026 × BREATH_HZ of
            // its bandwidth of half the sample rate.
            breath_scale: (3.0 * sample_rate / (2.0 * 1.026 * BREATH_HZ)).sqrt(),
        }
    }

    /// Switches to a harmony at once, without a crossfade.
    pub fn load(&mut self, harmony: &Harmony) {
        self.sets[0].load(harmony, &mut self.rng);
        self.sets[0].level = 1.0;
        self.sets[1].count = 0;
        self.current = 0;
        self.fade = 1.0;
    }

    /// Glides the bass of the current harmony to `frequency`.
    pub fn set_bass(&mut self, frequency: f32) {
        let set = &mut self.sets[self.current];
        set.glide = [set.tones[0].frequency, frequency, 0.0];
    }

    /// The bass's accent, from the pulse.
    pub fn set_bass_accent(&mut self, accent: f32) {
        self.bass_accent = accent;
    }

    /// Moves the next chord tone of the current harmony to another octave: it fades out,
    /// changes its octave and fades back in.
    pub fn move_voice(&mut self) {
        let set = &mut self.sets[self.current];
        if set.count < 2 || set.voice.is_some() {
            return;
        }
        let index = set.next_voice.min(set.count - 1);
        set.next_voice = if index + 1 < set.count { index + 1 } else { 1 };
        set.voice = Some((index, 0.0));
    }

    #[cfg(test)]
    fn bass(&self) -> f32 {
        self.sets[self.current].tones[0].frequency
    }

    pub fn fading(&self) -> bool {
        self.fade < 1.0
    }

    /// Crossfades to a new harmony.
    pub fn change(&mut self, harmony: &Harmony) {
        let next = 1 - self.current;
        self.sets[next].load(harmony, &mut self.rng);
        self.sets[next].level = 0.0;
        self.current = next;
        self.fade = 0.0;
    }

    pub fn update_block(&mut self, params: &Params) {
        let block_seconds = BLOCK as f32 / self.sample_rate;
        self.seconds += block_seconds;
        if self.fade < 1.0 {
            self.fade = (self.fade + block_seconds / CROSSFADE_SECONDS).min(1.0);
            self.sets[self.current].level = smooth(self.fade);
            self.sets[1 - self.current].level = 1.0 - smooth(self.fade);
            if self.fade >= 1.0 {
                self.sets[1 - self.current].count = 0;
            }
        }

        let warmth = smooth(params.heat);
        let stretch = 0.035 * (1.0 - 2.0 * params.heat).max(0.0);
        let wow_depth = 0.0012 * warmth;
        let wow = 1.0
            + wow_depth
                * (0.6 * (std::f32::consts::TAU * 0.11 * self.seconds).sin()
                    + 0.8 * (self.wow.advance(0.05 * block_seconds, &mut self.rng) - 0.5));
        let twin = 0.3 + 0.4 * warmth;
        // Tones above the root lean up when cold and down when warm.
        let tilt = lerp(0.15, -0.3, warmth);
        let depth = (0.45 + 0.35 * params.activity + 0.15 * warmth).min(0.95);
        let partial_rate = (0.02 + 0.2 * params.flow) * (1.0 + warmth) * block_seconds;
        let tone_rate = (0.01 + 0.08 * params.flow) * block_seconds;
        let sweep_rate = (0.01 + 0.06 * params.flow) * block_seconds;
        // Chord tones dip lower when warm.
        let tone_floor = lerp(0.6, 0.3, warmth);
        self.breath_step = (lerp(BREATH[0], BREATH[1], warmth) - self.breath) / BLOCK as f32;
        let top = (0.42 * self.sample_rate).min(14_000.0);
        let keep = 1.0 - 0.9 * params.void;
        let masks = coefficient(0.6, self.sample_rate / BLOCK as f32);

        let bass_rolloff = lerp(BASS_ROLLOFF[0], BASS_ROLLOFF[1], warmth);
        // Of the bass and of the chord.
        let mut power = [0.0; 2];
        for set in &mut self.sets {
            if set.count == 0 {
                continue;
            }
            let [from, to, progress] = &mut set.glide;
            if *progress < 1.0 {
                *progress = (*progress + block_seconds / GLIDE_SECONDS).min(1.0);
                set.tones[0].frequency = *from * (*to / *from).powf(smooth(*progress));
            }
            let mut voice = (usize::MAX, 1.0);
            if let Some((index, progress)) = &mut set.voice {
                let before = *progress;
                *progress += block_seconds / VOICE_SECONDS;
                if before < 1.0 && *progress >= 1.0 {
                    // Silent now: change the octave, towards the middle of the range.
                    let frequency = set.tones[*index].frequency * set.octaves[*index];
                    let down = frequency * 0.5 >= VOICE_RANGE[0]
                        && (frequency > 400.0 || frequency * 2.0 > VOICE_RANGE[1]);
                    set.octaves[*index] *= if down { 0.5 } else { 2.0 };
                }
                let level = if *progress < 1.0 {
                    1.0 - smooth(*progress)
                } else {
                    smooth(*progress - 1.0)
                };
                voice = (*index, level);
                if *progress >= 2.0 {
                    set.voice = None;
                }
            }
            for tone_index in 0..set.count {
                let mut tone = set.tones[tone_index];
                tone.frequency *= set.octaves[tone_index];
                let bass = tone_index == 0;
                let tone_level = if bass {
                    1.0
                } else {
                    tone.level * (tone.frequency / 400.0).powf(tilt).clamp(0.4, 2.0)
                } * if tone_index == voice.0 { voice.1 } else { 1.0 };
                let tone_motion = tone_floor
                    + (1.0 - tone_floor)
                        * set.tone_drifts[tone_index].advance(
                            tone_rate * set.partials[tone_index][0].drift_rate,
                            &mut self.rng,
                        );
                let sweep = lerp(
                    SWEEP[0],
                    SWEEP[1],
                    set.sweeps[tone_index].advance(sweep_rate, &mut self.rng),
                );
                for (index, partial) in set.partials[tone_index].iter_mut().enumerate() {
                    let harmonic = (index + 1) as f32;
                    let detune = if bass || index == 0 {
                        1.0
                    } else {
                        2.0_f32.powf(DETUNE_CENTS * warmth * partial.detune / 1200.0)
                    };
                    let frequency = tone.frequency * harmonic.powf(1.0 + stretch) * wow * detune;
                    let brightness = if bass {
                        1.0
                    } else {
                        lerp(1.0, 1.0 / (1.0 + (harmonic / sweep).powi(4)).sqrt(), warmth)
                    };
                    let depth = if bass { 0.4 * depth } else { depth };
                    let motion = 1.0 - depth
                        + depth
                            * partial
                                .drift
                                .advance(partial_rate * partial.drift_rate, &mut self.rng);
                    let anchor = tone_index == 0 && index == 0;
                    let weight = if anchor {
                        1.0
                    } else {
                        params.band_weight(frequency)
                    };
                    let (profile, rolloff) = if bass {
                        (
                            lerp(BASS_COLD[index], BASS_WARM[index], warmth),
                            bass_rolloff,
                        )
                    } else {
                        (lerp(COLD[index], WARM[index], warmth), ROLLOFF)
                    };
                    let ceiling = ((top - frequency) / (0.2 * top)).clamp(0.0, 1.0)
                        / (1.0 + (frequency / rolloff).powi(4)).sqrt();
                    partial.frequency = frequency;
                    partial.strength =
                        tone_level * tone_motion * profile * brightness * motion * weight * ceiling;
                }
            }

            // Void by rank: only the strongest share keeps sounding.
            let total = set.count * PARTIALS;
            let order = &mut self.order[..total];
            for (position, slot) in order.iter_mut().enumerate() {
                *slot = position as u8;
            }
            let strength = |slot: u8| {
                set.partials[slot as usize / PARTIALS][slot as usize % PARTIALS].strength
            };
            order.sort_unstable_by(|a, b| strength(*b).total_cmp(&strength(*a)));
            let audible = order
                .iter()
                .filter(|&&slot| strength(slot) > 1e-4)
                .count()
                .max(1);
            for (rank, &slot) in order.iter().enumerate() {
                let (tone_index, index) = (slot as usize / PARTIALS, slot as usize % PARTIALS);
                let partial = &mut set.partials[tone_index][index];
                let position = (rank as f32 + 0.5) / audible as f32;
                let target = if tone_index == 0 && index == 0 {
                    1.0
                } else {
                    1.0 - smoothstep(keep - 0.06, keep + 0.06, position)
                };
                partial.mask += (target - partial.mask) * masks;
                partial.level = partial.strength * partial.mask * set.level;
                let weighted = partial.level * k_weight(partial.frequency);
                power[(tone_index > 0) as usize] += 0.5
                    * weighted
                    * weighted
                    * (1.0 + partial_twin(twin, partial.frequency).powi(2));
            }
        }

        for ((normalization, power), level) in self
            .normalization
            .iter_mut()
            .zip(power)
            .zip([BASS_LEVEL, LEVEL])
        {
            let target = (level / (power.sqrt() + 1e-6)).min(8.0);
            *normalization +=
                (target - *normalization) * coefficient(0.4, self.sample_rate / BLOCK as f32);
        }

        for set in &mut self.sets {
            for tone_index in 0..set.count {
                let normalization = self.normalization[(tone_index > 0) as usize];
                for (index, partial) in set.partials[tone_index].iter_mut().enumerate() {
                    let accent = if tone_index == 0 {
                        let top = index as f32 / (PARTIALS - 1) as f32;
                        1.0 + self.bass_accent * (BASS_ACCENT[0] + BASS_ACCENT[1] * top)
                    } else {
                        1.0
                    };
                    let main = partial.level * normalization * accent;
                    let targets = [main, main * partial_twin(twin, partial.frequency)];
                    let beat = partial.beat
                        * (0.4 + 1.2 * params.heat)
                        * (1.0 + partial.frequency / 1500.0).min(4.0);
                    let beat = if partial.frequency < 150.0 {
                        beat.clamp(-0.12, 0.12)
                    } else {
                        beat
                    };
                    partial.oscillators[0].set_frequency(partial.frequency, self.sample_rate);
                    partial.oscillators[1]
                        .set_frequency(partial.frequency + beat, self.sample_rate);
                    for ((oscillator, step), (target, gain)) in partial
                        .oscillators
                        .iter_mut()
                        .zip(&mut partial.steps)
                        .zip(targets.iter().zip(&partial.gains))
                    {
                        oscillator.normalize();
                        *step = (target - gain) / BLOCK as f32;
                    }
                }
            }
        }
    }

    #[inline]
    /// `chord` scales the chord tones; the bass is left alone.
    pub fn next(&mut self, chord: f32) -> [f32; 2] {
        self.breath += self.breath_step;
        let mut output = [0.0; 2];
        for set in &mut self.sets {
            for (tone_index, partials) in set.partials[..set.count].iter_mut().enumerate() {
                let mut tone = [0.0; 2];
                for partial in partials.iter_mut() {
                    if partial.gains[0] <= 0.0 && partial.steps[0] <= 0.0 {
                        continue;
                    }
                    let sample = partial.oscillators[0].next() * partial.gains[0]
                        + partial.oscillators[1].next() * partial.gains[1];
                    partial.gains[0] = (partial.gains[0] + partial.steps[0]).max(0.0);
                    partial.gains[1] = (partial.gains[1] + partial.steps[1]).max(0.0);
                    tone[0] += sample * partial.pan[0];
                    tone[1] += sample * partial.pan[1];
                }
                // The bass stays steady; chord tones breathe.
                let gain = if tone_index == 0 {
                    1.0
                } else {
                    let noise = set.breaths[tone_index]
                        .iter_mut()
                        .fold(self.rng.range(-1.0, 1.0), |noise, filter| {
                            filter.lowpass(noise)
                        });
                    (1.0 + self.breath * self.breath_scale * noise).max(0.0) * chord
                };
                output[0] += tone[0] * gain;
                output[1] += tone[1] * gain;
            }
        }
        output
    }
}

/// Noise through narrow resonators tuned to the harmony, folded into 500 Hz to 4 kHz:
/// wind and breath, strongest when cold.
pub struct Air {
    sample_rate: f32,
    resonators: [[Svf; 2]; MAX_TONES],
    frequencies: [f32; MAX_TONES],
    targets: [f32; MAX_TONES],
    drifts: [Drift; MAX_TONES],
    gains: [f32; MAX_TONES],
    count: usize,
    level: f32,
    step: f32,
    rng: Rng,
}

impl Air {
    pub fn new(sample_rate: f32, harmony: &Harmony, mut rng: Rng) -> Self {
        let mut air = Self {
            sample_rate,
            resonators: [[Svf::default(); 2]; MAX_TONES],
            frequencies: [1000.0; MAX_TONES],
            targets: [1000.0; MAX_TONES],
            drifts: std::array::from_fn(|_| Drift::new(&mut rng)),
            gains: [0.0; MAX_TONES],
            count: 0,
            level: 0.0,
            step: 0.0,
            rng,
        };
        air.change(harmony);
        air.frequencies = air.targets;
        air
    }

    /// Switches to a harmony at once, without gliding.
    pub fn load(&mut self, harmony: &Harmony) {
        self.count = 0;
        self.change(harmony);
    }

    pub fn change(&mut self, harmony: &Harmony) {
        // Resonators keep gliding from where they are; new ones start at their target.
        for (index, tone) in harmony.tones().iter().enumerate() {
            let mut frequency = tone.frequency;
            while frequency < 500.0 {
                frequency *= 2.0;
            }
            while frequency >= 4000.0 {
                frequency *= 0.5;
            }
            self.targets[index] = frequency;
            if index >= self.count {
                self.frequencies[index] = frequency;
            }
        }
        self.count = harmony.count;
    }

    pub fn update_block(&mut self, params: &Params) {
        let block_seconds = BLOCK as f32 / self.sample_rate;
        let glide = coefficient(2.0, self.sample_rate / BLOCK as f32);
        let warmth = smooth(params.heat);
        let q = lerp(20.0, 10.0, warmth);
        // Narrower bands pass less noise: keep the power independent of Q.
        let normalization = (q / 10.0).sqrt();
        let gust_rate = (0.04 + 0.25 * params.flow) * block_seconds;
        // Even when cold, darker when warm.
        let tilt = lerp(0.0, -1.0, warmth);
        for index in 0..self.count {
            self.frequencies[index] += (self.targets[index] - self.frequencies[index]) * glide;
            for resonator in &mut self.resonators[index] {
                resonator.set(self.frequencies[index], q, self.sample_rate);
            }
            let gust =
                self.drifts[index].advance(gust_rate * (1.0 + 0.3 * index as f32), &mut self.rng);
            self.gains[index] = (0.25 + 0.75 * gust * gust)
                * (self.frequencies[index] / 1000.0).powf(tilt)
                * normalization;
        }
        let target = lerp(0.09, 0.025, warmth) * (0.8 + 0.5 * params.void);
        self.step = (target - self.level) / BLOCK as f32;
    }

    #[inline]
    pub fn next(&mut self) -> [f32; 2] {
        self.level += self.step;
        let noise = [self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0)];
        let mut output = [0.0; 2];
        for index in 0..self.count {
            for channel in 0..2 {
                output[channel] +=
                    self.resonators[index][channel].bandpass(noise[channel]) * self.gains[index];
            }
        }
        output.map(|sample| sample * self.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(bed: &mut Bed, seconds: f32, rate: f32) {
        let params = Params {
            heat: 0.8,
            flow: 0.5,
            form: 0.0,
            void: 0.5,
            flow_setting: 0.5,
            activity: 0.0,
            looseness: 0.0,
            layers: crate::audio::features::Features::default().layers,
        };
        for frame in 0..(seconds * rate) as usize {
            if frame % BLOCK == 0 {
                bed.update_block(&params);
            }
            let [left, right] = bed.next(1.0);
            assert!(left.is_finite() && right.is_finite());
        }
    }

    #[test]
    fn bass_glides_and_chord_tones_change_octave() {
        let rate = 16_000.0;
        let harmony = Harmony::from_palette(&crate::palettes::PALETTES[3]);
        let mut bed = Bed::new(rate, &harmony, Rng::new(2));
        render(&mut bed, 0.5, rate);
        let target = harmony.root() * 0.75;
        bed.set_bass(target);
        render(&mut bed, 4.0, rate);
        let halfway = bed.bass();
        assert!(halfway < harmony.root() && halfway > target, "{halfway}");
        render(&mut bed, 4.5, rate);
        assert_eq!(bed.bass(), target);

        bed.move_voice();
        render(&mut bed, 13.0, rate);
        let set = &bed.sets[bed.current];
        assert!(set.voice.is_none());
        let octave = set.octaves[1];
        assert!(octave == 0.5 || octave == 2.0, "{octave}");
        let frequency = set.tones[1].frequency * octave;
        assert!((VOICE_RANGE[0]..=VOICE_RANGE[1] * 2.0).contains(&frequency));
    }

    #[test]
    fn breath_noise_has_unit_rms() {
        let rate = 48_000.0;
        let harmony = Harmony::from_palette(&crate::palettes::PALETTES[0]);
        let mut bed = Bed::new(rate, &harmony, Rng::new(1));
        let mut sum = 0.0;
        let frames = rate as usize * 20;
        for _ in 0..frames {
            let noise = bed.sets[0].breaths[1]
                .iter_mut()
                .fold(bed.rng.range(-1.0, 1.0), |noise, filter| {
                    filter.lowpass(noise)
                });
            let scaled = noise * bed.breath_scale;
            sum += scaled * scaled;
        }
        let rms = (sum / frames as f32).sqrt();
        assert!((rms - 1.0).abs() < 0.1, "{rms}");
    }
}
