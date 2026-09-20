use crate::content::{Content, PaletteCache, advance};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const DRIFT_VOXELS_PER_PHASE: [f32; 3] = [0.45, -0.25, 0.6];
const WASH_SPACING_VOXELS: [f32; 3] = [10.0, 10.0, 14.0];
const SPARSE_COVERAGE_THRESHOLD: f32 = 0.65;
const COVERAGE_EDGE_WIDTH: f32 = 0.25;

fn palette_position(position: [f32; 3], phase: f32) -> f32 {
    let [column, row, height] =
        std::array::from_fn(|axis| position[axis] + phase * DRIFT_VOXELS_PER_PHASE[axis]);
    let gradient = (column * 0.25 + row * 0.18 + height * 0.3).sin();
    0.5 + 0.45 * gradient
}

fn wash_offset(center: [f32; 3], phase: f32) -> [f32; 3] {
    std::array::from_fn(|axis| {
        let amplitude = center[axis] * 0.65;
        amplitude * (phase * DRIFT_VOXELS_PER_PHASE[axis] / amplitude.max(1.0)).sin()
    })
}

fn coverage(position: [f32; 3], offset: [f32; 3], form: f32) -> f32 {
    let shape: f32 = (0..3)
        .map(|axis| {
            let distance = position[axis] + offset[axis];
            0.5 + 0.5 * (distance * std::f32::consts::TAU / WASH_SPACING_VOXELS[axis]).cos()
        })
        .product();
    let threshold =
        SPARSE_COVERAGE_THRESHOLD - form * (SPARSE_COVERAGE_THRESHOLD + COVERAGE_EDGE_WIDTH);
    let edge = ((shape - threshold) / COVERAGE_EDGE_WIDTH).clamp(0.0, 1.0);
    edge * edge * (3.0 - 2.0 * edge)
}

pub struct Field {
    phase: f64,
    palette: PaletteCache,
}

impl Field {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            palette: PaletteCache::default(),
        }
    }
}

impl Content for Field {
    fn name(&self) -> &str {
        "Field"
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
            dim.0.saturating_sub(1) as f32 * 0.5,
            dim.1.saturating_sub(1) as f32 * 0.5,
            dim.2.saturating_sub(1) as f32 * 0.5,
        ];
        let offset = wash_offset(center, phase);
        for column in 0..dim.0 {
            for row in 0..dim.1 {
                for height in 0..dim.2 {
                    let position = if state.heat() == 0.0 {
                        0.5
                    } else {
                        palette_position([column as f32, row as f32, height as f32], phase)
                    };
                    let intensity = coverage(
                        [
                            column as f32 - center[0],
                            row as f32 - center[1],
                            height as f32 - center[2],
                        ],
                        offset,
                        state.form(),
                    );
                    next.set(column, row, height, palette.sample(position) * intensity);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_reveals_fixed_colors_smoothly_even_at_zero_heat() {
        let mut settings = crate::app_state::AppState::default();
        let previous = VoxelImage::new((5, 5, 10));
        let mut image = previous.clone();
        let mut effect = Field::new();
        for heat in [0.0, 0.2, 1.0] {
            settings.set_heat(heat);
            settings.set_form(1.0);
            effect.render(
                &RenderState::from(&settings),
                0.0,
                0.0,
                &previous,
                &mut image,
            );
            let full = image.clone();
            let mut last = previous.clone();
            let mut previous_lit = 0;
            let mut sparse_lit = 0;
            for step in 0..=100 {
                let form = step as f32 / 100.0;
                settings.set_form(form);
                effect.render(
                    &RenderState::from(&settings),
                    0.0,
                    0.0,
                    &previous,
                    &mut image,
                );
                let mut lit = 0;
                let mut peaks = 0;
                for column in 0..5 {
                    for row in 0..5 {
                        for height in 0..10 {
                            let rgb = image.get(column, row, height);
                            let color = full.get(column, row, height);
                            let intensity = coverage(
                                [column as f32 - 2.0, row as f32 - 2.0, height as f32 - 4.5],
                                [0.0; 3],
                                form,
                            );
                            assert!((rgb - color * intensity).length() < 0.00001);
                            if rgb.max_element() > 0.0 {
                                lit += 1;
                            }
                            if (rgb - color).length() < 0.00001 {
                                peaks += 1;
                            }
                            if step > 0 {
                                let change = rgb - last.get(column, row, height);
                                assert!(change.min_element() >= -0.00001);
                                assert!(change.max_element() < 0.06);
                            }
                        }
                    }
                }
                if step == 0 {
                    assert!(lit > 0 && lit < 125);
                    sparse_lit = lit;
                }
                if step == 50 {
                    assert!(lit > sparse_lit && lit < 250);
                }
                if step == 100 {
                    assert_eq!(peaks, 250);
                }
                assert!(peaks > 0);
                assert!(lit >= previous_lit);
                previous_lit = lit;
                last = image.clone();
            }
        }
    }

    #[test]
    fn form_preserves_drift_speed() {
        let position = [2.0, 3.0, 5.0];
        for form in [0.0, 0.5, 1.0] {
            for phase in [0.0, 2.0, 7.0] {
                let shifted = std::array::from_fn(|axis| {
                    position[axis] + phase * DRIFT_VOXELS_PER_PHASE[axis]
                });
                assert_eq!(
                    palette_position(position, phase),
                    palette_position(shifted, 0.0)
                );
                let offset = wash_offset([2.0, 2.0, 4.5], phase);
                let shifted = std::array::from_fn(|axis| position[axis] + offset[axis]);
                assert_eq!(
                    coverage(position, offset, form),
                    coverage(shifted, [0.0; 3], form)
                );
            }
        }
    }

    #[test]
    fn sparse_wash_keeps_a_full_strength_core_throughout_motion() {
        for dim in [(1, 1, 1), (2, 2, 2), (5, 5, 10), (10, 5, 10)] {
            let center = [
                (dim.0 - 1) as f32 * 0.5,
                (dim.1 - 1) as f32 * 0.5,
                (dim.2 - 1) as f32 * 0.5,
            ];
            for frame in 0..1000 {
                let phase = frame as f32 * 0.4;
                let offset = wash_offset(center, phase);
                for form in [0.0, 0.25, 0.5, 1.0] {
                    let mut peak = 0.0_f32;
                    for column in 0..dim.0 {
                        for row in 0..dim.1 {
                            for height in 0..dim.2 {
                                peak = peak.max(coverage(
                                    [
                                        column as f32 - center[0],
                                        row as f32 - center[1],
                                        height as f32 - center[2],
                                    ],
                                    offset,
                                    form,
                                ));
                            }
                        }
                    }
                    assert_eq!(peak, 1.0, "layout={dim:?}, phase={phase}, form={form}");
                }
            }
        }
    }
}
