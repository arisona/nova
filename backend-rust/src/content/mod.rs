use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

pub mod fill;
pub mod ramp;

pub trait Content {
    fn name(&self) -> String;
    fn reset(&mut self, state: &RenderState, image: &mut VoxelImage);
    fn render(&mut self, state: &RenderState, image: &mut VoxelImage, delta: f32);
}

pub fn get_all_content() -> Vec<Box<dyn Content>> {
    vec![Box::new(fill::Fill::new()), Box::new(ramp::Ramp::new())]
}

pub fn get_all_content_names() -> Vec<String> {
    assert_ne!(get_all_content().len(), 0);
    get_all_content().iter().map(|c| c.name()).collect()
}
