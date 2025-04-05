use crate::app_state::AppState;
use crate::content::get_all_content;
use crate::voxel_image::VoxelImage;

pub struct RenderState {
    hue: f32,
    saturation: f32,
    brightness: f32,
    speed: f32,

    selected_content_index: usize,
}

impl RenderState {
    pub fn from(state: &AppState) -> Self {
        Self {
            hue: state.hue(),
            saturation: state.saturation(),
            brightness: state.brightness(),
            speed: state.speed(),

            selected_content_index: state.selected_content_index(),
        }
    }

    pub fn hue(&self) -> f32 {
        self.hue
    }

    pub fn saturation(&self) -> f32 {
        self.saturation
    }

    pub fn brightness(&self) -> f32 {
        self.brightness
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }
}

pub struct Renderer {
    content: Vec<Box<dyn crate::content::Content>>,
    selected_content_index: usize,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            content: get_all_content(),
            selected_content_index: usize::MAX,
        }
    }

    pub fn render(&mut self, state: &RenderState, image: &mut VoxelImage, delta: f32) {
        //println!("renderer: rendering frame {delta}");
        if state.selected_content_index != self.selected_content_index
            && state.selected_content_index < self.content.len()
        {
            self.selected_content_index = state.selected_content_index;
            if self.selected_content_index < self.content.len() {
                //println!("renderer: reset");
                self.content[self.selected_content_index].reset(state, image);
            }
        }
        self.content[self.selected_content_index].render(state, image, delta);
    }
}
