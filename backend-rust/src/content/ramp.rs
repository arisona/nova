use glam::vec3;

use crate::renderer::RenderState;
use crate::voxel_image::{self, VoxelImage};
pub struct Ramp {}

impl Ramp {
    pub fn new() -> Self {
        Self {}
    }
}

impl super::Content for Ramp {
    fn name(&self) -> &str {
        "Ramp"
    }

    // TODO: this is just an uninspired fake implementation
    fn render(
        &mut self,
        state: &RenderState,
        elapsed: f32,
        _: f32,
        _: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        let (dx, dy, dz) = (next.dx() as f32, next.dy() as f32, next.dz() as f32);
        let t = elapsed / 10.0;
        for x in 0..next.dx() {
            for y in 0..next.dy() {
                for z in 0..next.dz() {
                    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
                    let h = ((t + fx / dx + fy / dy + fz / dz) * state.hue()).rem_euclid(1.0);
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
