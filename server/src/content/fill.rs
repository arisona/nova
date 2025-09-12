use glam::vec3;

use crate::content;
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;
use crate::voxel_image::hsb_to_rgb;

pub struct Fill {}

impl Fill {
    pub fn new() -> Self {
        Self {}
    }
}

impl content::Content for Fill {
    fn name(&self) -> &str {
        "Fill"
    }

    fn render(
        &mut self,
        state: &RenderState,
        _elapsed: f32,
        _delta: f32,
        _prev: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        let rgb = hsb_to_rgb(vec3(state.tone(), state.punch(), state.glow()));
        next.fill(rgb);
    }
}
