use noise::{NoiseFn, Simplex};

use crate::content::{Content, HEAT_CONTRAST_START, PaletteCache, advance};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const CLOUD_INTENSITY_GAIN: f32 = 2.0;
const SPARSE_POOL_WIDTH_VOXELS: f32 = 1.3;
const COLOR_SHAPE_MIN: f32 = 0.2;
const COLOR_SHAPE_MAX: f32 = 0.75;

fn palette_position(shape: f32, heat: f32) -> f32 {
    let spread = ((heat - HEAT_CONTRAST_START) / (1.0 - HEAT_CONTRAST_START)).clamp(0.0, 1.0);
    let spread = spread * spread * (3.0 - 2.0 * spread);
    let position =
        ((shape - COLOR_SHAPE_MIN) / (COLOR_SHAPE_MAX - COLOR_SHAPE_MIN)).clamp(0.0, 1.0);
    0.5 + spread * (position - 0.5)
}

pub struct Cloud {
    phase: f64,
    noise: Simplex,
    palette: PaletteCache,
}

impl Cloud {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            noise: Simplex::new(0x8240),
            palette: PaletteCache::default(),
        }
    }
}

impl Content for Cloud {
    fn name(&self) -> &str {
        "Cloud"
    }

    fn render(
        &mut self,
        state: &RenderState,
        _elapsed: f32,
        delta: f32,
        _prev: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        let phase = advance(&mut self.phase, state, delta);
        let palette = self.palette.get(state.tone(), state.heat());
        let dim = next.dim();
        let center = [
            dim.0.saturating_sub(1) as f32 * (0.5 + 0.22 * (phase * 0.23).sin()),
            dim.1.saturating_sub(1) as f32 * (0.5 + 0.22 * (phase * 0.19).cos()),
            dim.2.saturating_sub(1) as f32 * (0.5 + 0.25 * (phase * 0.17).sin()),
        ];
        let width = SPARSE_POOL_WIDTH_VOXELS;
        for column in 0..dim.0 {
            for row in 0..dim.1 {
                for height in 0..dim.2 {
                    let distance_squared = (column as f32 - center[0]).powi(2)
                        + (row as f32 - center[1]).powi(2)
                        + (height as f32 - center[2]).powi(2);
                    let pool = (-0.5 * distance_squared / (width * width)).exp();
                    let noise = self.noise.get([
                        column as f64 * 0.18,
                        row as f64 * 0.18,
                        height as f64 * 0.18 - phase as f64 * 0.05,
                        phase as f64 * 0.09,
                    ]) as f32;
                    let organic = (0.5 + noise * 0.65).clamp(0.0, 1.0);
                    let shape = pool * (1.0 - state.form()) + organic * state.form();
                    let intensity = (shape.powf(1.8) * CLOUD_INTENSITY_GAIN).min(1.0);
                    let color = palette.sample(palette_position(shape, state.heat()));
                    next.set(column, row, height, color * intensity);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Palette;

    #[test]
    fn heat_stays_on_tone_then_smoothly_opens_the_palette() {
        for shape in [0.0, 0.2, 0.5, 0.75, 1.0] {
            for heat in [0.0, 0.25, 0.5] {
                assert_eq!(palette_position(shape, heat), 0.5);
            }
            let mut previous_spread = 0.0;
            let mut previous_position = 0.5;
            for step in 0..=100 {
                let position = palette_position(shape, 0.5 + step as f32 * 0.005);
                let spread = (position - 0.5).abs();
                assert!(spread >= previous_spread);
                assert!((position - previous_position).abs() < 0.008);
                previous_spread = spread;
                previous_position = position;
            }
        }
        assert_eq!(palette_position(0.2, 1.0), 0.0);
        assert_eq!(palette_position(0.75, 1.0), 1.0);
    }

    #[test]
    fn sparse_pool_is_smaller_while_full_form_keeps_its_intensity() {
        let mut settings = crate::app_state::AppState::default();
        settings.set_heat(0.5);
        let color = Palette::new(0.0, 0.5).sample(0.5);
        let previous = VoxelImage::new((5, 5, 10));
        let mut image = previous.clone();
        let mut effect = Cloud::new();
        effect.render(
            &RenderState::from(&settings),
            0.0,
            0.0,
            &previous,
            &mut image,
        );
        let sparse = image.clone();
        settings.set_form(1.0);
        effect.render(
            &RenderState::from(&settings),
            0.0,
            0.0,
            &previous,
            &mut image,
        );
        let mut sparse_lit = 0;
        let mut old_lit = 0;
        let mut peak = 0.0_f32;
        for column in 0..5 {
            for row in 0..5 {
                for height in 0..10 {
                    let distance_squared = (column as f32 - 2.0).powi(2)
                        + (row as f32 - 2.88).powi(2)
                        + (height as f32 - 4.5).powi(2);
                    let old_pool = (-0.5 * distance_squared / 2.3_f32.powi(2)).exp();
                    let old_intensity = (old_pool.powf(1.8) * CLOUD_INTENSITY_GAIN).min(1.0);
                    let intensity = sparse.get(column, row, height).length() / color.length();
                    assert!(intensity <= old_intensity + 0.00001);
                    if intensity > 0.1 {
                        sparse_lit += 1;
                    }
                    if old_intensity > 0.1 {
                        old_lit += 1;
                    }
                    peak = peak.max(intensity);
                    let noise = effect.noise.get([
                        column as f64 * 0.18,
                        row as f64 * 0.18,
                        height as f64 * 0.18,
                        0.0,
                    ]) as f32;
                    let organic = (0.5 + noise * 0.65).clamp(0.0, 1.0);
                    let expected = color * (organic.powf(1.8) * CLOUD_INTENSITY_GAIN).min(1.0);
                    assert!((image.get(column, row, height) - expected).length() < 0.00001);
                }
            }
        }
        assert!(sparse_lit > 0 && sparse_lit * 2 < old_lit);
        assert!((peak - 1.0).abs() < 0.00001);
    }

    #[test]
    fn intensity_gain_lifts_midtones_and_preserves_highlight_colors() {
        let mut settings = crate::app_state::AppState::default();
        settings.set_heat(1.0);
        let previous = VoxelImage::new((1, 1, 1));
        let mut image = previous.clone();
        for tone in [0.0, 0.25, 0.5, 0.75] {
            settings.set_tone(tone);
            let palette = Palette::new(tone, 1.0);
            for form in [0.0, 1.0] {
                settings.set_form(form);
                let mut effect = Cloud::new();
                effect.render(
                    &RenderState::from(&settings),
                    0.0,
                    0.0,
                    &previous,
                    &mut image,
                );
                let shape: f32 = if form == 0.0 { 1.0 } else { 0.5 };
                let color = palette.sample(palette_position(shape, 1.0));
                let base_intensity = shape.powf(1.8);
                let expected = color * (base_intensity * CLOUD_INTENSITY_GAIN).min(1.0);
                let actual = image.get(0, 0, 0);
                assert!((actual - expected).length() < 0.00001);
                assert!(actual.is_finite());
                assert!(actual.min_element() >= 0.0 && actual.max_element() <= 1.0);
                if form == 0.0 {
                    assert!((actual - color).length() < 0.00001);
                } else {
                    assert!(actual.length() > (color * base_intensity).length() * 1.5);
                }
            }
        }
    }
}
