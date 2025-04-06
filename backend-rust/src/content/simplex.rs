use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;
pub struct Simplex {}

impl Simplex {
    pub fn new() -> Self {
        Self {}
    }
}

impl super::Content for Simplex {
    fn name(&self) -> &str {
        "Simplex"
    }

    fn render(&mut self, state: &RenderState, image: &mut VoxelImage, elapsed: f32, delta: f32) {}
}
