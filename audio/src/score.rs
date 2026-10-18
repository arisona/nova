//! The score: an instrument's rhythm as data, which the conductor plays (see the rhythm in
//! `doc/audio_concept.md`).
//!
//! A pattern gives every 16th a weight from 1 to 9, how essential the step is, written as a
//! string: `"9... 5... 7... 5..."` is a kick that plays once per bar, then twice, then four
//! to the floor; `.` rests, spaces only separate the beats. Density, from the rhythm (which
//! Void sets, see `params::RHYTHM`), decides how much of a pattern plays: the steps whose weight clears a threshold, so a pattern grows by
//! adding steps, never by moving them. The weight also sets how hard a step plays.
//!
//! Each part's patterns run from straight (cold) to syncopated (warm); Heat picks where
//! between them it plays, and while the bass is away, and in a fill, it moves to a warmer
//! neighbour. Between neighbours the weights blend, so one pattern turns into the next step
//! by step. A bass line's patterns also carry notes, as intervals above the root.

/// Longest pattern in steps: four bars.
pub const MAX_STEPS: usize = 64;

/// How far the rhythm must cross a threshold before the density changes, either way.
const HYSTERESIS: f32 = 0.02;

/// Velocity of a step of weight 1 and of weight 9.
const VELOCITY: [f32; 2] = [0.64, 1.0];

/// Weights of a cycle of steps, counted from the start of the music, so a pattern of 7 or
/// 12 steps runs against the bar; for a bass line also its notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pattern {
    weights: [u8; MAX_STEPS],
    /// Intervals above the root, 0 for the root itself.
    notes: [u8; MAX_STEPS],
    len: usize,
}

/// Reads a step string: a digit from 1 to 9 per step, `.` for none, spaces ignored.
const fn read(steps: &str) -> ([u8; MAX_STEPS], usize) {
    let bytes = steps.as_bytes();
    let mut digits = [0; MAX_STEPS];
    let (mut index, mut len) = (0, 0);
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if byte == b' ' {
            continue;
        }
        assert!(len < MAX_STEPS, "a pattern has at most four bars");
        match byte {
            b'.' => {}
            b'1'..=b'9' => digits[len] = byte - b'0',
            _ => panic!("steps are digits from 1 to 9 or '.'"),
        }
        len += 1;
    }
    assert!(len > 0, "a pattern has steps");
    (digits, len)
}

/// Frequency ratios of the intervals 1 (the root) to 9 (a ninth), in just intonation, with
/// the minor third and seventh of dub.
const INTERVALS: [f32; 9] = [
    1.0,
    9.0 / 8.0,
    6.0 / 5.0,
    4.0 / 3.0,
    3.0 / 2.0,
    5.0 / 3.0,
    16.0 / 9.0,
    2.0,
    9.0 / 4.0,
];

impl Pattern {
    /// Reads a step string: a digit from 1 to 9 weighs a step, `.` rests, spaces are
    /// ignored.
    pub const fn new(steps: &str) -> Self {
        let (weights, len) = read(steps);
        Self {
            weights,
            notes: [0; MAX_STEPS],
            len,
        }
    }

    /// With notes: `notes` is a step string beside `steps`. For a bass line they are
    /// intervals above the root, `1` the root, `5` the fifth, `8` the octave, and `.` plays
    /// the root; for bells, degrees of the chord, `1` its lowest tone.
    pub const fn with_notes(steps: &str, notes: &str) -> Self {
        let (weights, len) = read(steps);
        let (notes, count) = read(notes);
        assert!(count == len, "as many notes as steps");
        Self {
            weights,
            notes,
            len,
        }
    }

    /// The weight of `step`, counted from the start, 0 for a rest.
    pub fn weight(&self, step: u64) -> u8 {
        self.weights[(step % self.len as u64) as usize]
    }

    /// The digit of the note on `step`, 0 where none is written.
    pub fn digit(&self, step: u64) -> u8 {
        self.notes[(step % self.len as u64) as usize]
    }

    /// The frequency ratio of the note on `step` to the root.
    pub fn note(&self, step: u64) -> f32 {
        INTERVALS[self.digit(step).max(1) as usize - 1]
    }
}

/// The patterns of the kick, the ticks, the stabs or the gate.
#[derive(Clone, Copy, Debug)]
pub struct Patterns {
    /// Rhythm at which the first steps play; an infinite one leaves them out.
    pub from: f32,
    /// In order from straight (cold) to syncopated (warm).
    pub list: &'static [Pattern],
}

impl Patterns {
    pub const NONE: Self = Self {
        from: f32::INFINITY,
        list: &[Pattern::new(".")],
    };

    /// How many weights play at `rhythm`, from 0 (none) to 9 (every step), changing from
    /// `current` only beyond the hysteresis: a step of weight 9 plays from `from` on, a step
    /// of weight 1 only near a rhythm of 1, beyond the full one.
    pub fn density(&self, rhythm: f32, current: u8) -> u8 {
        if !self.from.is_finite() {
            return 0;
        }
        let mut density = 0;
        for level in 1..=9 {
            let threshold = self.from + (1.0 - self.from) * (level - 1) as f32 / 9.0;
            let margin = if level <= current {
                -HYSTERESIS
            } else {
                HYSTERESIS
            };
            if rhythm >= threshold + margin {
                density = level;
            }
        }
        density
    }

    /// Where in the list to play, from 0 (the first pattern) to the last: Heat picks it, and
    /// every move takes it one pattern warmer, turning back cooler at the warmest.
    pub fn position(&self, heat: f32, moves: usize) -> f32 {
        let last = (self.list.len() - 1) as f32;
        if last == 0.0 {
            return 0.0;
        }
        let position = (heat.clamp(0.0, 1.0) * last + moves as f32) % (2.0 * last);
        if position > last {
            2.0 * last - position
        } else {
            position
        }
    }

    /// The weight of `step` at `position`, blended between the neighbouring patterns: the
    /// steps of the next one come in during the first half of the way, strong ones first,
    /// and those of the previous one leave during the second half, weak ones first, so the
    /// blend never leaves a hole.
    pub fn weight(&self, position: f32, step: u64) -> f32 {
        let index = (position as usize).min(self.list.len() - 1);
        let next = (index + 1).min(self.list.len() - 1);
        let blend = position - index as f32;
        let [from, to] = [index, next].map(|index| self.list[index].weight(step) as f32);
        (from * (2.0 - 2.0 * blend).min(1.0)).max(to * (2.0 * blend).min(1.0))
    }

    /// The pattern that `step` at `position` mostly comes from, for its note.
    pub fn source(&self, position: f32, step: u64) -> &Pattern {
        let index = (position as usize).min(self.list.len() - 1);
        let next = (index + 1).min(self.list.len() - 1);
        let blend = position - index as f32;
        let [from, to] = [index, next].map(|index| &self.list[index]);
        let near = (from.weight(step) as f32 * (2.0 - 2.0 * blend).min(1.0))
            >= (to.weight(step) as f32 * (2.0 * blend).min(1.0));
        if near { from } else { to }
    }

    /// The velocity of `step` at `position` and `density`, if it plays.
    pub fn hit(&self, position: f32, density: u8, step: u64) -> Option<f32> {
        let weight = self.weight(position, step);
        (weight > 0.0 && weight + density as f32 >= 10.0).then(|| {
            let [soft, hard] = VELOCITY;
            soft + (hard - soft) * (weight - 1.0).max(0.0) / 8.0
        })
    }
}

/// When and how hard stabs are thrown into the dub delay.
#[derive(Clone, Copy, Debug)]
pub struct Throws {
    /// Every this many bars the last stab is thrown, twice as often while the bass is away;
    /// 0 never.
    pub phrase_bars: u64,
    /// Whether bass and palette changes, and sudden rises in visual activity, throw the
    /// next stab.
    pub sections: bool,
    pub onsets: bool,
    /// Chance per bar of a throw at full Looseness (see the conductor's tide), and as the
    /// light thins out from half lit to dark (see `Features::density`).
    pub loose: f32,
    pub sparse: f32,
    /// Delay boost, and its hold and release in bars: of phrase and loose throws, and of
    /// the bigger section and onset throws.
    pub phrase: [f32; 3],
    pub section: [f32; 3],
}

impl Throws {
    /// For short listening: a throw every phrase.
    pub const SHORT: Self = Self {
        phrase_bars: 4,
        sections: true,
        onsets: true,
        loose: 0.3,
        sparse: 0.25,
        phrase: [0.18, 0.5, 1.0],
        section: [0.28, 1.0, 2.0],
    };

    /// For long listening: rarer gestures that keep the attention without wearing out.
    pub const LONG: Self = Self {
        phrase_bars: 16,
        sections: true,
        onsets: false,
        loose: 0.1,
        sparse: 0.25,
        phrase: [0.15, 0.5, 1.0],
        section: [0.24, 1.0, 2.0],
    };

    pub const NONE: Self = Self {
        phrase_bars: 0,
        sections: false,
        onsets: false,
        loose: 0.0,
        sparse: 0.0,
        phrase: [0.0; 3],
        section: [0.0; 3],
    };
}

/// An instrument's rhythm.
#[derive(Clone, Copy, Debug)]
pub struct Score {
    pub kick: Patterns,
    pub tick: Patterns,
    pub stab: Patterns,
    /// Most stabs a bar plays: beyond it the weakest leave, as if the rhythm were thinner
    /// for the stabs alone.
    pub stabs: usize,
    /// A bass line, with notes.
    pub bass: Patterns,
    /// Other bass lines, each from cold to warm like `bass`: the conductor plays them in
    /// turn, one section of the harmony each, so the low end varies over minutes.
    pub basses: &'static [&'static [Pattern]],
    /// Bells, with degrees of the chord as notes.
    pub bell: Patterns,
    /// The steps on which the chord gate closes.
    pub gate: Patterns,
    /// Time of the dub delay in 16ths, one per section of the harmony in turn: the first
    /// at home, the next while the bass is away, and so on.
    pub echo: &'static [f32],
    /// When the rhythm loosens (see the conductor's Looseness tide): the chance of a quiet
    /// ghost tick on an empty step and of leaving out a tick off the beat. Kick and stabs
    /// stay exact.
    pub ghost: f32,
    pub drop: f32,
    pub throws: Throws,
    /// Every this many bars, the last bar is a fill: every pattern moves one warmer for it;
    /// 0 never.
    pub fills: u64,
    /// Every this many phrases, the last phrase is a break: kick, ticks and bass rest while
    /// every stab is thrown, and the first stab after it is thrown hard as they drop back
    /// in; 0 never.
    pub breaks: u64,
}

/// Velocity of a ghost tick.
pub const GHOST: f32 = 0.3;

#[cfg(test)]
mod tests {
    use super::*;

    const KICK: Patterns = Patterns {
        from: 0.4,
        list: &[Pattern::new("9... 5... 7... 5...")],
    };

    fn steps(patterns: &Patterns, rhythm: f32) -> Vec<u64> {
        let density = patterns.density(rhythm, 0);
        (0..16)
            .filter(|&step| patterns.hit(0.0, density, step).is_some())
            .collect()
    }

    #[test]
    fn patterns_grow_by_adding_steps() {
        assert_eq!(steps(&KICK, 0.3), []);
        assert_eq!(steps(&KICK, 0.45), [0]);
        assert_eq!(steps(&KICK, 0.58), [0, 8]);
        assert_eq!(steps(&KICK, 0.75), [0, 4, 8, 12]);
        assert_eq!(steps(&Patterns::NONE, 1.0), []);
        let pattern = Pattern::new("9.. 1");
        assert_eq!(
            (pattern.weight(0), pattern.weight(3), pattern.weight(4)),
            (9, 1, 9)
        );
        let line = Pattern::with_notes("9.5.", "1.8.");
        assert_eq!((line.note(0), line.note(1), line.note(2)), (1.0, 1.0, 2.0));
    }

    #[test]
    fn density_holds_near_a_threshold() {
        // Weight 7 comes in at a rhythm of 0.533.
        assert_eq!(KICK.density(0.54, 2), 2);
        assert_eq!(KICK.density(0.56, 2), 3);
        assert_eq!(KICK.density(0.52, 3), 3);
        assert_eq!(KICK.density(0.5, 3), 2);
        let hard = KICK.hit(0.0, 9, 0).unwrap();
        let soft = KICK.hit(0.0, 9, 4).unwrap();
        assert!(hard == 1.0 && soft < hard, "{soft}");
    }

    #[test]
    fn heat_and_the_bass_move_through_the_patterns() {
        const PATTERNS: Patterns = Patterns {
            from: 0.0,
            list: &[
                Pattern::new("9..."),
                Pattern::new("5.9."),
                Pattern::new("..9."),
            ],
        };
        let patterns = PATTERNS;
        assert_eq!(patterns.position(0.0, 0), 0.0);
        assert_eq!(patterns.position(0.25, 1), 1.5);
        assert_eq!(patterns.position(1.0, 1), 1.0, "cooler from the warmest");
        assert_eq!(patterns.position(0.75, 2), 0.5);
        assert_eq!(KICK.position(0.7, 1), 0.0, "a single pattern stays");
        // On the way, step 2 comes in, then step 0 steps back.
        assert_eq!(patterns.weight(0.25, 2), 4.5);
        assert_eq!(patterns.weight(0.5, 0), 9.0);
        assert_eq!(patterns.weight(0.5, 2), 9.0);
        assert_eq!(patterns.weight(0.75, 0), 5.0);
        assert_eq!(patterns.weight(1.5, 2), 9.0);
        assert!(patterns.hit(0.25, 4, 2).is_none() && patterns.hit(0.25, 6, 2).is_some());
    }
}
