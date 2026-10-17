use std::sync::Arc;
use std::time::Instant;

use nova_core::tides::{self, TideClock};
use nova_core::{Bus, Controls};

use crate::calibration::Pattern;
use crate::content::{Content, all_content};
use crate::host::Settings;
use crate::light::Meter;
use crate::voxel_image::VoxelImage;

/// Longest frame step the renderer passes on, so a stalled frame never makes content or
/// tides jump.
const MAX_FRAME_DELTA_SECONDS: f32 = 0.25;

/// What content renders from: the controls after the tides, the tide clock, and whether to
/// start over. It only carries values; content interprets them.
pub struct RenderState {
    palette: usize,
    heat: f32,
    flow: f32,
    form: f32,
    void: f32,
    tide_seconds: f64,
    reset: bool,
}

impl RenderState {
    /// The controls as they are, at the start of the tide clock.
    pub fn new(controls: &Controls) -> Self {
        Self {
            palette: controls.palette,
            heat: controls.heat,
            flow: controls.flow,
            form: controls.form,
            void: controls.void,
            tide_seconds: 0.0,
            reset: false,
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
    /// The tide clock, for tides inside content modules (see `nova_core::tides`).
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

/// Renders the content with the controls from the bus, and puts the tide clock and the
/// features of the light back on it with every frame.
pub struct Renderer {
    bus: Arc<Bus>,
    content: Vec<Box<dyn Content>>,
    selected_content_index: usize,

    next: VoxelImage,
    output: VoxelImage,

    delta_time: Instant,
    tides: TideClock,
    meter: Meter,
}

impl Renderer {
    pub fn new(dim: (usize, usize, usize), bus: Arc<Bus>) -> Self {
        Self {
            bus,
            content: all_content(),
            selected_content_index: usize::MAX,

            next: VoxelImage::new(dim),
            output: VoxelImage::new(dim),

            delta_time: Instant::now(),
            tides: TideClock::default(),
            meter: Meter::default(),
        }
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    /// Renders the next frame, timed by the clock; see `render_step`.
    pub fn render(&mut self, settings: &Settings) {
        let delta = self
            .delta_time
            .elapsed()
            .as_secs_f32()
            .min(MAX_FRAME_DELTA_SECONDS);
        self.delta_time = Instant::now();
        self.render_step(settings, delta)
    }

    /// Renders one frame `delta` seconds after the previous one; offline renderings call
    /// this directly with a fixed step.
    pub fn render_step(&mut self, settings: &Settings, delta: f32) {
        let controls: Controls = self.bus.controls().sanitized();
        // A calibration test pattern replaces content; content, tides and the light the
        // sound follows pause meanwhile.
        if settings.pattern != Pattern::Off {
            settings.pattern.render(controls.palette, &mut self.next);
            self.output
                .copy_scaled_from(&self.next, controls.brightness);
            return;
        }

        // Tides move the effective controls around the user's settings, for every module.
        self.tides.advance(delta, controls.flow);
        let seconds = self.tides.seconds();
        let mut state = RenderState::new(&tides::effective(&controls, seconds));
        state.tide_seconds = seconds;
        let selected_index = settings.content.min(self.content.len() - 1);
        if selected_index != self.selected_content_index {
            self.selected_content_index = selected_index;
            state.set_reset(true);
            self.next.clear();
        }

        self.content[self.selected_content_index].render(&state, delta, &mut self.next);

        self.output
            .copy_scaled_from(&self.next, controls.brightness);
        self.bus.set_clock(seconds);
        self.bus.set_light(&self.meter.measure(&self.next, delta));
    }

    pub fn image(&self) -> &VoxelImage {
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{MODULE_X_RES, MODULE_Y_RES, MODULE_Z_RES};
    use nova_core::Features;

    #[test]
    fn renderer_plays_the_controls_on_the_bus_and_leaves_clock_and_light() {
        let bus = Arc::new(Bus::new(&Controls::default()));
        let dim = (MODULE_X_RES, MODULE_Y_RES, MODULE_Z_RES);
        let mut renderer = Renderer::new(dim, Arc::clone(&bus));
        let settings = Settings::default();
        for _ in 0..100 {
            renderer.render_step(&settings, 0.04);
        }
        assert!((bus.clock() - 4.0).abs() < 1e-4, "{}", bus.clock());
        let light = bus.light();
        assert!(light.level > 0.0 && light.density > 0.0 && light.activity > 0.0);

        // Brightness scales the output, not the light the sound follows.
        bus.set_controls(&Controls {
            brightness: 0.0,
            ..Controls::default()
        });
        renderer.render_step(&settings, 0.04);
        assert_eq!(renderer.image().get(2, 2, 5), glam::Vec3::ZERO);
        assert!(bus.light().level > 0.0);

        // Flow 0 stops the tide clock; a test pattern pauses content, clock and light.
        bus.set_controls(&Controls {
            flow: 0.0,
            ..Controls::default()
        });
        let clock = bus.clock();
        renderer.render_step(&settings, 0.04);
        assert_eq!(bus.clock(), clock);
        bus.set_controls(&Controls::default());
        let light: Features = bus.light();
        let pattern = Settings {
            pattern: Pattern::Gray,
            ..Settings::default()
        };
        renderer.render_step(&pattern, 0.04);
        assert_eq!((bus.clock(), bus.light()), (clock, light));
    }
}
