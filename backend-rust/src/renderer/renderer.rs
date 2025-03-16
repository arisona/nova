use super::super::app_state::AppState;
use super::voxel_image::VoxelImage;

pub struct Renderer {}

impl Renderer {
    pub fn new() -> Self {
        Renderer {}
    }

    pub fn render_frame(&self, state: &AppState, image: &mut VoxelImage, delta: f32) {
        println!("Rendering frame {delta}");
    }
}
