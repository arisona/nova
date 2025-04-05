use glam::vec3;

use crate::renderer::RenderState;
use crate::voxel_image::{self, VoxelImage};
pub struct Ramp {
    time: f32,
}

impl Ramp {
    pub fn new() -> Self {
        println!("Ramp.new");
        Self { time: 0.0 }
    }
}

impl super::Content for Ramp {
    fn name(&self) -> String {
        "Ramp".to_string()
    }

    fn reset(&mut self, _: &RenderState, _: &mut VoxelImage) {}

    fn render(&mut self, state: &RenderState, image: &mut VoxelImage, delta: f32) {
        println!("Ramp.render {} {}", delta, self.time);
        let (dx, dy, dz) = (image.dx() as f32, image.dy() as f32, image.dz() as f32);
        let t = self.time / 10.0;
        for x in 0..image.dx() {
            for y in 0..image.dy() {
                for z in 0..image.dz() {
                    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
                    let h = (t + fx / dx * fy / dy * fz / dz * state.hue()).rem_euclid(1.0);
                    image.set(
                        x,
                        y,
                        z,
                        voxel_image::hsb_to_rgb(vec3(h, 1.0, state.brightness())),
                    );
                }
            }
        }
        self.time += delta;
    }
}
