use crate::app_state::AppState;
use crate::renderer::voxel_image::VoxelImage;

pub trait Content {
    fn name(&self) -> String;
    fn reset(&self, state: &AppState);
    fn render(&self, state: &AppState, image: &mut VoxelImage, delta: f32);
}

pub fn get_all_content() -> Vec<Box<dyn Content>> {
    vec![
        Box::new(super::fill::Fill {}),
        Box::new(super::ramp::Ramp {}),
    ]
}

pub fn get_all_content_names() -> Vec<String> {
    assert_ne!(get_all_content().len(), 0);
    get_all_content().iter().map(|c| c.name()).collect()
}
