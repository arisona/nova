use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::content::{Content, FadeSequence, Palette, structure_weight};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

pub struct Threads {
    fades: FadeSequence,
}

impl Threads {
    pub fn new() -> Self {
        Self {
            fades: FadeSequence::new(),
        }
    }
}

impl Content for Threads {
    fn name(&self) -> &str {
        "Threads"
    }

    fn render(
        &mut self,
        state: &RenderState,
        _elapsed: f32,
        delta: f32,
        _prev: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        let palette = Palette::new(state.tone(), state.heat());
        let dim = next.dim();
        next.clear();
        let mut weights = vec![0.0_f32; dim.0 * dim.1];
        let capacity = (dim.0 * dim.1).div_ceil(8).clamp(2, 32);
        for event in self.fades.advance(state, delta, capacity) {
            let mut rng = StdRng::seed_from_u64((event.identity as u64).wrapping_add(811));
            let regular_column =
                (event.identity + dim.0 as i64 / 2).rem_euclid(dim.0 as i64) as f32;
            let regular_row = (event.identity.div_euclid(dim.0 as i64) + dim.1 as i64 / 2)
                .rem_euclid(dim.1 as i64) as f32;
            let random_column = rng.random_range(0..dim.0) as f32;
            let random_row = rng.random_range(0..dim.1) as f32;
            let center_column = regular_column + (random_column - regular_column) * state.form();
            let center_row = regular_row + (random_row - regular_row) * state.form();
            let nearest_distance = ((center_column - center_column.round()).powi(2)
                + (center_row - center_row.round()).powi(2))
            .sqrt();
            let peak = structure_weight(nearest_distance, state.form());
            let palette_position = if event.identity.rem_euclid(5) == 4 {
                0.95
            } else {
                0.5
            };
            let color = palette.sample(palette_position) * event.intensity;
            let first_column = (center_column - 3.0).max(0.0) as usize;
            let last_column = ((center_column + 3.0).ceil() as usize + 1).min(dim.0);
            let first_row = (center_row - 3.0).max(0.0) as usize;
            let last_row = ((center_row + 3.0).ceil() as usize + 1).min(dim.1);
            for column in first_column..last_column {
                for row in first_row..last_row {
                    let distance_squared =
                        (column as f32 - center_column).powi(2) + (row as f32 - center_row).powi(2);
                    let weight = structure_weight(distance_squared.sqrt(), state.form()) / peak;
                    weights[column * dim.1 + row] += event.intensity * weight;
                    next.set(column, row, 0, next.get(column, row, 0) + color * weight);
                }
            }
        }
        for column in 0..dim.0 {
            for row in 0..dim.1 {
                let rgb = next.get(column, row, 0) / weights[column * dim.1 + row].max(1.0);
                for height in 0..dim.2 {
                    next.set(column, row, height, rgb);
                }
            }
        }
    }
}
