use crate::app_state::AppState;
use crate::content::get_all_content;
use crate::voxel_image::VoxelImage;

pub struct RenderState {
    hue: f32,
    saturation: f32,
    brightness: f32,
    speed: f32,

    reset: bool,

    selected_content_index: usize,
}

impl RenderState {
    pub fn from(state: &AppState) -> Self {
        Self {
            hue: state.hue(),
            saturation: state.saturation(),
            brightness: state.brightness(),
            speed: state.speed(),

            reset: false,

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

    pub fn should_reset(&self) -> bool {
        self.reset
    }

    pub fn set_reset(&mut self, reset: bool) {
        self.reset = reset;
    }
}

pub struct Renderer {
    content: Vec<Box<dyn crate::content::Content>>,
    selected_content_index: usize,

    prev: VoxelImage,
    next: VoxelImage,

    elapsed_time: std::time::Instant,
    delta_time: std::time::Instant,
}

impl Renderer {
    pub fn new(dim: (usize, usize, usize)) -> Self {
        Self {
            content: get_all_content(),
            selected_content_index: usize::MAX,

            prev: VoxelImage::new(dim),
            next: VoxelImage::new(dim),

            elapsed_time: std::time::Instant::now(),
            delta_time: std::time::Instant::now(),
        }
    }

    pub fn render(&mut self, state: &mut RenderState) {
        //println!("renderer: rendering frame {delta}");
        let elapsed = self.elapsed_time.elapsed().as_secs_f32();
        let delta = self.delta_time.elapsed().as_secs_f32();
        self.delta_time = std::time::Instant::now();

        if state.selected_content_index != self.selected_content_index
            && state.selected_content_index < self.content.len()
        {
            self.selected_content_index = state.selected_content_index;
            if self.selected_content_index < self.content.len() {
                state.set_reset(true);
                self.prev.clear();
                self.next.clear();
                self.elapsed_time = std::time::Instant::now();
            }
        }

        std::mem::swap(&mut self.prev, &mut self.next);

        self.content[self.selected_content_index].render(
            state,
            elapsed,
            delta,
            &self.prev,
            &mut self.next,
        );

        // Testing wait for a random time between 5 and 20 ms
        // let mut rng = rand::rng();
        // let random_delay = rng.random_range(5..=100);
        // println!("Random delay: {random_delay}ms");
        // std::thread::sleep(Duration::from_millis(random_delay));
    }

    pub fn image(&mut self) -> &VoxelImage {
        &self.next
    }
}
