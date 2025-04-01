use crate::renderer::renderer::RenderState;
use crate::renderer::voxel_image::VoxelImage;
pub struct Ramp {}

impl super::content::Content for Ramp {
    fn name(&self) -> String {
        "Ramp".to_string()
    }

    fn reset(&self, _: &RenderState) {
        println!("Ramp.reset");
    }

    fn render(&self, state: &RenderState, image: &mut VoxelImage, _: f32) {
        //println!("Ramp.render");
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
}
