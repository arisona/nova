use glam::Vec3;
use palette::convert::IntoColorUnclamped;
use palette::{Oklch, Srgb};

use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

pub mod cloud;
pub mod field;
pub mod layers;
pub mod threads;

const FLOW_SPEED_MULTIPLIER: f32 = 10.0;
const MAX_FRAME_DELTA_SECONDS: f32 = 0.25;
const FADE_EVENTS_PER_PHASE_UNIT: f32 = 0.18;
const FADE_SECONDS: f64 = 1.875;
const FORM_EDGE_SPREAD_VOXELS: f32 = 0.65;

const HEAT_FULL_COLOR: f32 = 0.5;
const HEAT_CONTRAST_START: f32 = 0.5;
const PALETTE_HUE_SPREAD_DEGREES: f32 = 70.0;
const PALETTE_ACCENT_OFFSET_DEGREES: f32 = 110.0;

pub trait Content {
    fn name(&self) -> &str;
    fn render(
        &mut self,
        state: &RenderState,
        elapsed: f32,
        delta: f32,
        prev: &VoxelImage,
        next: &mut VoxelImage,
    );
}

pub fn get_all_content() -> Vec<Box<dyn Content>> {
    vec![
        Box::new(field::Field::new()),
        Box::new(layers::Layers::new()),
        Box::new(threads::Threads::new()),
        Box::new(cloud::Cloud::new()),
    ]
}

pub fn get_all_content_names() -> Vec<String> {
    assert_ne!(get_all_content().len(), 0);
    get_all_content()
        .iter()
        .map(|c| c.name().to_string())
        .collect()
}

pub fn advance(phase: &mut f64, state: &RenderState, delta: f32) -> f32 {
    if state.should_reset() {
        *phase = 0.0;
    } else {
        *phase += (FLOW_SPEED_MULTIPLIER * state.flow() * delta.clamp(0.0, MAX_FRAME_DELTA_SECONDS))
            as f64;
    }
    *phase as f32
}

pub struct Palette {
    colors: [Vec3; 64],
}

impl Palette {
    pub fn new(tone: f32, heat: f32) -> Self {
        let colorfulness = 1.0 - (1.0 - (heat / HEAT_FULL_COLOR).clamp(0.0, 1.0)).powi(3);
        let contrast = ((heat - HEAT_CONTRAST_START) / (1.0 - HEAT_CONTRAST_START)).clamp(0.0, 1.0);
        let contrast = contrast * contrast * (3.0 - 2.0 * contrast);
        let colors = std::array::from_fn(|index| {
            let position = index as f32 / 63.0;
            let accent = ((position - 0.8) / 0.2).clamp(0.0, 1.0);
            let accent = accent * accent * (3.0 - 2.0 * accent);
            let hue = tone * 360.0
                + contrast
                    * ((position - 0.5) * PALETTE_HUE_SPREAD_DEGREES
                        + accent * PALETTE_ACCENT_OFFSET_DEGREES);
            let lightness = 0.68 + (position - 0.5) * 0.16 * contrast;
            let convert = |chroma| {
                let rgb: Srgb<f32> = Oklch::new(lightness, chroma, hue).into_color_unclamped();
                Vec3::new(rgb.red, rgb.green, rgb.blue)
            };
            let mut low = 0.0;
            let mut high = 0.5;
            for _ in 0..12 {
                let middle = (low + high) * 0.5;
                let rgb = convert(middle);
                if rgb.min_element() >= 0.0 && rgb.max_element() <= 1.0 {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            convert(low * (0.12 + 0.86 * colorfulness)).clamp(Vec3::ZERO, Vec3::ONE)
        });
        Self { colors }
    }

    pub fn sample(&self, position: f32) -> Vec3 {
        let position = position.clamp(0.0, 1.0) * 63.0;
        let index = (position as usize).min(62);
        self.colors[index].lerp(self.colors[index + 1], position - index as f32)
    }
}

pub struct FadeEvent {
    pub identity: i64,
    pub intensity: f32,
}

struct TimedFade {
    identity: i64,
    started: f64,
    peak_cycle: Option<f64>,
    released: Option<f64>,
}

pub struct FadeSequence {
    phase: f64,
    seconds: f64,
    events: Vec<TimedFade>,
}

impl FadeSequence {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            seconds: 0.0,
            events: vec![TimedFade {
                identity: 0,
                started: -FADE_SECONDS,
                peak_cycle: Some(0.0),
                released: None,
            }],
        }
    }

    pub fn advance(&mut self, state: &RenderState, delta: f32, capacity: usize) -> Vec<FadeEvent> {
        if state.should_reset() {
            *self = Self::new();
        } else if state.flow() > 0.0 && delta > 0.0 {
            let delta = delta.clamp(0.0, MAX_FRAME_DELTA_SECONDS) as f64;
            let previous_cycle = self.phase * FADE_EVENTS_PER_PHASE_UNIT as f64;
            advance(&mut self.phase, state, delta as f32);
            let cycle = self.phase * FADE_EVENTS_PER_PHASE_UNIT as f64;
            let previous_seconds = self.seconds;
            self.seconds += delta;
            for identity in (previous_cycle.floor() as i64 + 1)..=cycle.floor() as i64 {
                let started = previous_seconds
                    + delta * (identity as f64 - previous_cycle) / (cycle - previous_cycle);
                self.events.push(TimedFade {
                    identity,
                    started,
                    peak_cycle: None,
                    released: None,
                });
            }
            let extra_hold = state.form() as f64 * capacity.saturating_sub(1) as f64;
            for event in &mut self.events {
                if event.peak_cycle.is_none() && self.seconds >= event.started + FADE_SECONDS {
                    let fraction =
                        ((event.started + FADE_SECONDS - previous_seconds) / delta).clamp(0.0, 1.0);
                    event.peak_cycle = Some(previous_cycle + fraction * (cycle - previous_cycle));
                }
                let Some(peak_cycle) = event.peak_cycle else {
                    continue;
                };
                let release_cycle = peak_cycle.max(event.identity as f64 + 1.0) + extra_hold;
                if event.released.is_none() && cycle >= release_cycle {
                    let handoff = previous_seconds
                        + delta
                            * ((release_cycle - previous_cycle) / (cycle - previous_cycle))
                                .clamp(0.0, 1.0);
                    let released = handoff.max(event.started + FADE_SECONDS);
                    if released <= self.seconds {
                        event.released = Some(released);
                    }
                }
            }
            self.events.retain(|event| {
                event
                    .released
                    .is_none_or(|released| self.seconds - released < FADE_SECONDS)
            });
        }
        self.events
            .iter()
            .map(|event| {
                let progress = if let Some(released) = event.released {
                    1.0 - ((self.seconds - released) / FADE_SECONDS).clamp(0.0, 1.0)
                } else {
                    ((self.seconds - event.started) / FADE_SECONDS).clamp(0.0, 1.0)
                };
                FadeEvent {
                    identity: event.identity,
                    intensity: (progress * std::f64::consts::FRAC_PI_2).sin().powi(2) as f32,
                }
            })
            .collect()
    }
}

pub fn structure_weight(distance: f32, form: f32) -> f32 {
    (1.0 - distance.abs() / (1.0 + FORM_EDGE_SPREAD_VOXELS * form))
        .max(0.0)
        .powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette::IntoColor;

    #[test]
    fn overlapping_structures_preserve_monochromatic_palette_colors() {
        let previous = VoxelImage::new((5, 5, 10));
        let mut image = previous.clone();
        let mut settings = crate::app_state::AppState::default();
        settings.set_flow(1.0);
        settings.set_form(1.0);
        for tone in [0.0, 0.25, 0.5, 0.75] {
            settings.set_tone(tone);
            for heat in [0.0, 0.4, 0.5] {
                settings.set_heat(heat);
                let state = RenderState::from(&settings);
                let expected = Palette::new(tone, heat).sample(0.5);
                for mut effect in [
                    Box::new(layers::Layers::new()) as Box<dyn Content>,
                    Box::new(threads::Threads::new()) as Box<dyn Content>,
                ] {
                    for frame in 0..500 {
                        effect.render(&state, frame as f32 * 0.04, 0.04, &previous, &mut image);
                        for rgb in pixels(&image) {
                            let intensity = rgb.max_element() / expected.max_element();
                            assert!(
                                intensity <= 1.00001,
                                "{} overlap overbrightens",
                                effect.name()
                            );
                            assert!(
                                (rgb - expected * intensity).length() < 0.00001,
                                "{} overlap shifts Tone at frame {frame}",
                                effect.name()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn palette_contrast_starts_above_the_heat_midpoint() {
        for tone in [0.0, 0.25, 0.5, 0.75] {
            for heat in [0.0, 0.25, 0.4, 0.5] {
                let palette = Palette::new(tone, heat);
                for color in palette.colors {
                    assert_eq!(color, palette.colors[0]);
                }
            }
            let midpoint = Palette::new(tone, 0.5);
            let just_above = Palette::new(tone, 0.501);
            for (before, after) in midpoint.colors.iter().zip(just_above.colors.iter()) {
                assert!((*before - *after).length() < 0.001);
            }
            let hot = Palette::new(tone, 1.0);
            assert!((hot.sample(0.95) - hot.sample(0.5)).length() > 0.1);
        }
    }

    #[test]
    fn flow_uses_configured_speed_without_phase_jumps() {
        let mut settings = crate::app_state::AppState::default();
        let mut phase = 2.0;
        let mut expected = 2.0;
        for flow in [0.0, 0.25, 1.0, 0.5, 0.0] {
            settings.set_flow(flow);
            let state = RenderState::from(&settings);
            assert!((advance(&mut phase, &state, 0.0) - expected).abs() < 0.00001);
            expected += FLOW_SPEED_MULTIPLIER * flow * 0.2;
            assert!((advance(&mut phase, &state, 0.2) - expected).abs() < 0.00001);
        }
        let mut state = RenderState::from(&settings);
        state.set_reset(true);
        assert_eq!(advance(&mut phase, &state, 0.2), 0.0);
    }

    #[test]
    fn heat_reaches_rich_color_early_and_keeps_it_above_midpoint() {
        for step in 0..12 {
            let tone = step as f32 / 12.0;
            let midpoint_chroma = |heat| {
                let rgb = Palette::new(tone, heat).sample(0.5);
                let color: Oklch = Srgb::new(rgb.x, rgb.y, rgb.z).into_color();
                color.chroma
            };
            let peak = midpoint_chroma(0.5);
            assert!(
                midpoint_chroma(0.0) < peak * 0.15,
                "tone={tone}: quiet={}, peak={peak}",
                midpoint_chroma(0.0)
            );
            assert!(midpoint_chroma(0.2) > peak * 0.78);
            assert!(midpoint_chroma(0.35) > peak * 0.96);
            assert!((midpoint_chroma(1.0) - peak).abs() < 0.005);
            let restrained = Palette::new(tone, 0.5);
            let emphatic = Palette::new(tone, 1.0);
            assert!(
                (emphatic.sample(0.95) - emphatic.sample(0.5)).length()
                    > (restrained.sample(0.95) - restrained.sample(0.5)).length()
            );
            let below = Palette::new(tone, 0.4999);
            let above = Palette::new(tone, 0.5001);
            assert!((below.sample(0.9) - above.sample(0.9)).length() < 0.005);
        }
    }

    #[test]
    fn palette_is_finite_in_gamut_and_wraps_tone() {
        for tone in 0..=12 {
            for heat in [0.0, 0.5, 1.0] {
                let palette = Palette::new(tone as f32 / 12.0, heat);
                for step in 0..=100 {
                    let rgb = palette.sample(step as f32 / 100.0);
                    assert!(rgb.is_finite());
                    assert!(rgb.min_element() >= 0.0 && rgb.max_element() <= 1.0);
                }
            }
        }
        for heat in [0.0, 1.0] {
            let start = Palette::new(0.0, heat);
            let end = Palette::new(1.0, heat);
            for step in 0..=100 {
                let position = step as f32 / 100.0;
                assert!((start.sample(position) - end.sample(position)).length() < 0.002);
            }
        }
    }

    #[test]
    fn test_get_all_content_names() {
        assert_eq!(
            get_all_content_names(),
            ["Field", "Layers", "Threads", "Cloud"]
        );
    }

    #[test]
    fn form_adds_overlap_density_even_at_maximum_flow() {
        for capacity in [2, 3, 4] {
            for flow in [0.25, 0.5, 1.0] {
                let mut totals = Vec::new();
                for form in [0.0, 0.5, 1.0] {
                    let mut settings = crate::app_state::AppState::default();
                    settings.set_flow(flow);
                    settings.set_form(form);
                    let state = RenderState::from(&settings);
                    let mut sequence = FadeSequence::new();
                    let mut total = 0.0;
                    for frame in 0..1000 {
                        let events = sequence.advance(&state, 0.04, capacity);
                        if frame >= 500 {
                            total += events.iter().map(|event| event.intensity).sum::<f32>();
                        }
                    }
                    totals.push(total / 500.0);
                }
                assert!(
                    totals[1] > totals[0] + 0.35,
                    "capacity={capacity}, flow={flow}: {totals:?}"
                );
                assert!(
                    totals[2] > totals[1] + 0.35,
                    "capacity={capacity}, flow={flow}: {totals:?}"
                );
            }
        }
    }

    #[test]
    fn fades_stay_slow_at_maximum_flow() {
        let mut settings = crate::app_state::AppState::default();
        settings.set_flow(1.0);
        let state = RenderState::from(&settings);
        for capacity in [2, 3, 4, 32] {
            for form in [0.0, 0.5, 1.0] {
                settings.set_form(form);
                let state = RenderState::from(&settings);
                let mut sequence = FadeSequence::new();
                let mut previous = sequence.advance(&state, 0.0, capacity);
                for _frame in 0..250 {
                    let current = sequence.advance(&state, 0.04, capacity);
                    for event in previous.iter().chain(current.iter()) {
                        let intensity = |events: &[FadeEvent]| {
                            events
                                .iter()
                                .find(|candidate| candidate.identity == event.identity)
                                .map_or(0.0, |candidate| candidate.intensity)
                        };
                        assert!((intensity(&current) - intensity(&previous)).abs() < 0.035);
                    }
                    previous = current;
                }
            }
        }
        let mut sequence = FadeSequence::new();
        for _frame in 0..25 {
            sequence.advance(&state, 0.04, 3);
        }
        let events = sequence.advance(&state, 0.0, 3);
        let outgoing = events.iter().find(|event| event.identity == 0).unwrap();
        assert!(outgoing.intensity > 0.4);
    }

    #[test]
    fn cycling_tracks_flow_while_fades_use_active_seconds() {
        let mut newest = Vec::new();
        for flow in [0.25, 1.0] {
            let mut settings = crate::app_state::AppState::default();
            settings.set_flow(flow);
            let state = RenderState::from(&settings);
            let mut sequence = FadeSequence::new();
            for _frame in 0..250 {
                sequence.advance(&state, 0.04, 3);
            }
            newest.push(sequence.advance(&state, 0.0, 3).last().unwrap().identity);
        }
        assert_eq!(newest[0], 4);
        assert!((17..=18).contains(&newest[1]));

        let mut settings = crate::app_state::AppState::default();
        settings.set_flow(1.0);
        let mut sequence = FadeSequence::new();
        for _frame in 0..25 {
            sequence.advance(&RenderState::from(&settings), 0.04, 3);
        }
        let started = sequence
            .events
            .iter()
            .find(|event| event.identity == 1)
            .unwrap()
            .started;
        let snapshot = |events: Vec<FadeEvent>| {
            events
                .into_iter()
                .map(|event| (event.identity, event.intensity))
                .collect::<Vec<_>>()
        };
        let before = snapshot(sequence.advance(&RenderState::from(&settings), 0.0, 3));
        settings.set_flow(0.1);
        assert_eq!(
            before,
            snapshot(sequence.advance(&RenderState::from(&settings), 0.0, 3))
        );
        for _frame in 0..25 {
            let events = sequence.advance(&RenderState::from(&settings), 0.04, 3);
            let event = events.iter().find(|event| event.identity == 1).unwrap();
            let age = sequence.seconds - started;
            let expected = (age / FADE_SECONDS * std::f64::consts::FRAC_PI_2)
                .sin()
                .powi(2) as f32;
            assert!((event.intensity - expected).abs() < 0.00001);
        }
        let before = snapshot(sequence.advance(&RenderState::from(&settings), 0.0, 3));
        settings.set_flow(0.0);
        assert_eq!(
            before,
            snapshot(sequence.advance(&RenderState::from(&settings), 0.2, 3))
        );
        settings.set_flow(0.5);
        assert_eq!(
            before,
            snapshot(sequence.advance(&RenderState::from(&settings), 0.0, 3))
        );
    }

    #[test]
    fn sparse_fades_overlap_at_full_strength_without_dark_handoffs() {
        for capacity in [2, 3, 4, 32] {
            for flow in [0.05, 0.25, 1.0] {
                let mut settings = crate::app_state::AppState::default();
                settings.set_flow(flow);
                let state = RenderState::from(&settings);
                let mut sequence = FadeSequence::new();
                assert_eq!(sequence.advance(&state, 0.0, capacity)[0].intensity, 1.0);
                let mut saw_overlap = false;
                for _frame in 0..1000 {
                    let events = sequence.advance(&state, 0.04, capacity);
                    saw_overlap |= events.len() > 1;
                    assert!(events.iter().map(|event| event.intensity).sum::<f32>() >= 0.99);
                    assert!(
                        events
                            .iter()
                            .all(|event| (0.0..=1.0).contains(&event.intensity))
                    );
                    assert!(events.len() <= capacity + 8);
                }
                assert!(saw_overlap);
            }
        }
    }

    #[test]
    fn sparse_geometry_reaches_full_palette_intensity_across_layouts() {
        for dim in [(1, 1, 1), (5, 5, 10), (10, 10, 10)] {
            let mut settings = crate::app_state::AppState::default();
            settings.set_tone(0.6);
            settings.set_heat(0.5);
            let state = RenderState::from(&settings);
            let expected = Palette::new(state.tone(), state.heat()).sample(0.5);
            let previous = VoxelImage::new(dim);
            let mut image = previous.clone();
            for (mut effect, count) in [
                (
                    Box::new(layers::Layers::new()) as Box<dyn Content>,
                    dim.0 * dim.1,
                ),
                (Box::new(threads::Threads::new()) as Box<dyn Content>, dim.2),
            ] {
                effect.render(&state, 0.0, 0.0, &previous, &mut image);
                let lit: Vec<_> = pixels(&image)
                    .into_iter()
                    .filter(|rgb| *rgb != Vec3::ZERO)
                    .collect();
                assert_eq!(
                    lit.len(),
                    count,
                    "{} must light only one structure at its peak",
                    effect.name()
                );
                assert!(lit.iter().all(|rgb| (*rgb - expected).length() < 0.00001));
            }
        }
    }

    #[test]
    fn sparse_layers_and_threads_remain_lit_through_handoffs() {
        let mut settings = crate::app_state::AppState::default();
        settings.set_heat(0.0);
        settings.set_flow(1.0);
        let state = RenderState::from(&settings);
        let peak = Palette::new(state.tone(), state.heat())
            .sample(0.5)
            .max_element();
        for dim in [(1, 1, 1), (5, 5, 10), (10, 5, 10)] {
            let previous = VoxelImage::new(dim);
            let mut image = previous.clone();
            for (mut effect, structure_size) in [
                (
                    Box::new(layers::Layers::new()) as Box<dyn Content>,
                    dim.0 * dim.1,
                ),
                (Box::new(threads::Threads::new()) as Box<dyn Content>, dim.2),
            ] {
                for frame in 0..160 {
                    effect.render(&state, frame as f32 * 0.04, 0.04, &previous, &mut image);
                    let lit: Vec<_> = pixels(&image)
                        .into_iter()
                        .filter(|rgb| rgb.max_element() > 0.0)
                        .collect();
                    assert!(!lit.is_empty() && lit.len() <= 8 * structure_size);
                    assert!(
                        lit.iter().any(|rgb| rgb.max_element() >= peak * 0.249),
                        "{} handoff frame={frame}",
                        effect.name()
                    );
                }
            }
        }
    }

    fn pixels(image: &VoxelImage) -> Vec<Vec3> {
        let dim = image.dim();
        (0..dim.0)
            .flat_map(|column| {
                (0..dim.1).flat_map(move |row| {
                    (0..dim.2).map(move |height| image.get(column, row, height))
                })
            })
            .collect()
    }

    #[test]
    fn form_changes_sparse_compositions_continuously() {
        let mut settings = crate::app_state::AppState::default();
        settings.set_heat(0.8);
        let previous = VoxelImage::new((10, 10, 10));
        let mut image = previous.clone();
        for mut effect in [
            Box::new(layers::Layers::new()) as Box<dyn Content>,
            Box::new(threads::Threads::new()) as Box<dyn Content>,
        ] {
            for step in 0..=200 {
                let form = step as f32 / 200.0;
                settings.set_form(form);
                effect.render(
                    &RenderState::from(&settings),
                    0.0,
                    0.0,
                    &previous,
                    &mut image,
                );
                let before = pixels(&image);
                settings.set_form((form + 0.0001).min(1.0));
                effect.render(
                    &RenderState::from(&settings),
                    0.0,
                    0.0,
                    &previous,
                    &mut image,
                );
                assert!(
                    before
                        .iter()
                        .zip(pixels(&image))
                        .all(|(before, after)| (*before - after).length() < 0.01),
                    "{} form={form}",
                    effect.name()
                );
            }
        }
    }

    #[test]
    fn effects_are_finite_freeze_at_zero_flow_and_reset() {
        for dim in [(1, 1, 1), (5, 5, 10), (10, 5, 10)] {
            for heat in [0.0, 0.5, 1.0] {
                for form in [0.0, 0.5, 1.0] {
                    let mut settings = crate::app_state::AppState::default();
                    settings.set_heat(heat);
                    settings.set_form(form);
                    let mut state = RenderState::from(&settings);
                    let previous = VoxelImage::new(dim);
                    let mut image = VoxelImage::new(dim);
                    for mut effect in get_all_content() {
                        image.fill(Vec3::splat(f32::NAN));
                        state.set_reset(true);
                        effect.render(&state, 0.0, 0.04, &previous, &mut image);
                        let initial = pixels(&image);
                        assert!(initial.iter().all(|rgb| rgb.is_finite()
                            && rgb.min_element() >= 0.0
                            && rgb.max_element() <= 1.0));
                        assert!(initial.iter().any(|rgb| rgb.max_element() > 0.0));
                        state.set_reset(false);
                        effect.render(&state, 10.0, 0.2, &previous, &mut image);
                        assert_eq!(initial, pixels(&image), "{} must freeze", effect.name());
                        settings.set_flow(1.0);
                        let mut moving = RenderState::from(&settings);
                        for frame in 0..20 {
                            effect.render(&moving, frame as f32 * 0.1, 0.1, &previous, &mut image);
                        }
                        if heat > 0.0 && dim == (5, 5, 10) {
                            assert_ne!(initial, pixels(&image), "{} must move", effect.name());
                        }
                        moving.set_reset(true);
                        effect.render(&moving, 100.0, 0.1, &previous, &mut image);
                        assert_eq!(initial, pixels(&image), "{} must reset", effect.name());
                    }
                }
            }
        }
    }

    #[test]
    fn families_preserve_their_spatial_identity() {
        let mut settings = crate::app_state::AppState::default();
        settings.set_form(1.0);
        let previous = VoxelImage::new((5, 5, 10));
        let mut image = previous.clone();
        field::Field::new().render(
            &RenderState::from(&settings),
            0.0,
            0.0,
            &previous,
            &mut image,
        );
        assert!(
            pixels(&image)
                .iter()
                .all(|pixel| *pixel == image.get(0, 0, 0))
        );
        settings.set_heat(0.8);
        settings.set_form(0.7);
        let state = RenderState::from(&settings);
        layers::Layers::new().render(&state, 0.0, 0.0, &previous, &mut image);
        assert!(
            pixels(&image)
                .iter()
                .any(|pixel| *pixel != image.get(0, 0, 0))
        );
        for column in 0..5 {
            for row in 0..5 {
                for height in 0..10 {
                    assert_eq!(image.get(column, row, height), image.get(0, 0, height));
                }
            }
        }
        threads::Threads::new().render(&state, 0.0, 0.0, &previous, &mut image);
        assert!(
            pixels(&image)
                .iter()
                .any(|pixel| *pixel != image.get(0, 0, 0))
        );
        for column in 0..5 {
            for row in 0..5 {
                for height in 0..10 {
                    assert_eq!(image.get(column, row, height), image.get(column, row, 0));
                }
            }
        }
    }

    #[test]
    fn artistic_controls_change_each_family_without_advancing_time() {
        let mut settings = crate::app_state::AppState::default();
        settings.set_heat(0.6);
        let previous = VoxelImage::new((5, 5, 10));
        let mut image = previous.clone();
        for mut effect in get_all_content() {
            let state = RenderState::from(&settings);
            effect.render(&state, 0.0, 0.0, &previous, &mut image);
            let baseline = pixels(&image);
            settings.set_tone(0.5);
            effect.render(
                &RenderState::from(&settings),
                0.0,
                0.0,
                &previous,
                &mut image,
            );
            assert_ne!(baseline, pixels(&image), "{} tone", effect.name());
            settings.set_tone(0.0);
            settings.set_heat(1.0);
            effect.render(
                &RenderState::from(&settings),
                0.0,
                0.0,
                &previous,
                &mut image,
            );
            assert_ne!(baseline, pixels(&image), "{} heat", effect.name());
            settings.set_heat(0.6);
            settings.set_form(1.0);
            effect.render(
                &RenderState::from(&settings),
                0.0,
                0.0,
                &previous,
                &mut image,
            );
            assert_ne!(baseline, pixels(&image), "{} form", effect.name());
            settings.set_form(0.0);
        }
    }

    #[test]
    #[ignore = "writes a visual contact sheet and prints local render timings"]
    fn content_preview_and_timings() {
        let width = 920;
        let height = 2520;
        let mut bitmap = vec![10_u8; width * height * 3];
        for (row, (heat, form)) in [
            (0.0, 0.0),
            (0.25, 0.0),
            (0.5, 0.0),
            (0.75, 0.0),
            (1.0, 0.0),
            (1.0, 0.5),
            (1.0, 1.0),
            (0.0, 0.5),
            (0.0, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            let mut settings = crate::app_state::AppState::default();
            settings.set_tone(0.56);
            settings.set_heat(heat);
            settings.set_form(form);
            settings.set_flow(1.0);
            let state = RenderState::from(&settings);
            let previous = VoxelImage::new((5, 5, 10));
            let mut image = previous.clone();
            for (column, mut effect) in get_all_content().into_iter().enumerate() {
                effect.render(&state, 0.0, 0.0, &previous, &mut image);
                for voxel_column in 0..5 {
                    for voxel_row in 0..5 {
                        for voxel_height in 0..10 {
                            let center_horizontal = column as i32 * 230
                                + 115
                                + (voxel_column as i32 - voxel_row as i32) * 18;
                            let center_vertical = row as i32 * 280
                                + 20
                                + (voxel_column + voxel_row) as i32 * 8
                                + voxel_height as i32 * 20;
                            let rgb = image.get(voxel_column, voxel_row, voxel_height);
                            for offset_horizontal in -6..=6 {
                                for offset_vertical in -6..=6 {
                                    if offset_horizontal * offset_horizontal
                                        + offset_vertical * offset_vertical
                                        > 36
                                    {
                                        continue;
                                    }
                                    let pixel_horizontal =
                                        (center_horizontal + offset_horizontal) as usize;
                                    let pixel_vertical =
                                        (center_vertical + offset_vertical) as usize;
                                    let index = (pixel_vertical * width + pixel_horizontal) * 3;
                                    bitmap[index] = (rgb.x.clamp(0.0, 1.0) * 255.0) as u8;
                                    bitmap[index + 1] = (rgb.y.clamp(0.0, 1.0) * 255.0) as u8;
                                    bitmap[index + 2] = (rgb.z.clamp(0.0, 1.0) * 255.0) as u8;
                                }
                            }
                        }
                    }
                }
            }
        }
        let path = std::env::temp_dir().join("nova-content-preview.ppm");
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        bytes.extend(bitmap);
        std::fs::write(&path, bytes).unwrap();
        println!(
            "Preview: {} (columns: Field, Layers, Threads, Cloud; rows: Heat 0/.25/.5/.75/1 at Form 0, then Heat 1 at Form .5/1, then Heat 0 at Form .5/1)",
            path.display()
        );
        let mut settings = crate::app_state::AppState::default();
        settings.set_heat(0.75);
        settings.set_form(1.0);
        settings.set_flow(1.0);
        let state = RenderState::from(&settings);
        for dim in [(5, 5, 10), (10, 10, 10), (50, 50, 10)] {
            let previous = VoxelImage::new(dim);
            let mut image = previous.clone();
            for mut effect in get_all_content() {
                let start = std::time::Instant::now();
                for frame in 0..60 {
                    effect.render(&state, frame as f32 * 0.04, 0.04, &previous, &mut image);
                    std::hint::black_box(&image);
                }
                println!(
                    "{} {dim:?}: {:.3} ms/frame",
                    effect.name(),
                    start.elapsed().as_secs_f64() * 1000.0 / 60.0
                );
            }
        }
    }
}
