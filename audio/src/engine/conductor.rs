//! The conductor: the musical time, the harmony and the rhythm all instruments share.
//!
//! Tempo follows the Flow slider (without its tide, so it does not wobble) on a grid of 16ths
//! in bars of 16 and phrases of 4 bars, swung when warm. The harmony moves in sections, which
//! end on a phrase start: at every boundary the bass moves between home and the palette's own
//! pitch classes, halfway one chord tone changes its octave; a new palette starts the
//! progression again, once the previous crossfade is done. The audio tides run from the
//! host's tide clock.
//!
//! The conductor plays the instrument's score (see `score.rs`): at every bar start it sets
//! the density of each pattern from the rhythm (from Void), and on every step it hands out
//! the hits, with ghost ticks and dropped ones when the rhythm loosens or dissolves, fills and breaks, and decides which stabs
//! are thrown.
//! Everything goes out as signals once per block, and the grid with its hits as `Now` once
//! per sample.

use nova_core::Controls;
use nova_core::tides::LOOSENESS;

use crate::BLOCK;
use crate::dsp::{Rng, coefficient, lerp, smooth, smoothstep};
use crate::harmony::{CROSSFADE_SECONDS, Harmony, Palette};
use crate::params::Params;
use crate::score::{GHOST, Patterns, Score};
use crate::signals::{BAR, Change, Hits, Now, PHRASE, Signals};

/// Tempo range in beats per minute, over the Flow slider.
const SLOWEST: f32 = 40.0;
const FASTEST: f32 = 120.0;

/// Swing when cold and warm: where the second 16th of a pair starts, as a share of the pair.
const SWING: [f32; 2] = [0.5, 0.57];

/// Extra swing when the rhythm is loose.
const LOOSE_SWING: f32 = 0.025;

/// How loose the rhythm is once it has dissolved, on top of the Looseness tide.
const LOOSE_DISSOLVED: f32 = 0.6;

/// Seconds per section of the harmony at Flow 0 and 1. The bass moves at every boundary,
/// one chord tone changes its octave halfway.
const SECTION_SECONDS: [f32; 2] = [120.0, 30.0];

pub struct Conductor {
    sample_rate: f32,
    primed: bool,
    tempo: f32,
    /// Progress through the current pair of 16ths, from 0 to 1, and whether its second
    /// half has started.
    pair_phase: f64,
    odd_played: bool,
    /// The latest 16th, counted from the start.
    step: u64,
    /// Swing and Heat for the current bar.
    swing: f32,
    heat: f32,
    looseness: f32,
    palette: usize,
    /// The palette's harmony, over the bass at home, and the current one.
    home: Harmony,
    harmony: Harmony,
    /// Progress of the crossfade to a new palette, from 0 to 1.
    crossfade: f32,
    /// Sections of the harmony so far, and steps of the bass progression.
    section: f64,
    progression: usize,
    /// A change of the harmony waiting for the next phrase start, and one to announce with
    /// the next block.
    due: Option<Change>,
    announce: Option<Change>,
    score: Score,
    /// The score to play from the next phrase start, when the instrument changes.
    next_score: Option<Score>,
    rng: Rng,
    /// The rhythm, Flow as set and how sparse the light is, from the latest block.
    rhythm: f32,
    flow_setting: f32,
    sparse: f32,
    /// The current bar, and whether the bass was away at its start.
    bar: u64,
    bar_away: bool,
    /// Density of kick, ticks, stabs, bass, gate and bells for the current bar, and where
    /// they play in their patterns.
    density: [u8; 6],
    position: [f32; 6],
    /// Whether the current bar is in a break: kick, ticks and bass rest.
    breaking: bool,
    /// Stabs in the current bar, the steps of the bar they play on, and the step of the
    /// last one.
    stabs: usize,
    stab_steps: u16,
    last_stab: u64,
    /// A throw waiting for the next stab.
    armed: Option<[f32; 3]>,
}

impl Conductor {
    /// Plays `score`; `seed` varies the details of loose rhythm.
    pub fn new(sample_rate: f32, harmony: Harmony, score: Score, seed: u64) -> Self {
        Self {
            sample_rate,
            primed: false,
            tempo: lerp(SLOWEST, FASTEST, 0.5),
            pair_phase: 0.999,
            odd_played: true,
            step: u64::MAX - 1,
            swing: 0.0,
            heat: 0.5,
            looseness: 0.0,
            palette: 0,
            home: harmony,
            harmony,
            crossfade: 1.0,
            section: 0.0,
            progression: 0,
            due: None,
            announce: None,
            score,
            next_score: None,
            rng: Rng::new(seed),
            rhythm: 0.5,
            flow_setting: 0.5,
            sparse: 0.0,
            bar: 0,
            bar_away: false,
            density: [0; 6],
            position: [0.0; 6],
            breaking: false,
            stabs: 0,
            stab_steps: 0,
            last_stab: 0,
            armed: None,
        }
    }

    /// Once per block, before the instruments: the tempo, the tides and the harmony.
    /// `controls` are as set, unsmoothed, and `clock` is the tide clock.
    pub fn update_block(
        &mut self,
        params: &Params,
        controls: &Controls,
        clock: f64,
        palettes: &[Palette],
        signals: &mut Signals,
    ) {
        let block_seconds = BLOCK as f32 / self.sample_rate;
        signals.change = None;
        if !self.primed {
            // The first block takes everything as it is.
            self.primed = true;
            self.tempo = lerp(SLOWEST, FASTEST, params.flow_setting);
            self.palette = controls.palette;
            self.home = Harmony::from_palette(&palettes[self.palette]);
            self.harmony = self.home;
            signals.change = Some(Change::Load);
        }
        // Exponential approach with a slew limit, so tempo changes stay gentle.
        let target = lerp(SLOWEST, FASTEST, params.flow_setting);
        let change = (target - self.tempo) * coefficient(2.5, 1.0 / block_seconds);
        self.tempo += change.clamp(-8.0 * block_seconds, 8.0 * block_seconds);
        self.heat = params.heat;
        self.looseness =
            (LOOSENESS.offset(clock).max(0.0) + LOOSE_DISSOLVED * params.dissolve).min(1.0);
        self.rhythm = params.rhythm;
        self.flow_setting = params.flow_setting;
        self.sparse = params.light.sparseness();
        let throws = self.score.throws;
        if signals.onset && throws.onsets && self.density[2] >= 2 {
            self.armed = Some(throws.section);
        }

        if controls.palette != self.palette && self.crossfade >= 1.0 {
            self.palette = controls.palette;
            self.home = Harmony::from_palette(&palettes[self.palette]);
            self.harmony = self.home;
            self.section = 0.0;
            self.progression = 0;
            self.due = None;
            self.announce = None;
            self.crossfade = 0.0;
            signals.change = Some(Change::Palette);
            self.arm_section();
        }
        if self.crossfade < 1.0 {
            self.crossfade = (self.crossfade + block_seconds / CROSSFADE_SECONDS).min(1.0);
        }

        // Sections of the harmony: the bass moves at every boundary, the voicing halfway,
        // both on the next phrase start.
        let before = self.section;
        let seconds = lerp(SECTION_SECONDS[0], SECTION_SECONDS[1], params.flow);
        self.section += (block_seconds / seconds) as f64;
        if self.section.floor() > before.floor() {
            self.due = Some(Change::Bass);
        } else if (self.section - 0.5).floor() > (before - 0.5).floor() {
            self.due = self.due.or(Some(Change::Voice));
        }
        if let Some(change) = self.announce.take() {
            signals.change = signals.change.or(Some(change));
        }

        signals.tempo = self.tempo;
        signals.harmony = self.harmony;
        signals.away = !self.progression.is_multiple_of(2);
        signals.looseness = self.looseness;
        signals.stabs = self.stabs;
        let echo = self.score.echo;
        signals.echo = echo[self.progression % echo.len()];
    }

    /// Plays `score` from the next phrase start, as one instrument mixes over to another.
    pub fn set_score(&mut self, score: Score) {
        self.next_score = Some(score);
    }

    /// Throws the next stab, for a change of bass or palette.
    fn arm_section(&mut self) {
        if self.score.throws.sections {
            self.armed = Some(self.score.throws.section);
        }
    }

    /// At a bar start: a harmony change due on a phrase start, and the density of every
    /// pattern, which holds for the bar.
    fn new_bar(&mut self, step: u64) {
        self.bar = step / BAR as u64;
        if self.bar.is_multiple_of(PHRASE as u64)
            && let Some(score) = self.next_score.take()
        {
            self.score = score;
        }
        if self.bar.is_multiple_of(PHRASE as u64)
            && let Some(change) = self.due.take()
        {
            if change == Change::Bass {
                self.progression += 1;
                self.harmony = self.home.with_bass(self.home.bass(self.progression));
                self.arm_section();
            }
            self.announce = Some(change);
        }
        self.bar_away = !self.progression.is_multiple_of(2);
        // Like the pulse, the rhythm rests at Flow 0.
        let resting = smoothstep(0.0, 0.15, self.flow_setting) < 0.01;
        let score = self.score;
        // The last bar of every few is a fill, one pattern warmer.
        let fill = score.fills > 0 && (self.bar + 1).is_multiple_of(score.fills);
        let moves = self.bar_away as usize + fill as usize;
        let parts = [
            score.kick,
            score.tick,
            score.stab,
            self.bass(),
            score.gate,
            score.bell,
        ];
        for (part, patterns) in parts.iter().enumerate() {
            self.density[part] = if resting {
                0
            } else {
                patterns.density(self.rhythm, self.density[part])
            };
            self.position[part] = patterns.position(self.heat, moves);
        }
        // The last phrase of every few is a break, if the kick plays; the stab after it is
        // thrown hard as kick, ticks and bass drop back in.
        let phrase = self.bar / PHRASE as u64;
        let was_breaking = self.breaking;
        self.breaking =
            score.breaks > 0 && (phrase + 1).is_multiple_of(score.breaks) && self.density[0] > 0;
        if was_breaking && !self.breaking && score.throws.sections {
            self.armed = Some(score.throws.section);
        }
        // The bar's strongest stabs, as many as the score lets play, the earlier first among
        // equals.
        let mut stabs = [(0.0_f32, 0_u64); BAR];
        let mut count = 0;
        for step in step..step + BAR as u64 {
            if self.plays(&score.stab, 2, step) {
                stabs[count] = (score.stab.weight(self.position[2], step), step);
                count += 1;
            }
        }
        let stabs = &mut stabs[..count];
        stabs.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let kept = &stabs[..count.min(score.stabs)];
        self.stab_steps = kept
            .iter()
            .fold(0, |steps, &(_, step)| steps | 1 << (step % BAR as u64));
        self.stabs = kept.len();
        self.last_stab = kept.iter().map(|&(_, step)| step).max().unwrap_or(0);

        // Throws begin once the stabs play more than their first step.
        let throws = score.throws;
        if self.density[2] < 2 {
            self.armed = None;
        } else if self.armed.is_none()
            && self.rng.unit() < throws.loose * self.looseness + throws.sparse * self.sparse
        {
            self.armed = Some(throws.phrase);
        }
    }

    /// The bass line of the current section: the score's lines in turn.
    fn bass(&self) -> Patterns {
        let score = &self.score;
        match self.progression % (1 + score.basses.len()) {
            0 => score.bass,
            line => Patterns {
                list: score.basses[line - 1],
                ..score.bass
            },
        }
    }

    fn plays(&self, patterns: &Patterns, part: usize, step: u64) -> bool {
        patterns
            .hit(self.position[part], self.density[part], step)
            .is_some()
    }

    /// The hits of `step`.
    fn play(&mut self, step: u64) -> Hits {
        let bar_step = step % BAR as u64;
        if bar_step == 0 {
            self.new_bar(step);
        }
        let score = self.score;
        let line = self.bass();
        let [mut kick, mut tick, stab, mut bass, gate, bell] = self.density;
        if self.breaking {
            (kick, tick, bass) = (0, 0, 0);
        }
        let [at_kick, at_tick, at_stab, at_bass, at_gate, at_bell] = self.position;
        let away = self.bar_away;
        let mut hits = Hits {
            kick: score.kick.hit(at_kick, kick, step).unwrap_or(0.0),
            bass: line.hit(at_bass, bass, step).unwrap_or(0.0),
            note: line.source(at_bass, step).note(step),
            bell: score.bell.hit(at_bell, bell, step).unwrap_or(0.0),
            degree: score.bell.source(at_bell, step).digit(step).max(1),
            gate: score.gate.hit(at_gate, gate, step).unwrap_or(0.0),
            ..Hits::default()
        };
        // Loose: a few ticks off the beat drop out, a few empty steps get a ghost tick.
        let loose = self.looseness;
        hits.tick = match score.tick.hit(at_tick, tick, step) {
            Some(velocity) => {
                if bar_step.is_multiple_of(4) || self.rng.unit() >= score.drop * loose {
                    velocity
                } else {
                    0.0
                }
            }
            None if tick > 0 && loose > 0.0 && self.rng.unit() < score.ghost * loose => GHOST,
            None => 0.0,
        };
        let kept = self.stab_steps & 1 << bar_step != 0;
        if let Some(velocity) = score.stab.hit(at_stab, stab, step).filter(|_| kept) {
            hits.stab = velocity;
            if stab >= 2 {
                // The last stab of every few bars, twice as often while the bass is away.
                let bars = score.throws.phrase_bars;
                let period = if away { (bars / 2).max(1) } else { bars };
                let phrase = self.breaking
                    || bars > 0 && step == self.last_stab && (self.bar + 1).is_multiple_of(period);
                hits.throw = self.armed.take().or(phrase.then_some(score.throws.phrase));
            }
        }
        hits
    }

    /// Holds the bass away from home or at home, for tests.
    #[cfg(test)]
    pub(crate) fn hold_away(&mut self, away: bool) {
        self.progression = away as usize;
    }

    /// Once per sample: the grid. The second 16th of a pair starts late when the rhythm
    /// swings; swing changes only at bar boundaries.
    #[inline]
    pub fn next(&mut self) -> Now {
        let mut step = None;
        self.pair_phase += (self.tempo / 30.0 / self.sample_rate) as f64;
        if self.pair_phase >= 1.0 {
            self.pair_phase -= 1.0;
            self.step = self.step.wrapping_add(2);
            self.odd_played = false;
            if self.step.is_multiple_of(BAR as u64) {
                self.swing =
                    lerp(SWING[0], SWING[1], smooth(self.heat)) + LOOSE_SWING * self.looseness;
            }
            step = Some(self.step);
        }
        if step.is_none() && !self.odd_played && self.pair_phase >= self.swing as f64 {
            self.odd_played = true;
            step = Some(self.step.wrapping_add(1));
        }
        let hits = step.map_or(Hits::default(), |step| self.play(step));
        let swing = self.swing as f64;
        let progress = if self.odd_played {
            (self.pair_phase - swing) / (1.0 - swing)
        } else {
            self.pair_phase / swing
        } as f32;
        Now {
            step,
            progress,
            hits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::tests::palette;
    use crate::instruments::strata::SCORE;

    const RATE: f32 = 8000.0;

    fn conductor(seed: u64) -> Conductor {
        Conductor::new(RATE, Harmony::from_palette(&palette()), SCORE, seed)
    }

    /// What a run of the conductor played: the frame of every step, the changes it announced
    /// with the latest step before each, and the ticks.
    struct Run {
        steps: Vec<(usize, u64)>,
        changes: Vec<(Change, u64)>,
        ticks: usize,
    }

    /// Runs the conductor for `seconds` at the tide `clock`.
    fn run_at(
        conductor: &mut Conductor,
        params: &Params,
        controls: &Controls,
        clock: f64,
        seconds: f32,
    ) -> Run {
        let palettes = [palette(), Palette::new(&palette().colors()[..2])];
        let mut signals = Signals::new(conductor.harmony);
        let (mut steps, mut changes, mut ticks) = (Vec::new(), Vec::new(), 0);
        for frame in 0..(seconds * RATE) as usize {
            if frame % BLOCK == 0 {
                conductor.update_block(params, controls, clock, &palettes, &mut signals);
                let latest = steps.last().map_or(0, |&(_, step)| step);
                changes.extend(signals.change.map(|change| (change, latest)));
            }
            let now = conductor.next();
            if let Some(step) = now.step {
                steps.push((frame, step));
                ticks += (now.hits.tick > 0.0) as usize;
            }
        }
        Run {
            steps,
            changes,
            ticks,
        }
    }

    fn run(
        conductor: &mut Conductor,
        params: &Params,
        controls: &Controls,
        seconds: f32,
    ) -> (Vec<(usize, u64)>, Vec<Change>) {
        let run = run_at(conductor, params, controls, 0.0, seconds);
        let changes = run.changes.into_iter().map(|(change, _)| change).collect();
        (run.steps, changes)
    }

    #[test]
    fn steps_follow_the_tempo_and_swing_when_warm() {
        for (heat, swung) in [(0.0, false), (1.0, true)] {
            let mut conductor = conductor(1);
            let params = Params {
                heat,
                flow_setting: 1.0,
                ..Params::default()
            };
            let (steps, _) = run(&mut conductor, &params, &Controls::default(), 4.0);
            assert!(
                steps
                    .iter()
                    .enumerate()
                    .all(|(i, &(_, step))| step == i as u64)
            );
            // 120 BPM: a pair of 16ths every 60 / 120 / 2 s.
            let pair = 60.0 / 120.0 / 2.0 * RATE;
            let (start, end) = (steps[0].0, steps[steps.len() - 1].0);
            let pairs = (end - start) as f32 / pair;
            assert!(
                (pairs - (steps.len() - 1) as f32 / 2.0).abs() < 0.1,
                "{pairs}"
            );
            let first = (steps[1].0 - steps[0].0) as f32 / pair;
            assert_eq!(first > 0.53, swung, "second 16th at {first} of the pair");
        }
    }

    #[test]
    fn sections_move_the_bass_and_a_palette_change_starts_again() {
        let harmony = Harmony::from_palette(&palette());
        let mut conductor = conductor(1);
        let params = Params {
            flow: 1.0,
            ..Params::default()
        };
        let changes = run_at(&mut conductor, &params, &Controls::default(), 0.0, 80.0).changes;
        // Harmony changes wait for a phrase start.
        for &(change, step) in &changes[1..] {
            assert!(
                step.is_multiple_of((PHRASE * BAR) as u64),
                "{change:?} at {step}"
            );
        }
        let changes: Vec<Change> = changes.into_iter().map(|(change, _)| change).collect();
        assert_eq!(
            changes,
            [
                Change::Load,
                Change::Voice,
                Change::Bass,
                Change::Voice,
                Change::Bass,
            ]
        );
        assert_eq!(conductor.harmony.root(), harmony.root(), "home again");

        let controls = Controls {
            palette: 1,
            ..Controls::default()
        };
        let (_, changes) = run(&mut conductor, &params, &controls, 1.0);
        assert_eq!(changes, [Change::Palette]);
        assert_eq!(conductor.progression, 0);
        assert!(conductor.crossfade > 0.0 && conductor.crossfade < 1.0);
    }

    #[test]
    fn rhythm_is_exact_unless_the_tide_loosens_it() {
        // Clocks where the Looseness tide is at rest, and where it is high.
        let clocks: Vec<f64> = (0..400).map(|second| second as f64).collect();
        let strict = *clocks
            .iter()
            .find(|&&clock| LOOSENESS.offset(clock) < 0.0)
            .unwrap();
        let loose = *clocks
            .iter()
            .find(|&&clock| LOOSENESS.offset(clock) > 0.8)
            .unwrap();
        let params = Params {
            rhythm: 0.7,
            ..Params::default()
        };
        let ticks = |clock, seed| {
            run_at(
                &mut conductor(seed),
                &params,
                &Controls::default(),
                clock,
                40.0,
            )
            .ticks
        };
        let exact = ticks(strict, 1);
        assert!(exact > 50, "{exact}");
        assert_eq!(exact, ticks(strict, 2), "no chance involved while strict");
        let loose = [ticks(loose, 1), ticks(loose, 2)];
        assert!(
            loose.iter().all(|&count| count != exact),
            "{exact} vs {loose:?}"
        );
        assert!(loose[0] != loose[1]);
    }

    #[test]
    fn bars_keep_their_strongest_stabs_and_sections_turn_the_bass_line() {
        use crate::instruments::fathom;
        let palettes = [palette()];
        let harmony = Harmony::from_palette(&palettes[0]);
        let params = Params {
            rhythm: 0.8,
            ..Params::default()
        };
        let mut signals = Signals::new(harmony);
        for most in [1, 2] {
            let score = Score {
                stabs: most,
                ..fathom::SCORE
            };
            let mut conductor = Conductor::new(RATE, harmony, score, 1);
            let mut per_bar = [0; 8];
            for frame in 0.. {
                if frame % BLOCK == 0 {
                    conductor.update_block(
                        &params,
                        &Controls::default(),
                        0.0,
                        &palettes,
                        &mut signals,
                    );
                }
                let now = conductor.next();
                let Some(step) = now.step else { continue };
                let bar = (step / BAR as u64) as usize;
                if bar == per_bar.len() {
                    break;
                }
                per_bar[bar] += (now.hits.stab > 0.0) as usize;
            }
            assert!(per_bar.iter().all(|&count| count == most), "{per_bar:?}");
        }
        // Each section plays the next line.
        let mut conductor = Conductor::new(RATE, harmony, fathom::SCORE, 1);
        let lines = 1 + fathom::SCORE.basses.len();
        let firsts: Vec<_> = (0..lines + 1)
            .map(|progression| {
                conductor.progression = progression;
                conductor.bass().list[0]
            })
            .collect();
        assert!(firsts[0] != firsts[1] && firsts[0] == firsts[lines]);
    }

    #[test]
    fn breaks_rest_the_low_end_and_drop_back_in_with_a_throw() {
        use crate::instruments::fathom;
        let palettes = [palette()];
        // Every stab of the patterns, not only the strongest of each bar.
        let score = Score {
            stabs: BAR,
            ..fathom::SCORE
        };
        let mut conductor = Conductor::new(RATE, Harmony::from_palette(&palettes[0]), score, 1);
        let params = Params {
            rhythm: 0.8,
            ..Params::default()
        };
        let mut signals = Signals::new(conductor.harmony);
        // Kicks and bass notes per phrase, and the phrase of every thrown stab.
        let mut low = [0; 10];
        let mut throws = Vec::new();
        let phrase_steps = (PHRASE * BAR) as u64;
        for frame in 0.. {
            if frame % BLOCK == 0 {
                conductor.update_block(&params, &Controls::default(), 0.0, &palettes, &mut signals);
            }
            let now = conductor.next();
            let Some(step) = now.step else { continue };
            let phrase = (step / phrase_steps) as usize;
            if phrase == low.len() {
                break;
            }
            low[phrase] += (now.hits.kick > 0.0) as usize + (now.hits.bass > 0.0) as usize;
            if now.hits.throw.is_some() {
                throws.push(phrase);
            }
        }
        let breaks = fathom::SCORE.breaks as usize;
        for (phrase, &count) in low.iter().enumerate() {
            assert_eq!(count == 0, phrase + 1 == breaks, "phrase {phrase}: {count}");
        }
        // Every stab in the break is thrown, and the first one after it.
        assert!(
            throws
                .iter()
                .filter(|&&phrase| phrase == breaks - 1)
                .count()
                > 4
        );
        assert!(throws.contains(&breaks), "{throws:?}");
    }
}
