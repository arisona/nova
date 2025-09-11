use glam::vec3;

use crate::content;
use crate::renderer::RenderState;
use crate::voxel_image::{self, VoxelImage};

pub struct Ramp {
    phase: f32,
}

impl Ramp {
    pub fn new() -> Self {
        Self { phase: 0.0 }
    }
}

impl content::Content for Ramp {
    fn name(&self) -> &str {
        "Ramp"
    }

    fn render(
        &mut self,
        state: &RenderState,
        _elapsed: f32,
        delta: f32,
        _prev: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        if state.should_reset() {
            self.phase = 0.0;
        }

        self.phase += state.speed() * delta.min(0.1);
        let hue = state.hue().max(0.001);
        let t = self.phase / hue;

        let (dx, dy, dz) = (next.dx() as f32, next.dy() as f32, next.dz() as f32);
        for x in 0..next.dx() {
            for y in 0..next.dy() {
                for z in 0..next.dz() {
                    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
                    let h = ((t + fx / dx + fy / dy + fz / dz) * hue).rem_euclid(1.0);
                    next.set(
                        x,
                        y,
                        z,
                        voxel_image::hsb_to_rgb(vec3(h, state.saturation(), state.brightness())),
                    );
                }
            }
        }
    }
}
