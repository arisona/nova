use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;
use glam::Vec3;
use noise::{NoiseFn, Simplex as SimplexNoise};

pub struct Simplex {
    simplex: SimplexNoise,
    palette: Vec<Vec3>,
}

impl Simplex {
    pub fn new() -> Self {
        Self {
            simplex: SimplexNoise::new(0xffff8240),
            palette: vec![
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
        }
    }
}

impl super::Content for Simplex {
    fn name(&self) -> &str {
        "Simplex"
    }

    fn render(
        &mut self,
        state: &RenderState,
        elapsed: f32,
        _delta: f32,
        _prev: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        let entropy = state.hue() as f64;
        let scale = state.saturation() as f64;
        let speed = state.speed() as f64;
        let elapsed = elapsed as f64;

        let dim = next.dim();
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    let fx = x as f64 / dim.0 as f64;
                    let fy = y as f64 / dim.1 as f64;
                    let fz = z as f64 / dim.2 as f64;

                    let base =
                        self.simplex
                            .get([fx * scale, fy * scale, fz * scale, elapsed * speed]);

                    let turb1 = self.simplex.get([
                        2.0 * fx * scale,
                        2.0 * fy * scale,
                        2.0 * fz * scale,
                        1.5 * elapsed * speed,
                    ]);

                    let turb2 = self.simplex.get([
                        4.0 * fx * scale,
                        4.0 * fy * scale,
                        4.0 * fz * scale,
                        2.5 * elapsed * speed,
                    ]);

                    let noise_value = base + entropy * turb1 + entropy.powi(2) * turb2;
                    let noise_value = ((noise_value + 1.0) * 0.5).clamp(0.0, 0.9999);
                    let palette_index = noise_value * (self.palette.len() as f64 - 1.0);
                    let lower_index = palette_index.floor() as usize;
                    let upper_index = lower_index + 1;

                    let mix = palette_index.fract();
                    let color = if upper_index < self.palette.len() {
                        self.palette[lower_index].lerp(self.palette[upper_index], mix as f32)
                    } else {
                        self.palette[lower_index]
                    };

                    next.set(x, y, z, color * state.brightness());
                }
            }
        }
    }
}
