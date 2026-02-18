use glam::Vec3;
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::content::{Content, FadeSequence, Palette, structure_weight};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

pub struct Layers {
    fades: FadeSequence,
}

impl Layers {
    pub fn new() -> Self {
        Self {
            fades: FadeSequence::new(),
        }
    }
}

impl Content for Layers {
    fn name(&self) -> &str {
        "Layers"
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
        let slices: Vec<_> = self
            .fades
            .advance(state, delta, 3)
            .into_iter()
            .map(|event| {
                let mut rng = StdRng::seed_from_u64((event.identity as u64).wrapping_add(1709));
                let regular = (event.identity + dim.2 as i64 / 2).rem_euclid(dim.2 as i64) as f32;
                let random = rng.random_range(0..dim.2) as f32;
                let position = regular + (random - regular) * state.form();
                let palette_position = if event.identity.rem_euclid(5) == 4 {
                    0.95
                } else {
                    0.5
                };
                (position, palette.sample(palette_position), event.intensity)
            })
            .collect();
        for height in 0..dim.2 {
            let mut rgb = Vec3::ZERO;
            let mut weight = 0.0;
            for (position, color, intensity) in &slices {
                let peak = structure_weight(position - position.round(), state.form());
                let contribution =
                    intensity * structure_weight(height as f32 - position, state.form()) / peak;
                rgb += *color * contribution;
                weight += contribution;
            }
            let rgb = rgb / weight.max(1.0);
            for column in 0..dim.0 {
                for row in 0..dim.1 {
                    next.set(column, row, height, rgb);
                }
            }
        }
    }
}
