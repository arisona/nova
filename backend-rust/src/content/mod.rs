use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

pub mod fill;
pub mod ramp;
pub mod simplex;

pub trait Content {
    fn name(&self) -> &str;
    fn render(
        &mut self,
        state: &RenderState,
        elapsed: f32,
        delta: f32,
        prev: &VoxelImage,
        next: &mut VoxelImage,
    );
}

pub fn get_all_content() -> Vec<Box<dyn Content>> {
    vec![
        Box::new(fill::Fill::new()),
        Box::new(ramp::Ramp::new()),
        Box::new(simplex::Simplex::new()),
    ]
}

pub fn get_all_content_names() -> Vec<String> {
    assert_ne!(get_all_content().len(), 0);
    get_all_content()
        .iter()
        .map(|c| c.name().to_string())
        .collect()
}
