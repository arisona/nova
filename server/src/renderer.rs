use crate::app_state::AppState;
use crate::content::get_all_content;
use crate::voxel_image::VoxelImage;

pub struct RenderState {
    glow: f32,
    tone: f32,
    punch: f32,
    flow: f32,
    form: f32,

    reset: bool,

    selected_content_index: usize,
}

impl RenderState {
    pub fn from(state: &AppState) -> Self {
        Self {
            glow: state.glow(),
            tone: state.tone(),
            punch: state.punch(),
            flow: state.flow(),
            form: state.form(),

            reset: false,

            selected_content_index: state.selected_content_index(),
        }
    }

    pub fn glow(&self) -> f32 {
        self.glow
    }
    pub fn tone(&self) -> f32 {
        self.tone
    }
    pub fn punch(&self) -> f32 {
        self.punch
    }
    pub fn flow(&self) -> f32 {
        self.flow
    }
    pub fn form(&self) -> f32 {
        self.form
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
        //log::debug!("renderer: rendering frame {delta}");
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

        state.set_reset(false);

        // TODO: move this to a separate content module for debugging
        // Testing: wait for a random time between 5 and 20 ms
        // let mut rng = rand::rng();
        // let random_delay = rng.random_range(5..=100);
        // log::debug!("Random delay: {random_delay}ms");
        // std::thread::sleep(Duration::from_millis(random_delay));
    }

    pub fn image(&mut self) -> &VoxelImage {
        &self.next
    }
}
