use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

mod flux;

const FLOW_SPEED_MULTIPLIER: f32 = 10.0;

pub trait Content {
    fn name(&self) -> &str;
    /// `elapsed` is the time since this module was selected; `delta` is the frame time,
    /// capped by the renderer so a stalled frame never jumps.
    fn render(
        &mut self,
        state: &RenderState,
        elapsed: f32,
        delta: f32,
        prev: &VoxelImage,
        next: &mut VoxelImage,
    );
}

/// Content modules in display order. Settings and the API refer to them by name: keep
/// names unique and free of commas.
pub fn all_content() -> Vec<Box<dyn Content>> {
    vec![Box::new(flux::Flux::new())]
}

pub fn all_content_names() -> Vec<String> {
    let names: Vec<String> = all_content().iter().map(|c| c.name().to_string()).collect();
    assert!(!names.is_empty());
    names
}

pub fn advance(phase: &mut f64, state: &RenderState, delta: f32) -> f32 {
    if state.should_reset() {
        *phase = 0.0;
    } else {
        *phase += (FLOW_SPEED_MULTIPLIER * state.flow() * delta) as f64;
    }
    *phase as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::palettes::PALETTES;
    use glam::Vec3;

    #[test]
    fn flow_uses_configured_speed_without_phase_jumps() {
        let mut settings = AppState::default();
        let mut phase = 2.0;
        let mut expected = 2.0;
        for flow in [0.0, 0.25, 1.0, 0.5, 0.0] {
            settings.set_flow(flow);
            let state = RenderState::from(&settings);
            assert!((advance(&mut phase, &state, 0.0) - expected).abs() < 0.00001);
            expected += FLOW_SPEED_MULTIPLIER * flow * 0.2;
            assert!((advance(&mut phase, &state, 0.2) - expected).abs() < 0.00001);
        }
        let mut state = RenderState::from(&settings);
        state.set_reset(true);
        assert_eq!(advance(&mut phase, &state, 0.2), 0.0);
    }

    fn pixels(image: &VoxelImage) -> Vec<Vec3> {
        let dim = image.dim();
        (0..dim.0)
            .flat_map(|x| (0..dim.1).flat_map(move |y| (0..dim.2).map(move |z| image.get(x, y, z))))
            .collect()
    }

    fn assert_bounded(image: &VoxelImage, name: &str) {
        for rgb in pixels(image) {
            assert!(
                rgb.is_finite() && rgb.min_element() >= 0.0 && rgb.max_element() <= 1.0,
                "{name} wrote {rgb}"
            );
        }
    }

    /// Every content module must stay finite and within [0, 1] (the hardware maps values
    /// straight to 10-bit duty cycles), freeze at Flow 0, and restart on reset. Covers one
    /// and two modules.
    #[test]
    fn content_is_bounded_freezes_and_resets() {
        for dim in [(5, 5, 10), (10, 5, 10)] {
            for form in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for value in [0.0, 0.5, 1.0] {
                    let mut settings = AppState::default();
                    settings.set_flow(0.0);
                    settings.set_form(form);
                    let palette = (value * (PALETTES.len() - 1) as f32).round() as usize;
                    settings.set_palette(PALETTES[palette].name);
                    settings.set_heat(value);
                    settings.set_void(value);
                    let previous = VoxelImage::new(dim);
                    for mut content in all_content() {
                        let name = content.name().to_string();
                        let mut image = VoxelImage::new(dim);
                        let mut still = RenderState::from(&settings);
                        still.set_reset(true);
                        content.render(&still, 0.0, 0.04, &previous, &mut image);
                        let initial = pixels(&image);
                        assert_bounded(&image, &name);

                        still.set_reset(false);
                        content.render(&still, 10.0, 0.2, &previous, &mut image);
                        assert_eq!(initial, pixels(&image), "{name} must freeze at Flow 0");

                        settings.set_flow(1.0);
                        let mut moving = RenderState::from(&settings);
                        for frame in 0..50 {
                            content.render(&moving, frame as f32 * 0.1, 0.1, &previous, &mut image);
                            assert_bounded(&image, &name);
                        }
                        moving.set_reset(true);
                        content.render(&moving, 100.0, 0.1, &previous, &mut image);
                        assert_eq!(initial, pixels(&image), "{name} must restart on reset");
                        settings.set_flow(0.0);
                    }
                }
            }
        }
    }
}
