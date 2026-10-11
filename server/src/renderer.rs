use std::time::Instant;

use crate::app_state::AppState;
use crate::audio::FeedPublisher;
use crate::calibration::Pattern;
use crate::content::{Content, all_content};
use crate::tides::{self, TideClock};
use crate::voxel_image::VoxelImage;

/// Longest frame step the renderer passes on, so a stalled frame never makes content or
/// tides jump.
const MAX_FRAME_DELTA_SECONDS: f32 = 0.25;

pub struct RenderState {
    brightness: f32,
    palette: usize,
    heat: f32,
    flow: f32,
    form: f32,
    void: f32,
    tide_seconds: f64,

    reset: bool,

    selected_content_index: usize,
    calibration_pattern: Pattern,
}

impl From<&AppState> for RenderState {
    fn from(state: &AppState) -> Self {
        Self {
            brightness: state.brightness(),
            palette: state.palette_index(),
            heat: state.heat(),
            flow: state.flow(),
            form: state.form(),
            void: state.void(),
            tide_seconds: 0.0,

            reset: false,

            selected_content_index: state.selected_content_index(),
            calibration_pattern: state.calibration_pattern(),
        }
    }
}

impl RenderState {
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
    /// The tide clock, for tides inside content modules (see `crate::tides`).
    pub fn tide_seconds(&self) -> f64 {
        self.tide_seconds
    }

    pub fn should_reset(&self) -> bool {
        self.reset
    }

    pub fn set_reset(&mut self, reset: bool) {
        self.reset = reset;
    }
}

pub struct Renderer {
    content: Vec<Box<dyn Content>>,
    selected_content_index: usize,

    next: VoxelImage,
    output: VoxelImage,

    delta_time: Instant,
    tides: TideClock,

    audio: Option<FeedPublisher>,
}

impl Renderer {
    pub fn new(dim: (usize, usize, usize)) -> Self {
        Self {
            content: all_content(),
            selected_content_index: usize::MAX,

            next: VoxelImage::new(dim),
            output: VoxelImage::new(dim),

            delta_time: Instant::now(),
            tides: TideClock::default(),

            audio: None,
        }
    }

    /// Lets the sound follow the effective controls and the content of every frame.
    pub fn feed_audio(&mut self, publisher: FeedPublisher) {
        self.audio = Some(publisher);
    }

    pub fn render(&mut self, state: &mut RenderState) {
        let delta = self
            .delta_time
            .elapsed()
            .as_secs_f32()
            .min(MAX_FRAME_DELTA_SECONDS);
        self.delta_time = Instant::now();
        self.render_step(state, delta);
    }

    /// Renders one frame `delta` seconds after the previous one; offline renderings call
    /// this directly with a fixed step.
    pub fn render_step(&mut self, state: &mut RenderState, delta: f32) {
        // A calibration test pattern replaces content; content and tides pause meanwhile.
        if state.calibration_pattern != Pattern::Off {
            state
                .calibration_pattern
                .render(state.palette, &mut self.next);
            self.output.copy_scaled_from(&self.next, state.brightness);
            return;
        }

        let selected_index = state.selected_content_index.min(self.content.len() - 1);
        if selected_index != self.selected_content_index {
            self.selected_content_index = selected_index;
            state.set_reset(true);
            self.next.clear();
        }

        // Tides move the effective controls around the user's settings, for every module.
        let flow_setting = state.flow;
        self.tides.advance(delta, state.flow);
        let seconds = self.tides.seconds();
        state.heat = tides::HEAT.apply(state.heat, seconds);
        state.flow = tides::FLOW.apply(state.flow, seconds);
        state.form = tides::FORM.apply(state.form, seconds);
        state.void = tides::VOID.apply(state.void, seconds);
        state.tide_seconds = seconds;

        self.content[self.selected_content_index].render(state, delta, &mut self.next);

        state.set_reset(false);
        self.output.copy_scaled_from(&self.next, state.brightness);
        if let Some(audio) = &mut self.audio {
            audio.publish(state, flow_setting, &self.next, delta);
        }
    }

    pub fn image(&self) -> &VoxelImage {
        &self.output
    }
}
