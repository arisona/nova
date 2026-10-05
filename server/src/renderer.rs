use crate::app_state::AppState;
use crate::content::get_all_content;
use crate::voxel_image::VoxelImage;

pub struct RenderState {
    brightness: f32,
    palette: usize,
    heat: f32,
    flow: f32,
    form: f32,
    void: f32,

    reset: bool,

    selected_content_index: usize,
}

impl RenderState {
    pub fn from(state: &AppState) -> Self {
        Self {
            brightness: state.brightness(),
            palette: state.palette_index(),
            heat: state.heat(),
            flow: state.flow(),
            form: state.form(),
            void: state.void(),

            reset: false,

            selected_content_index: state.selected_content_index(),
        }
    }

    /// Position of the selected palette in `PALETTES`.
    pub fn palette(&self) -> usize {
        self.palette
    }
    pub fn heat(&self) -> f32 {
        self.heat
    }
    pub fn flow(&self) -> f32 {
        self.flow
    }
    pub fn form(&self) -> f32 {
        self.form
    }
    pub fn void(&self) -> f32 {
        self.void
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
    output: VoxelImage,

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
            output: VoxelImage::new(dim),

            elapsed_time: std::time::Instant::now(),
            delta_time: std::time::Instant::now(),
        }
    }

    pub fn render(&mut self, state: &mut RenderState) {
        //log::debug!("renderer: rendering frame {delta}");
        let mut elapsed = self.elapsed_time.elapsed().as_secs_f32();
        let delta = self.delta_time.elapsed().as_secs_f32();
        self.delta_time = std::time::Instant::now();

        let selected_index = state.selected_content_index.min(self.content.len() - 1);
        if selected_index != self.selected_content_index {
            self.selected_content_index = selected_index;
            state.set_reset(true);
            self.prev.clear();
            self.next.clear();
            self.elapsed_time = std::time::Instant::now();
            elapsed = 0.0;
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
        self.output.copy_scaled_from(&self.next, state.brightness);

        // TODO: move this to a separate content module for debugging
        // Testing: wait for a random time between 5 and 20 ms
        // let mut rng = rand::rng();
        // let random_delay = rng.random_range(5..=100);
        // log::debug!("Random delay: {random_delay}ms");
        // std::thread::sleep(Duration::from_millis(random_delay));
    }

    pub fn image(&mut self) -> &VoxelImage {
        &self.output
    }
}
