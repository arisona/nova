//! The conductor: the musical time and harmony all instruments share.
//!
//! Tempo follows the Flow slider (without its tide, so it does not wobble) on a grid of 16ths
//! in bars of 16, swung when warm. The harmony moves in sections: at every boundary the bass
//! moves between home and the palette's own pitch classes, halfway one chord tone changes its
//! octave; a new palette starts the progression again, once the previous crossfade is done.
//! The audio tides run from the host's tide clock. Everything goes out as signals once per
//! block, and the grid as `Now` once per sample.

use nova_core::Controls;
use nova_core::tides::LOOSENESS;

use crate::BLOCK;
use crate::dsp::{coefficient, lerp, smooth};
use crate::harmony::{CROSSFADE_SECONDS, Harmony, Palette};
use crate::params::Params;
use crate::signals::{BAR, Change, Now, Signals};

/// Tempo range in beats per minute, over the Flow slider.
const SLOWEST: f32 = 80.0;
const FASTEST: f32 = 110.0;

/// Swing when cold and warm: where the second 16th of a pair starts, as a share of the pair.
const SWING: [f32; 2] = [0.5, 0.57];

/// Extra swing when the rhythm is loose.
const LOOSE_SWING: f32 = 0.025;

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
}

impl Conductor {
    pub fn new(sample_rate: f32, harmony: Harmony) -> Self {
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
        self.looseness = LOOSENESS.offset(clock).max(0.0);

        if controls.palette != self.palette && self.crossfade >= 1.0 {
            self.palette = controls.palette;
            self.home = Harmony::from_palette(&palettes[self.palette]);
            self.harmony = self.home;
            self.section = 0.0;
            self.progression = 0;
            self.crossfade = 0.0;
            signals.change = Some(Change::Palette);
        }
        if self.crossfade < 1.0 {
            self.crossfade = (self.crossfade + block_seconds / CROSSFADE_SECONDS).min(1.0);
        }

        // Sections of the harmony: the bass moves at every boundary, the voicing halfway.
        let before = self.section;
        let seconds = lerp(SECTION_SECONDS[0], SECTION_SECONDS[1], params.flow);
        self.section += (block_seconds / seconds) as f64;
        if self.section.floor() > before.floor() {
            self.progression += 1;
            self.harmony = self.home.with_bass(self.home.bass(self.progression));
            signals.change = Some(Change::Bass);
        } else if (self.section - 0.5).floor() > (before - 0.5).floor() {
            signals.change = Some(Change::Voice);
        }

        signals.tempo = self.tempo;
        signals.harmony = self.harmony;
        signals.away = !self.progression.is_multiple_of(2);
        signals.looseness = self.looseness;
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
        let swing = self.swing as f64;
        let progress = if self.odd_played {
            (self.pair_phase - swing) / (1.0 - swing)
        } else {
            self.pair_phase / swing
        } as f32;
        Now { step, progress }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::tests::palette;

    const RATE: f32 = 8000.0;

    /// Runs the conductor for `seconds`; returns the steps and the changes it announced.
    fn run(
        conductor: &mut Conductor,
        params: &Params,
        controls: &Controls,
        seconds: f32,
    ) -> (Vec<(usize, u64)>, Vec<Change>) {
        let palettes = [palette(), Palette::new(&palette().colors()[..2])];
        let mut signals = Signals::new(conductor.harmony);
        let (mut steps, mut changes) = (Vec::new(), Vec::new());
        for frame in 0..(seconds * RATE) as usize {
            if frame % BLOCK == 0 {
                conductor.update_block(params, controls, 0.0, &palettes, &mut signals);
                changes.extend(signals.change);
            }
            if let Some(step) = conductor.next().step {
                steps.push((frame, step));
            }
        }
        (steps, changes)
    }

    #[test]
    fn steps_follow_the_tempo_and_swing_when_warm() {
        for (heat, swung) in [(0.0, false), (1.0, true)] {
            let mut conductor = Conductor::new(RATE, Harmony::from_palette(&palette()));
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
            // 110 BPM: a pair of 16ths every 60 / 110 / 2 s.
            let pair = 60.0 / 110.0 / 2.0 * RATE;
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
        let mut conductor = Conductor::new(RATE, harmony);
        let params = Params {
            flow: 1.0,
            ..Params::default()
        };
        let (_, changes) = run(&mut conductor, &params, &Controls::default(), 70.0);
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
}
