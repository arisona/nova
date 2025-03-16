use super::super::renderer::voxel_image::VoxelImage;

pub trait Content {
    fn name(&self) -> String;
    fn configure(&self, dimensions: (u32, u32, u32));
    fn render(&self, image: VoxelImage);
}

pub fn get_all_content() -> Vec<Box<dyn Content>> {
    vec![Box::new(super::fill::Fill {})]
}
