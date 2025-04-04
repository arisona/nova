use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;
use crate::voxel_image::hsb_to_rgb;

pub struct Fill {}

impl super::Content for Fill {
    fn name(&self) -> String {
        "Fill".to_string()
    }

    fn reset(&self, _: &RenderState, _: &mut VoxelImage) {
        println!("Fill.configure");
    }

    fn render(&self, state: &RenderState, image: &mut VoxelImage, _: f32) {
        //println!("Fill.render");
        let rgb = hsb_to_rgb((state.hue(), state.saturation(), state.brightness()));
        image.fill(rgb);
    }
}
