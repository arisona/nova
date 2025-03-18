use crate::app_state::AppState;
use crate::renderer::voxel_image::VoxelImage;
use crate::renderer::voxel_image::hsb_to_rgb;
pub struct Fill {}

impl super::content::Content for Fill {
    fn name(&self) -> String {
        "Fill".to_string()
    }

    fn reset(&self, state: &AppState) {
        println!("Fill.configure");
    }

    fn render(&self, state: &AppState, image: &mut VoxelImage, delta: f32) {
        //println!("Fill.render");
        let rgb = hsb_to_rgb((state.hue(), state.saturation(), state.brightness()));
        image.fill(rgb);
    }
}
