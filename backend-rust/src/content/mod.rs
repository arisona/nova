use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

pub mod fill;
pub mod ramp;

pub trait Content {
    fn name(&self) -> String;
    fn reset(&self, state: &RenderState, image: &mut VoxelImage);
    fn render(&self, state: &RenderState, image: &mut VoxelImage, delta: f32);
}

pub fn get_all_content() -> Vec<Box<dyn Content>> {
    vec![Box::new(fill::Fill {}), Box::new(ramp::Ramp {})]
}

pub fn get_all_content_names() -> Vec<String> {
    assert_ne!(get_all_content().len(), 0);
    get_all_content().iter().map(|c| c.name()).collect()
}
