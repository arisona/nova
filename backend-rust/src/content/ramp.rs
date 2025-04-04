use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;
pub struct Ramp {}

impl super::Content for Ramp {
    fn name(&self) -> String {
        "Ramp".to_string()
    }

    fn reset(&self, state: &RenderState, image: &mut VoxelImage) {
        println!("Ramp.reset");
        for x in 0..image.dx() {
            for y in 0..image.dy() {
                for z in 0..image.dz() {
                    let r = x as f32 / image.dx() as f32 * state.brightness();
                    let g = y as f32 / image.dy() as f32 * state.brightness();
                    let b = z as f32 / image.dz() as f32 * state.brightness();
                    image.set(x, y, z, (r, g, b));
                }
            }
        }
    }

    fn render(&self, state: &RenderState, image: &mut VoxelImage, delta: f32) {
        let (dx, dy, dz) = (image.dx(), image.dy(), image.dz());
        let (d_x, d_y, d_z) = (0.0, 0.0, 1.0); // Direction
        let v = state.speed();

        // Normalize direction
        let len = f32::sqrt(d_x * d_x + d_y * d_y + d_z * d_z);
        let (d_x, d_y, d_z) = (d_x / len, d_y / len, d_z / len);

        // Shift vector in voxel units
        let d = 1.0;
        let shift_x = (d * d_x).round() as i32;
        let shift_y = (d * d_y).round() as i32;
        let shift_z = (d * d_z).round() as i32;

        // Clone current voxel values
        let original = image.clone();

        for x in 0..dx {
            for y in 0..dy {
                for z in 0..dz {
                    let src_x = (x as i32 - shift_x).rem_euclid(dx as i32);
                    let src_y = (y as i32 - shift_y).rem_euclid(dy as i32);
                    let src_z = (z as i32 - shift_z).rem_euclid(dz as i32);

                    let color = original.get(src_x as usize, src_y as usize, src_z as usize);
                    image.set(x, y, z, color);
                }
            }
        }
    }
}
