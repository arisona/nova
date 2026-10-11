//! What the instrument plays from: Volume from the settings, and the effective controls and
//! image features from the renderer. Lock-free, so the audio callback can read it; values
//! may come from different frames, which the instrument's smoothing hides.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use super::features::{FeatureExtractor, Features, LAYERS};
use super::instrument::Controls;
use crate::app_state::AppState;
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

#[derive(Default)]
struct Float(AtomicU32);

impl Float {
    fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }

    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

pub struct AudioFeed {
    volume: Float,
    palette: AtomicUsize,
    heat: Float,
    flow: Float,
    form: Float,
    void: Float,
    flow_setting: Float,
    tide_seconds: AtomicU64,
    activity: Float,
    layers: [Float; LAYERS],
}

impl AudioFeed {
    /// Starts from the settings, so the instrument starts in the right place before the
    /// renderer's first frame.
    pub fn new(state: &AppState) -> Self {
        let layers = Features::default().layers;
        Self {
            volume: Float::new(state.volume()),
            palette: AtomicUsize::new(state.palette_index()),
            heat: Float::new(state.heat()),
            flow: Float::new(state.flow()),
            form: Float::new(state.form()),
            void: Float::new(state.void()),
            flow_setting: Float::new(state.flow()),
            tide_seconds: AtomicU64::new(0.0_f64.to_bits()),
            activity: Float::new(0.0),
            layers: layers.map(Float::new),
        }
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume.set(volume);
    }

    /// Publishes the effective controls (after the tides), the Flow slider setting and the
    /// tide clock.
    fn set_controls(&self, state: &RenderState, flow_setting: f32) {
        self.palette.store(state.palette(), Ordering::Relaxed);
        self.heat.set(state.heat());
        self.flow.set(state.flow());
        self.form.set(state.form());
        self.void.set(state.void());
        self.flow_setting.set(flow_setting);
        self.tide_seconds
            .store(state.tide_seconds().to_bits(), Ordering::Relaxed);
    }

    fn set_features(&self, features: &Features) {
        self.activity.set(features.activity);
        for (slot, &layer) in self.layers.iter().zip(&features.layers) {
            slot.set(layer);
        }
    }

    pub fn controls(&self) -> Controls {
        Controls {
            volume: self.volume.get(),
            palette: self.palette.load(Ordering::Relaxed),
            heat: self.heat.get(),
            flow: self.flow.get(),
            form: self.form.get(),
            void: self.void.get(),
            flow_setting: self.flow_setting.get(),
            tide_seconds: f64::from_bits(self.tide_seconds.load(Ordering::Relaxed)),
        }
    }

    pub fn features(&self) -> Features {
        Features {
            layers: self.layers.each_ref().map(Float::get),
            activity: self.activity.get(),
        }
    }
}

/// The renderer's side of the feed: extracts features from every rendered frame.
pub struct FeedPublisher {
    feed: std::sync::Arc<AudioFeed>,
    extractor: FeatureExtractor,
}

impl FeedPublisher {
    pub fn new(feed: std::sync::Arc<AudioFeed>) -> Self {
        Self {
            feed,
            extractor: FeatureExtractor::default(),
        }
    }

    /// `image` is the content before Brightness, `delta` the frame time.
    pub fn publish(
        &mut self,
        state: &RenderState,
        flow_setting: f32,
        image: &VoxelImage,
        delta: f32,
    ) {
        self.feed.set_controls(state, flow_setting);
        self.feed
            .set_features(&self.extractor.extract(image, delta));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palettes::PALETTES;
    use crate::renderer::Renderer;
    use std::sync::Arc;

    #[test]
    fn feed_carries_settings_then_effective_controls_and_features() {
        let mut state = AppState::default();
        state.set_volume(0.4);
        state.set_palette(PALETTES[2].name);
        state.set_heat(0.3);
        state.set_flow(0.5);
        state.set_form(0.6);
        state.set_void(0.7);
        let feed = Arc::new(AudioFeed::new(&state));
        assert_eq!(
            feed.controls(),
            Controls {
                volume: 0.4,
                palette: 2,
                heat: 0.3,
                flow: 0.5,
                form: 0.6,
                void: 0.7,
                flow_setting: 0.5,
                tide_seconds: 0.0,
            }
        );
        feed.set_volume(0.0);
        assert_eq!(feed.controls().volume, 0.0);

        let mut renderer = Renderer::new(state.dim());
        renderer.feed_audio(FeedPublisher::new(Arc::clone(&feed)));
        for _ in 0..100 {
            renderer.render_step(&mut RenderState::from(&state), 0.04);
        }
        let controls = feed.controls();
        assert_eq!(controls.flow_setting, 0.5);
        assert!(
            (controls.tide_seconds - 4.0).abs() < 1e-4,
            "{}",
            controls.tide_seconds
        );
        assert_ne!(controls.heat, 0.3, "the tides move the effective controls");
        let features = feed.features();
        assert!((features.layers.iter().sum::<f32>() - 1.0).abs() < 1e-4);
        assert!(features.activity > 0.0);
    }
}
