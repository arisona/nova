use super::super::renderer::voxel_image::VoxelImage;
pub struct Fill {}

impl super::content::Content for Fill {
    fn name(&self) -> String {
        "Fill".to_string()
    }

    fn configure(&self, _dimensions: (u32, u32, u32)) {
        println!("Fill.configure");
    }

    fn render(&self, image: VoxelImage) {
        println!("Fill.render");
    }
}
