//! The pluck: plucked strings by Karplus–Strong synthesis, ringing over each other like a
//! guitar's open strings.
//!
//! Every note is a delay line one period long, fed with a burst of noise, the plectrum's
//! stroke, and looped through a low-pass that takes a little of the highs on every period,
//! so the upper harmonics die away first, as on a string. A pickup hears the string at one
//! point, which leaves out the harmonics that have a node there. Cold is a bright, clean
//! bridge pickup and a shorter ring; warm a neck pickup: rounder, darker and longer. Soft
//! notes are darker than hard ones. Form mutes the strings, as a palm resting at the bridge:
//! ringing towards 0, short and thudding towards 1. The stroke rolls off above `ceiling`, so
//! plectrum clicks and high notes never ring piercingly. A string that still rings is picked
//! again rather than joined by another, the plectrum damping it on the way in, so fast
//! strokes on one note make one pulse. A note that has to make room fades out over 3 ms.

use crate::dsp::{Curve, DelayLine, OnePole, Rng, Svf, lerp, smooth};

/// Notes ringing at once.
const NOTES: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// The lowest note in Hz the strings can play.
    pub lowest: f32,
    /// Decay time constant of the fundamental in seconds, when cold and warm, for a note at
    /// 200 Hz: higher notes ring shorter, by the square root of their frequency.
    pub decay: [f32; 2],
    /// Cutoff in Hz of the loop's low-pass when cold and warm: how fast the upper harmonics
    /// die away.
    pub damping: [f32; 2],
    /// Where the pickup hears the string, as a share of its length from the bridge, when
    /// cold (a bridge pickup) and warm (a neck pickup).
    pub pickup: [f32; 2],
    /// Cutoff in Hz of a hard stroke when cold and warm; soft ones are darker.
    pub stroke: [f32; 2],
    /// How long notes ring over Form, as a factor of `decay`: ringing at the layers, muted at
    /// the grain.
    pub ring: Curve,
    /// How far the palm darkens the strings over Form, as a factor of `damping`.
    pub mute: Curve,
    /// The stroke rolls off above this frequency in Hz by 24 dB per octave, whatever the
    /// velocity and Heat.
    pub ceiling: f32,
    /// Share of a ringing string's vibration left when it is picked again.
    pub repick: f32,
}

impl Settings {
    /// A clean electric guitar.
    pub const GUITAR: Self = Self {
        lowest: 55.0,
        decay: [1.0, 1.8],
        damping: [4000.0, 2600.0],
        pickup: [0.1, 0.24],
        stroke: [2500.0, 1500.0],
        ring: Curve::new(&[[0.0, 1.6], [0.5, 1.0], [1.0, 0.1]]),
        mute: Curve::new(&[[0.5, 1.0], [1.0, 0.25]]),
        ceiling: 2500.0,
        repick: 0.35,
    };
}

/// How a string is plucked: velocity, frequency, pan, Heat and Form.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pick {
    pub velocity: f32,
    pub frequency: f32,
    pub pan: f32,
    pub heat: f32,
    pub form: f32,
}

struct Note {
    /// The string's frequency, for picking it again.
    frequency: f32,
    line: DelayLine,
    loop_filter: OnePole,
    stroke: [Svf; 2],
    /// Read delay in samples, so the loop with its filter is one period long, and where the
    /// pickup reads, behind the latest sample.
    delay: f32,
    pickup: f32,
    /// Gain of the loop per period.
    feedback: f32,
    /// Samples of the stroke left, of how many, and its level.
    burst: usize,
    burst_length: usize,
    burst_level: f32,
    /// The fundamental's level, falling as it decays, and its decay per sample: for making
    /// room and for leaving silent notes out.
    level: f32,
    decay: f32,
    pan: [f32; 2],
    /// Gain while making room for the next note, and that note.
    gain: f32,
    pending: Option<Pick>,
}

impl Note {
    fn loudness(&self) -> f32 {
        self.level * self.gain
    }
}

pub struct Pluck {
    sample_rate: f32,
    settings: Settings,
    notes: Vec<Note>,
    rng: Rng,
}

impl Pluck {
    pub fn new(sample_rate: f32, settings: Settings, rng: Rng) -> Self {
        let notes = (0..NOTES)
            .map(|_| Note {
                frequency: 0.0,
                line: DelayLine::new(1.5 / settings.lowest, sample_rate),
                loop_filter: OnePole::default(),
                stroke: [Svf::default(); 2],
                delay: 1.0,
                pickup: 1.0,
                feedback: 0.0,
                burst: 0,
                burst_length: 1,
                burst_level: 0.0,
                level: 0.0,
                decay: 0.0,
                pan: [std::f32::consts::FRAC_1_SQRT_2; 2],
                gain: 1.0,
                pending: None,
            })
            .collect();
        Self {
            sample_rate,
            settings,
            notes,
            rng,
        }
    }

    /// Plucks a string, panned from −1 (left) to 1 (right): the string of that note again if
    /// it still rings, or else in the quietest of the ringing notes' place.
    pub fn trigger(&mut self, pick: Pick) {
        let frequency = pick.frequency.max(self.settings.lowest);
        if let Some(note) = self.notes.iter_mut().find(|note| {
            note.pending.is_none()
                && note.loudness() > 1e-4
                && (note.frequency / frequency - 1.0).abs() < 0.005
        }) {
            Self::start(note, &self.settings, self.sample_rate, pick, false);
            return;
        }
        let note = self
            .notes
            .iter_mut()
            .min_by(|a, b| a.loudness().total_cmp(&b.loudness()))
            .expect("notes");
        if note.loudness() > 1e-4 {
            note.pending = Some(pick);
        } else {
            Self::start(note, &self.settings, self.sample_rate, pick, true);
        }
    }

    /// Starts a note on a silent string, or, if not `fresh`, picks a ringing one again.
    fn start(note: &mut Note, settings: &Settings, sample_rate: f32, pick: Pick, fresh: bool) {
        let Pick {
            velocity,
            frequency,
            pan,
            heat,
            form,
        } = pick;
        let warmth = smooth(heat);
        let frequency = frequency.max(settings.lowest);
        note.frequency = frequency;
        let period = sample_rate / frequency;
        // The loop's low-pass delays the fundamental too: the line is shorter by as much.
        let cutoff =
            lerp(settings.damping[0], settings.damping[1], warmth) * settings.mute.at(form);
        note.loop_filter.set(cutoff.max(frequency), sample_rate);
        // The plectrum damps what still rings.
        let left = if fresh { 0.0 } else { settings.repick };
        note.loop_filter.value *= left;
        note.line.scale(left);
        let pole = (-std::f32::consts::TAU * cutoff.max(frequency) / sample_rate).exp();
        let omega = std::f32::consts::TAU / period;
        let (sin, cos) = omega.sin_cos();
        let lag = (pole * sin).atan2(1.0 - pole * cos) / omega;
        note.delay = (period - lag).max(1.0);
        // The loop's gain at the fundamental: the filter's, made up by the feedback.
        let response = (1.0 - pole) / (1.0 - 2.0 * pole * cos + pole * pole).sqrt();
        let decay = lerp(settings.decay[0], settings.decay[1], warmth)
            * settings.ring.at(form)
            * (200.0 / frequency).sqrt();
        note.feedback = ((-1.0 / (decay * frequency)).exp() / response).min(0.9995);
        let position = lerp(settings.pickup[0], settings.pickup[1], warmth);
        note.pickup = (position * period).max(1.0);
        // Soft strokes are darker, and nothing rings high.
        let stroke = (lerp(settings.stroke[0], settings.stroke[1], warmth)
            * (0.35 + 0.65 * velocity))
            .min(settings.ceiling);
        note.stroke = [
            Svf::new(stroke, 0.541, sample_rate),
            Svf::new(stroke, 1.307, sample_rate),
        ];
        // Low-passed noise loses level; the burst makes up for it.
        note.burst_level = velocity * (0.5 * sample_rate / stroke).sqrt();
        note.burst_length = period as usize;
        note.burst = note.burst_length;
        note.level = velocity.max(note.level * left);
        note.decay = (-1.0 / (decay * sample_rate)).exp();
        let angle = (0.5 + 0.5 * pan.clamp(-1.0, 1.0)) * std::f32::consts::FRAC_PI_2;
        note.pan = [angle.cos(), angle.sin()];
        note.gain = 1.0;
    }

    /// Lets silent filters rest; once per block.
    pub fn settle(&mut self) {
        for note in &mut self.notes {
            for filter in &mut note.stroke {
                filter.settle();
            }
        }
    }

    #[inline]
    pub fn next(&mut self) -> [f32; 2] {
        let fade = 1.0 / (0.003 * self.sample_rate);
        let mut output = [0.0; 2];
        for note in &mut self.notes {
            if note.pending.is_some() {
                note.gain -= fade;
                if note.gain <= 0.0 {
                    let pick = note.pending.take().unwrap_or_default();
                    Self::start(note, &self.settings, self.sample_rate, pick, true);
                }
            }
            if note.level * note.gain < 1e-5 {
                continue;
            }
            // The stroke: a burst of low-passed noise over one period, in a raised sine.
            let mut input = 0.0;
            if note.burst > 0 {
                let age = (note.burst_length - note.burst) as f32 / note.burst_length as f32;
                let window = (std::f32::consts::PI * age).sin();
                let noise = self.rng.range(-1.0, 1.0);
                let [first, second] = &mut note.stroke;
                input = second.lowpass(first.lowpass(noise)) * window * note.burst_level;
                note.burst -= 1;
            }
            let string = input + note.feedback * note.loop_filter.tick(note.line.read(note.delay));
            // The pickup: the string less its image from the bridge, which doubles some
            // harmonics and leaves out others, by √2 on average.
            let sample = (string - note.line.read(note.pickup))
                * std::f32::consts::FRAC_1_SQRT_2
                * note.gain.max(0.0);
            note.line.write(string);
            note.level *= note.decay;
            output[0] += sample * note.pan[0];
            output[1] += sample * note.pan[1];
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 16_000.0;

    fn pick(frequency: f32, form: f32) -> Pick {
        Pick {
            velocity: 1.0,
            frequency,
            pan: 0.0,
            heat: 0.5,
            form,
        }
    }

    /// Peak of the pluck's output over `seconds`, after `skip` seconds.
    fn peak(pluck: &mut Pluck, skip: f32, seconds: f32) -> f32 {
        for _ in 0..(skip * RATE) as usize {
            pluck.next();
        }
        (0..(seconds * RATE) as usize)
            .map(|_| pluck.next()[0].abs())
            .fold(0.0, f32::max)
    }

    /// The pitch of the pluck's output near `frequency`, from the zero crossings of its
    /// fundamental.
    fn pitch(pluck: &mut Pluck, frequency: f32) -> f32 {
        let cutoff = 1.3 * frequency;
        let mut lowpass = [0.541, 1.307, 0.541, 1.307].map(|q| Svf::new(cutoff, q, RATE));
        let mut previous = 0.0;
        let mut crossings = Vec::new();
        for frame in 0..(RATE as usize) {
            let sample = lowpass
                .iter_mut()
                .fold(pluck.next()[0], |value, filter| filter.lowpass(value));
            if frame > (0.2 * RATE) as usize && previous <= 0.0 && sample > 0.0 {
                crossings.push(frame as f32 - sample / (sample - previous));
            }
            previous = sample;
        }
        let span = crossings.last().unwrap() - crossings.first().unwrap();
        (crossings.len() - 1) as f32 * RATE / span
    }

    #[test]
    fn strings_ring_in_tune_and_die_away() {
        for frequency in [82.4, 196.0] {
            let mut pluck = Pluck::new(RATE, Settings::GUITAR, Rng::new(1));
            pluck.trigger(pick(frequency, 0.5));
            let measured = pitch(&mut pluck, frequency);
            let cents = 1200.0 * (measured / frequency).log2();
            assert!(cents.abs() < 5.0, "{frequency} Hz: {measured} Hz");
        }
        let mut pluck = Pluck::new(RATE, Settings::GUITAR, Rng::new(1));
        pluck.trigger(pick(196.0, 0.5));
        let early = peak(&mut pluck, 0.0, 0.1);
        let late = peak(&mut pluck, 3.0, 0.1);
        assert!(early > 0.2 && late < 0.2 * early, "{early}, {late}");
    }

    #[test]
    fn the_palm_mutes_and_notes_make_room() {
        let rings = |form| {
            let mut pluck = Pluck::new(RATE, Settings::GUITAR, Rng::new(1));
            pluck.trigger(pick(196.0, form));
            peak(&mut pluck, 0.6, 0.1)
        };
        assert!(rings(0.0) > rings(0.5) && rings(0.5) > 10.0 * rings(1.0));
        let mut pluck = Pluck::new(RATE, Settings::GUITAR, Rng::new(1));
        for index in 0..30 {
            pluck.trigger(pick(110.0 + 23.0 * index as f32, 0.0));
            for _ in 0..300 {
                let [left, right] = pluck.next();
                assert!(left.abs() < 2.0 * NOTES as f32 && right.is_finite());
            }
        }
    }

    #[test]
    fn strokes_on_one_note_pick_its_string_again() {
        let mut pluck = Pluck::new(RATE, Settings::GUITAR, Rng::new(1));
        let mut loudest = 0.0_f32;
        for _ in 0..40 {
            pluck.trigger(pick(220.0, 0.0));
            loudest = loudest.max(peak(&mut pluck, 0.0, 0.06));
        }
        let sounding = pluck
            .notes
            .iter()
            .filter(|note| note.loudness() > 1e-4)
            .count();
        assert_eq!(sounding, 1);
        assert!(loudest > 0.2 && loudest < 1.5, "{loudest}");
    }
}
