//! The drone bed.
//!
//! An additive partial bank: every tone of the harmony is a set of partials, each with a
//! slightly detuned twin it beats against, a slow random amplitude drift and a place in the
//! stereo field (centred in the bass). The root is a synth bass: centred, steadier, with its
//! own spectrum and level. Heat crossfades from cold to warm: cold is thin and glassy (pure
//! fundamentals, weak low harmonics, stretched upper partials, upper tones emphasized), warm
//! is round (strong low harmonics, rolled-off highs, lower tones emphasized, wider beating
//! and a gentle wow). Warmth also keeps the chord from sounding like an organ: breath
//! (narrow-band noise modulating each tone's amplitude, which spreads its partials into
//! breathy bands), a slowly drifting brightness per tone, slightly detuned upper partials,
//! deeper movement, and tones that fade back now and then. Void keeps only the strongest
//! share of partials, ranked like Void by rank in the visuals; the root's fundamental always
//! stays. A K-weighted level estimate keeps the loudness constant whatever Heat, Void or the
//! palette do.
//!
//! From the signals, the bass takes its accent on thumps, and the chord's upper partials dip
//! with the chord gate, like a filter. Instruments that play the chord in rhythm can fold
//! it into a register, put a resonant low-pass on it, and take it apart from the bass.

use nova_core::HUES;

use crate::BLOCK;
use crate::dsp::{
    Drift, Rng, Rotor, Svf, coefficient, k_weight, lerp, resonant_lowpass, smooth, smoothstep,
};
use crate::harmony::{CROSSFADE_SECONDS, Harmony, MAX_TONES, Tone};
use crate::params::Params;
use crate::signals::Signals;

pub const PARTIALS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Partial levels when cold and warm.
    pub cold: [f32; PARTIALS],
    pub warm: [f32; PARTIALS],
    /// Partial levels of the bass (the root) when cold and warm.
    pub bass_cold: [f32; PARTIALS],
    pub bass_warm: [f32; PARTIALS],
    /// The bass's harmonics roll off above this frequency when cold and warm.
    pub bass_rolloff: [f32; 2],
    /// K-weighted RMS levels of the bass and of the chord above it.
    pub bass_level: f32,
    pub level: f32,
    /// Partials roll off above this frequency, like a gentle 12 dB per octave low-pass.
    pub rolloff: f32,
    /// Depth of the breath when cold and warm, and its bandwidth in Hz.
    pub breath: [f32; 2],
    pub breath_hz: f32,
    /// Largest detuning of the chord's upper partials when warm, in cents.
    pub detune_cents: f32,
    /// Range of the drifting brightness of the chord tones when warm, in harmonics: partials
    /// above it fade.
    pub sweep: [f32; 2],
    /// Seconds the bass takes to glide to a new note.
    pub glide_seconds: f32,
    /// Seconds a chord tone takes to fade out before it changes its octave, and to fade back
    /// in.
    pub voice_seconds: f32,
    /// Chord tones change their octave within this range, in Hz.
    pub voice_range: [f32; 2],
    /// The bass's accent at full strength in the signals: added level, and added level of the
    /// top harmonic, rising from the fundamental, like an opening filter.
    pub bass_accent: [f32; 2],
    /// The range in Hz the chord tones are folded into by octaves, if any; two octaves wide
    /// or more, so tones that land on each other can move apart. A tone that finds both
    /// octaves taken joins the tone there, and octave moves never land on another tone.
    pub register: Option<[f32; 2]>,
    /// How far the colors of the light move the chord tones, in dB: a tone whose color
    /// shows twice as much as the chord's colors on average comes forward by this much, one
    /// whose color is missing steps back as much (see `Features::hues`). The loudness of
    /// the chord stays; 0 ignores the colors.
    pub voicing: f32,
    /// How far the height of the light tilts the chord, in dB per octave around 400 Hz:
    /// light at the top lifts its upper tones and partials by this much, light at the
    /// bottom darkens them as much, light in the middle leaves them (see
    /// `Features::height`). The bass stays, and so does the chord's loudness; 0 ignores the
    /// height.
    pub height_tilt: f32,
}

impl Settings {
    pub const DEFAULT: Self = Self {
        cold: [1.0, 0.06, 0.03, 0.05, 0.08, 0.03, 0.06, 0.04],
        warm: [1.0, 0.62, 0.42, 0.28, 0.16, 0.08, 0.04, 0.02],
        // Hollow and square-like when cold, full and saw-like when warm.
        bass_cold: [1.0, 0.12, 0.38, 0.08, 0.2, 0.05, 0.1, 0.03],
        bass_warm: [1.0, 0.6, 0.4, 0.28, 0.18, 0.11, 0.07, 0.04],
        bass_rolloff: [420.0, 300.0],
        bass_level: 0.06,
        level: 0.08,
        rolloff: 2400.0,
        breath: [0.06, 0.3],
        breath_hz: 40.0,
        detune_cents: 8.0,
        sweep: [1.5, 6.0],
        glide_seconds: 8.0,
        voice_seconds: 6.0,
        voice_range: [120.0, 1600.0],
        bass_accent: [0.8, 2.4],
        register: None,
        voicing: 3.0,
        height_tilt: 1.5,
    };
}

/// A resonant low-pass on the chord tones (see `dsp::resonant_lowpass`), at block rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Filter {
    pub cutoff: f32,
    pub resonance: f32,
    /// Cutoff and resonance at which the chord keeps its loudness, typically the filter's
    /// most open state: the chord is as loud there as without a filter, and quieter when
    /// the filter closes.
    pub reference: [f32; 2],
}

/// One sample of the bed, whole and in parts.
#[derive(Clone, Copy, Debug, Default)]
pub struct Parts {
    /// Bass and chord, the chord scaled as asked.
    pub mix: [f32; 2],
    pub bass: [f32; 2],
    /// The chord tones, unscaled.
    pub chord: [f32; 2],
}

#[derive(Clone, Copy)]
struct Partial {
    oscillators: [Rotor; 2],
    /// Offset of the twin in Hz, before scaling by Heat and frequency.
    beat: f32,
    pan: [f32; 2],
    drift: Drift,
    drift_rate: f32,
    /// Detuning when warm, from −1 to 1 of `Settings::detune_cents`.
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
    fn new(sample_rate: f32, breath_hz: f32, rng: &mut Rng) -> Self {
        Self {
            tones: [Tone::default(); MAX_TONES],
            count: 0,
            partials: std::array::from_fn(|_| std::array::from_fn(|_| Partial::new(rng))),
            tone_drifts: std::array::from_fn(|_| Drift::new(rng)),
            sweeps: std::array::from_fn(|_| Drift::new(rng)),
            breaths: [[
                Svf::new(breath_hz, 0.541, sample_rate),
                Svf::new(breath_hz, 1.307, sample_rate),
            ]; MAX_TONES],
            glide: [0.0, 0.0, 1.0],
            octaves: [1.0; MAX_TONES],
            voice: None,
            next_voice: 1,
            level: 0.0,
        }
    }

    fn load(&mut self, harmony: &Harmony, register: Option<[f32; 2]>, rng: &mut Rng) {
        self.count = harmony.count;
        self.tones = harmony.tones;
        self.glide = [harmony.root(), harmony.root(), 1.0];
        self.octaves = [1.0; MAX_TONES];
        self.voice = None;
        self.next_voice = 1;
        if let Some([low, high]) = register {
            for index in 1..self.count {
                let frequency = self.tones[index].frequency;
                let mut octave = 1.0;
                while frequency * octave < low {
                    octave *= 2.0;
                }
                while frequency * octave >= high {
                    octave *= 0.5;
                }
                // A tone that lands on another moves an octave within the register.
                let taken = |octave: f32| {
                    (1..index).any(|other| {
                        let placed = self.tones[other].frequency * self.octaves[other];
                        (frequency * octave / placed - 1.0).abs() < 0.01
                    })
                };
                if taken(octave) {
                    if frequency * octave * 2.0 < high {
                        octave *= 2.0;
                    } else if frequency * octave * 0.5 >= low {
                        octave *= 0.5;
                    }
                }
                // With both octaves taken, the tone joins the one already there rather than
                // doubling it at a random phase.
                if let Some(other) = (1..index).find(|&other| {
                    let placed = self.tones[other].frequency * self.octaves[other];
                    (frequency * octave / placed - 1.0).abs() < 0.01
                }) {
                    self.tones[other].level =
                        (self.tones[other].level + self.tones[index].level).min(0.9);
                    self.tones[index].level = 0.0;
                }
                self.octaves[index] = octave;
            }
        }
        for (tone_index, (tone, partials)) in
            harmony.tones().iter().zip(&mut self.partials).enumerate()
        {
            for (index, partial) in partials.iter_mut().enumerate() {
                *partial = Partial::new(rng);
                // The bass stays centred; partials spread wider the higher they are.
                let frequency = tone.frequency * self.octaves[tone_index] * (index + 1) as f32;
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

/// The gain of each tone from the colors of the light: tones whose color shows more than
/// the chord's colors on average come forward, by up to `decibels` at twice the average,
/// the others step back. Grayer light moves them less; the root and tones without a color
/// stay as they are.
struct Voicing<'a> {
    hues: &'a [f32; HUES],
    /// The average share of the chord's colors, or 0 if they do not show.
    average: f32,
    /// Largest change in dB, scaled by how colored the light is.
    decibels: f32,
}

impl<'a> Voicing<'a> {
    fn new(tones: &[Tone], hues: &'a [f32; HUES], decibels: f32) -> Self {
        let mut voicing = Self {
            hues,
            average: 0.0,
            decibels: decibels * hues.iter().sum::<f32>().min(1.0),
        };
        let (mut sum, mut count) = (0.0, 0);
        for hue in tones.iter().filter_map(Self::color) {
            sum += voicing.share(hue);
            count += 1;
        }
        if count > 0 {
            voicing.average = sum / count as f32;
        }
        voicing
    }

    /// The hue of a tone that sounds and has a color.
    fn color(tone: &Tone) -> Option<f32> {
        tone.hue.filter(|_| tone.level > 0.0)
    }

    /// The light's share at `hue`, between the two nearest of its hues.
    fn share(&self, hue: f32) -> f32 {
        let place = hue.rem_euclid(360.0) / 30.0;
        let below = (place as usize).min(HUES - 1);
        let fraction = place - below as f32;
        self.hues[below] * (1.0 - fraction) + self.hues[(below + 1) % HUES] * fraction
    }

    fn gain(&self, tone: &Tone) -> f32 {
        match Self::color(tone) {
            Some(hue) if self.average > 1e-6 => {
                let lean = (self.share(hue) / self.average - 1.0).clamp(-1.0, 1.0);
                10.0_f32.powf(self.decibels * lean / 20.0)
            }
            _ => 1.0,
        }
    }
}

pub struct Bed {
    sample_rate: f32,
    settings: Settings,
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
    chord_gate: f32,
    filter: Option<Filter>,
    /// Scales the low-passed noise to an RMS of 1.
    breath_scale: f32,
}

impl Bed {
    pub fn new(sample_rate: f32, settings: Settings, harmony: &Harmony, mut rng: Rng) -> Self {
        let mut sets = [
            Set::new(sample_rate, settings.breath_hz, &mut rng),
            Set::new(sample_rate, settings.breath_hz, &mut rng),
        ];
        sets[0].load(harmony, settings.register, &mut rng);
        sets[0].level = 1.0;
        Self {
            sample_rate,
            settings,
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
            chord_gate: 0.0,
            filter: None,
            // Uniform noise has a variance of 1/3; the low-pass passes 1.026 × `breath_hz` of
            // its bandwidth of half the sample rate.
            breath_scale: (3.0 * sample_rate / (2.0 * 1.026 * settings.breath_hz)).sqrt(),
        }
    }

    /// Switches to a harmony at once, without a crossfade.
    pub fn load(&mut self, harmony: &Harmony) {
        self.sets[0].load(harmony, self.settings.register, &mut self.rng);
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

    /// Puts a resonant low-pass on the chord tones from the next block on, or takes it off.
    pub fn set_filter(&mut self, filter: Option<Filter>) {
        self.filter = filter;
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

    /// Crossfades to a new harmony.
    pub fn change(&mut self, harmony: &Harmony) {
        let next = 1 - self.current;
        self.sets[next].load(harmony, self.settings.register, &mut self.rng);
        self.sets[next].level = 0.0;
        self.current = next;
        self.fade = 0.0;
    }

    /// Reads the bass's accent and the chord gate from the signals.
    pub fn update_block(&mut self, params: &Params, signals: &Signals) {
        let settings = &self.settings;
        let filter = self.filter;
        self.bass_accent = signals.accent;
        self.chord_gate = signals.gate;
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
        // The height of the light tilts the chord, as an exponent of the frequency.
        let lift = settings.height_tilt * 2.0 * (params.image.height - 0.5) / 6.02;
        let depth = (0.45 + 0.35 * params.activity + 0.15 * warmth).min(0.95);
        let partial_rate = (0.02 + 0.2 * params.flow) * (1.0 + warmth) * block_seconds;
        let tone_rate = (0.01 + 0.08 * params.flow) * block_seconds;
        let sweep_rate = (0.01 + 0.06 * params.flow) * block_seconds;
        // Chord tones dip lower when warm.
        let tone_floor = lerp(0.6, 0.3, warmth);
        self.breath_step =
            (lerp(settings.breath[0], settings.breath[1], warmth) - self.breath) / BLOCK as f32;
        let top = (0.42 * self.sample_rate).min(14_000.0);
        let keep = 1.0 - 0.9 * params.void;
        let masks = coefficient(0.6, self.sample_rate / BLOCK as f32);

        let bass_rolloff = lerp(settings.bass_rolloff[0], settings.bass_rolloff[1], warmth);
        // Of the bass and of the chord.
        let mut power = [0.0; 2];
        for set in &mut self.sets {
            if set.count == 0 {
                continue;
            }
            let [from, to, progress] = &mut set.glide;
            if *progress < 1.0 {
                *progress = (*progress + block_seconds / settings.glide_seconds).min(1.0);
                set.tones[0].frequency = *from * (*to / *from).powf(smooth(*progress));
            }
            let mut voice = (usize::MAX, 1.0);
            if let Some((index, progress)) = &mut set.voice {
                let before = *progress;
                *progress += block_seconds / settings.voice_seconds;
                if before < 1.0 && *progress >= 1.0 {
                    // Silent now: change the octave, towards the middle of the range.
                    let frequency = set.tones[*index].frequency * set.octaves[*index];
                    let down = frequency * 0.5 >= settings.voice_range[0]
                        && (frequency > 400.0 || frequency * 2.0 > settings.voice_range[1]);
                    let moved = frequency * if down { 0.5 } else { 2.0 };
                    // In a register, a tone stays where it is rather than land on another.
                    let taken = settings.register.is_some()
                        && (1..set.count).any(|other| {
                            other != *index
                                && (set.tones[other].frequency * set.octaves[other] / moved - 1.0)
                                    .abs()
                                    < 0.01
                        });
                    if !taken {
                        set.octaves[*index] *= if down { 0.5 } else { 2.0 };
                    }
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
            let voicing = Voicing::new(
                &set.tones[..set.count],
                &params.image.hues,
                settings.voicing,
            );
            for tone_index in 0..set.count {
                let mut tone = set.tones[tone_index];
                tone.frequency *= set.octaves[tone_index];
                let bass = tone_index == 0;
                let tone_level = if bass {
                    1.0
                } else {
                    tone.level * (tone.frequency / 400.0).powf(tilt).clamp(0.4, 2.0)
                } * if tone_index == voice.0 { voice.1 } else { 1.0 }
                    * voicing.gain(&set.tones[tone_index]);
                let tone_motion = tone_floor
                    + (1.0 - tone_floor)
                        * set.tone_drifts[tone_index].advance(
                            tone_rate * set.partials[tone_index][0].drift_rate,
                            &mut self.rng,
                        );
                let sweep = lerp(
                    settings.sweep[0],
                    settings.sweep[1],
                    set.sweeps[tone_index].advance(sweep_rate, &mut self.rng),
                );
                for (index, partial) in set.partials[tone_index].iter_mut().enumerate() {
                    let harmonic = (index + 1) as f32;
                    let detune = if bass || index == 0 {
                        1.0
                    } else {
                        2.0_f32.powf(settings.detune_cents * warmth * partial.detune / 1200.0)
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
                    let (profile, rolloff) = if bass {
                        (
                            lerp(settings.bass_cold[index], settings.bass_warm[index], warmth),
                            bass_rolloff,
                        )
                    } else {
                        (
                            lerp(settings.cold[index], settings.warm[index], warmth),
                            settings.rolloff,
                        )
                    };
                    let ceiling = ((top - frequency) / (0.2 * top)).clamp(0.0, 1.0)
                        / (1.0 + (frequency / rolloff).powi(4)).sqrt();
                    partial.frequency = frequency;
                    let lifted = if bass || lift == 0.0 {
                        1.0
                    } else {
                        (frequency / 400.0).powf(lift).clamp(0.5, 2.0)
                    };
                    partial.strength =
                        tone_level * tone_motion * profile * brightness * motion * ceiling * lifted;
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
                let mut weighted = partial.level * k_weight(partial.frequency);
                if let Some(filter) = filter.filter(|_| tone_index > 0) {
                    let [cutoff, resonance] = filter.reference;
                    weighted *= resonant_lowpass(partial.frequency, cutoff, resonance);
                }
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
            .zip([settings.bass_level, settings.level])
        {
            let target = (level / (power.sqrt() + 1e-6)).min(8.0);
            *normalization +=
                (target - *normalization) * coefficient(0.4, self.sample_rate / BLOCK as f32);
        }

        for set in &mut self.sets {
            for tone_index in 0..set.count {
                let normalization = self.normalization[(tone_index > 0) as usize];
                for (index, partial) in set.partials[tone_index].iter_mut().enumerate() {
                    let top = index as f32 / (PARTIALS - 1) as f32;
                    let accent = if tone_index == 0 {
                        1.0 + self.bass_accent
                            * (settings.bass_accent[0] + settings.bass_accent[1] * top)
                    } else {
                        1.0 - self.chord_gate * lerp(0.6, 1.0, warmth) * top
                    };
                    let mut main = partial.level * normalization * accent;
                    if let Some(filter) = filter.filter(|_| tone_index > 0) {
                        let response =
                            resonant_lowpass(partial.frequency, filter.cutoff, filter.resonance);
                        // Silent partials stop their oscillators.
                        main *= if response < 1e-5 { 0.0 } else { response };
                    }
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

    /// `chord` scales the chord tones; the bass is left alone.
    #[inline]
    pub fn next(&mut self, chord: f32) -> [f32; 2] {
        self.next_parts(chord).mix
    }

    /// Like `next`, with the bass and the chord apart as well.
    #[inline]
    pub fn next_parts(&mut self, chord: f32) -> Parts {
        self.breath += self.breath_step;
        let mut output = [0.0; 2];
        let mut parts = Parts::default();
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
                    parts.bass[0] += tone[0];
                    parts.bass[1] += tone[1];
                    1.0
                } else {
                    let noise = set.breaths[tone_index]
                        .iter_mut()
                        .fold(self.rng.range(-1.0, 1.0), |noise, filter| {
                            filter.lowpass(noise)
                        });
                    let breath = (1.0 + self.breath * self.breath_scale * noise).max(0.0);
                    parts.chord[0] += tone[0] * breath;
                    parts.chord[1] += tone[1] * breath;
                    breath * chord
                };
                output[0] += tone[0] * gain;
                output[1] += tone[1] * gain;
            }
        }
        parts.mix = output;
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::tests::palette;

    fn render(bed: &mut Bed, seconds: f32, rate: f32) {
        let params = Params {
            heat: 0.8,
            flow: 0.5,
            form: 0.0,
            void: 0.5,
            flow_setting: 0.5,
            ..Params::default()
        };
        let signals = Signals::new(Harmony::from_palette(&palette()));
        for frame in 0..(seconds * rate) as usize {
            if frame % BLOCK == 0 {
                bed.update_block(&params, &signals);
            }
            let [left, right] = bed.next(1.0);
            assert!(left.is_finite() && right.is_finite());
        }
    }

    #[test]
    fn colors_of_the_light_lean_their_tones_forward() {
        let tone = |hue| Tone {
            frequency: 200.0,
            level: 0.5,
            hue,
        };
        let tones = [tone(None), tone(Some(30.0)), tone(Some(210.0)), tone(None)];
        let mut hues = [0.0; HUES];
        hues[1] = 1.0;
        let gains = |hues: &[f32; HUES], decibels| {
            let voicing = Voicing::new(&tones, hues, decibels);
            tones.map(|tone| 20.0 * voicing.gain(&tone).log10())
        };
        let decibels = gains(&hues, 3.0);
        assert_eq!(
            (decibels[0], decibels[3]),
            (0.0, 0.0),
            "root and whites stay"
        );
        assert!((decibels[1] - 3.0).abs() < 1e-4, "{decibels:?}");
        assert!((decibels[2] + 3.0).abs() < 1e-4, "{decibels:?}");
        // Grayer light moves them less, gray light not at all.
        hues[1] = 0.5;
        assert!((gains(&hues, 3.0)[1] - 1.5).abs() < 1e-4);
        assert_eq!(gains(&[0.0; HUES], 3.0), [0.0; 4]);
        assert_eq!(gains(&hues, 0.0), [0.0; 4]);
    }

    #[test]
    fn bass_glides_and_chord_tones_change_octave() {
        let rate = 16_000.0;
        let harmony = Harmony::from_palette(&palette());
        let mut bed = Bed::new(rate, Settings::DEFAULT, &harmony, Rng::new(2));
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
        let [low, high] = Settings::DEFAULT.voice_range;
        assert!((low..=high * 2.0).contains(&frequency));
    }

    #[test]
    fn a_register_folds_the_chord_without_doubling_tones() {
        // One pitch class in three octaves, as in Sahara Drift, and a fourth tone.
        let mut harmony = Harmony::from_palette(&palette());
        let root = 36.7;
        for (index, frequency) in [root, 147.0, 294.0, 588.0, 440.0].iter().enumerate() {
            harmony.tones[index].frequency = *frequency;
        }
        harmony.count = 5;
        let settings = Settings {
            register: Some([180.0, 720.0]),
            voice_range: [180.0, 720.0],
            ..Settings::DEFAULT
        };
        let mut bed = Bed::new(16_000.0, settings, &harmony, Rng::new(3));
        for _ in 0..4 {
            let set = &bed.sets[bed.current];
            let sounding: Vec<f32> = (1..set.count)
                .filter(|&index| set.tones[index].level > 0.0)
                .map(|index| set.tones[index].frequency * set.octaves[index])
                .collect();
            for (index, a) in sounding.iter().enumerate() {
                assert!((180.0..720.0).contains(a), "{sounding:?}");
                for b in &sounding[index + 1..] {
                    assert!((a / b - 1.0).abs() > 0.01, "doubled: {sounding:?}");
                }
            }
            // Octave moves never land on another tone either.
            bed.move_voice();
            render(&mut bed, 13.0, 16_000.0);
        }
    }

    #[test]
    fn breath_noise_has_unit_rms() {
        let rate = 48_000.0;
        let harmony = Harmony::from_palette(&palette());
        let mut bed = Bed::new(rate, Settings::DEFAULT, &harmony, Rng::new(1));
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
