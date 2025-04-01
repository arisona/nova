use crate::renderer::renderer::RenderState;
use crate::renderer::voxel_image::VoxelImage;
use crate::renderer::voxel_image::hsb_to_rgb;
pub struct Fill {}

impl super::content::Content for Fill {
    fn name(&self) -> String {
        "Fill".to_string()
    }

    fn reset(&self, _: &RenderState) {
        println!("Fill.configure");
    }

    fn render(&self, state: &RenderState, image: &mut VoxelImage, _: f32) {
        //println!("Fill.render");
        let rgb = hsb_to_rgb((state.hue(), state.saturation(), state.brightness()));
        image.fill(rgb);
    }
}
