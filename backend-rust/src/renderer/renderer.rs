use crate::app_state::AppState;
use crate::content::content::Content;
use crate::content::content::get_all_content;

use super::voxel_image::VoxelImage;

pub struct Renderer {
    selected_content_index: usize,
}

impl Renderer {
    pub fn new() -> Self {
        Renderer {
            selected_content_index: usize::MAX,
        }
    }

    pub fn update(&mut self, state: &AppState) {
        //println!("renderer: update");
        let content = get_all_content();
        if state.selected_content_index() != self.selected_content_index
            && state.selected_content_index() < content.len()
        {
            self.selected_content_index = state.selected_content_index();
            if self.selected_content_index < content.len() {
                content[self.selected_content_index].reset(state);
            }
        }
        content[self.selected_content_index].update(state);
    }

    pub fn render(&self, state: &AppState, image: &mut VoxelImage, delta: f32) {
        //println!("renderer: rendering frame {delta}");
        let content = get_all_content();
        content[self.selected_content_index].render(state, image, delta);
    }
}
