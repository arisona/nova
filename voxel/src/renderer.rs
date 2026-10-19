use std::sync::Arc;
use std::time::Instant;

use nova_core::tides::{self, TideClock};
use nova_core::{Bus, Controls};

use crate::calibration::Pattern;
use crate::content::common::ease;
use crate::content::{Content, all_content};
use crate::host::Settings;
use crate::light::Meter;
use crate::sparkle::Sparkle;
use crate::voxel_image::VoxelImage;

/// Longest frame step the renderer passes on, so a stalled frame never makes content or
/// tides jump.
const MAX_FRAME_DELTA_SECONDS: f32 = 0.25;

/// Seconds a content or palette change takes: the old one fades out through Void, the new
/// one fades in from the dark, each over half of it. Zero cuts.
const CONTENT_FADE_SECONDS: f32 = 5.0;
const PALETTE_FADE_SECONDS: f32 = 2.0;
/// How far a fade takes Void towards 1 at its darkest point: 1 is completely dark.
const FADE_DEPTH: f32 = 0.5;
/// How much a fade sparkles at its darkest point (see `sparkle.rs`): 0 not at all, 1 fully.
const FADE_SPARKLE: f32 = 1.0;

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
    /// The module and palette rendering, which during a fade are still the old ones until
    /// it is dark.
    selected_content_index: usize,
    palette: usize,
    /// Progress of a fade, from 0 to 1: dark at 0.5, done at 1.
    fade: f32,

    next: VoxelImage,
    output: VoxelImage,

    delta_time: Instant,
    tides: TideClock,
    meter: Meter,
    sparkle: Sparkle,
}

impl Renderer {
    pub fn new(dim: (usize, usize, usize), bus: Arc<Bus>) -> Self {
        Self {
            bus,
            content: all_content(),
            selected_content_index: usize::MAX,
            palette: 0,
            fade: 1.0,

            next: VoxelImage::new(dim),
            output: VoxelImage::new(dim),

            delta_time: Instant::now(),
            tides: TideClock::default(),
            meter: Meter::default(),
            sparkle: Sparkle::default(),
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
        let content_changes = selected_index != self.selected_content_index;
        let changes = content_changes || controls.palette != self.palette;
        let fade_seconds = if content_changes {
            CONTENT_FADE_SECONDS
        } else {
            PALETTE_FADE_SECONDS
        };
        if self.selected_content_index == usize::MAX || fade_seconds <= 0.0 {
            self.fade = 1.0;
        } else {
            // A change of mind turns the fade around, from the darkness it has reached.
            let fading_out = self.fade < 0.5;
            if fading_out != changes {
                self.fade = 1.0 - self.fade;
            }
            let before = self.fade;
            self.fade = (before + delta / fade_seconds).min(1.0);
            if before < 0.5 && self.fade >= 0.5 {
                // Swap at the darkest point; the new module or palette starts from there.
                self.fade = 0.5;
            }
        }
        if self.fade >= 0.5 {
            if content_changes {
                self.selected_content_index = selected_index;
                state.set_reset(true);
                self.next.clear();
            }
            self.palette = controls.palette;
        }
        state.palette = self.palette;
        // The fade lifts Void, and sparkles as much as it lifts: not at all where Void
        // already darkens everything.
        let darkness = ease(1.0 - (2.0 * self.fade - 1.0).abs());
        let lift = (1.0 - state.void) * darkness;
        state.void += FADE_DEPTH * lift;

        self.content[self.selected_content_index].render(&state, delta, &mut self.next);

        let light = self.meter.measure(&self.next, delta);
        self.sparkle
            .update(&light, FADE_SPARKLE * lift, delta, self.next.dim());
        self.output
            .copy_scaled_from(&self.next, controls.brightness);
        self.sparkle.draw(&mut self.output, controls.brightness);
        self.bus.set_clock(seconds);
        self.bus.set_light(&light);
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

    fn total(image: &VoxelImage) -> f32 {
        let (dx, dy, dz) = image.dim();
        (0..dx)
            .flat_map(|x| (0..dy).flat_map(move |y| (0..dz).map(move |z| (x, y, z))))
            .map(|(x, y, z)| image.get(x, y, z).element_sum())
            .sum()
    }

    /// A content change swaps the modules where the old one has faded to dark, turns
    /// around when the selection changes back, and ends on the selected module; a palette
    /// change fades the same way.
    #[test]
    fn changes_fade_through_the_dark() {
        let bus = Arc::new(Bus::new(&Controls::default()));
        let dim = (MODULE_X_RES, MODULE_Y_RES, MODULE_Z_RES);
        let mut renderer = Renderer::new(dim, Arc::clone(&bus));
        let step = 0.04;
        let frames = (CONTENT_FADE_SECONDS / step).ceil() as usize;
        let mut settings = Settings::default();
        renderer.render_step(&settings, step);
        assert!(
            total(renderer.image()) > 0.0,
            "the first module shows at once"
        );

        settings.content = 1;
        for _ in 0..frames / 4 {
            renderer.render_step(&settings, step);
        }
        assert_eq!(renderer.selected_content_index, 0);
        settings.content = 0;
        for _ in 0..frames {
            renderer.render_step(&settings, step);
        }
        assert_eq!((renderer.selected_content_index, renderer.fade), (0, 1.0));

        settings.content = 1;
        let lit = total(renderer.image());
        let mut darkest = f32::INFINITY;
        for _ in 0..frames + 1 {
            renderer.render_step(&settings, step);
            darkest = darkest.min(total(renderer.image()));
        }
        assert!(darkest < lit, "the swap happens in the dark");
        assert_eq!((renderer.selected_content_index, renderer.fade), (1, 1.0));
        assert!(total(renderer.image()) > 0.0);

        bus.set_controls(&Controls {
            palette: 1,
            ..Controls::default()
        });
        let frames = (PALETTE_FADE_SECONDS / step).ceil() as usize;
        for _ in 0..frames / 4 {
            renderer.render_step(&settings, step);
        }
        assert_eq!(renderer.palette, 0);
        for _ in 0..frames {
            renderer.render_step(&settings, step);
        }
        assert_eq!((renderer.palette, renderer.fade), (1, 1.0));
    }
}
