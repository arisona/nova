use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

mod common;
mod flux_capacitor;
mod light_cycle_grid;
mod spice_melange;
mod tannhauser_gate;
mod tears_in_rain;
mod the_shimmer;

const FLOW_SPEED_MULTIPLIER: f32 = 10.0;
/// Flow at which simulated modules run in real time.
const REAL_TIME_FLOW: f32 = 0.5;

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

/// Content modules in display order, alphabetical by name. Settings and the API refer to them by name: keep
/// names unique and free of commas.
pub fn all_content() -> Vec<Box<dyn Content>> {
    vec![
        Box::new(flux_capacitor::FluxCapacitor::new()),
        Box::new(light_cycle_grid::LightCycleGrid::new()),
        Box::new(spice_melange::SpiceMelange::new()),
        Box::new(tannhauser_gate::TannhauserGate::new()),
        Box::new(tears_in_rain::TearsInRain::new()),
        Box::new(the_shimmer::TheShimmer::new()),
    ]
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

/// Simulation time for stateful modules: advances the Flow-integrated phase like
/// `advance()` and returns the step in seconds of simulated time, which runs in real
/// time at Flow 0.5 and twice as fast at 1. Zero at Flow 0 and on reset.
pub fn advance_seconds(phase: &mut f64, state: &RenderState, delta: f32) -> f32 {
    let before = *phase;
    advance(phase, state, delta);
    if state.should_reset() {
        0.0
    } else {
        ((*phase - before) / (FLOW_SPEED_MULTIPLIER * REAL_TIME_FLOW) as f64) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::palettes::PALETTES;
    use glam::Vec3;

    #[test]
    fn content_is_in_alphabetical_order() {
        let names = all_content_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

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

    /// Bug-checking only (live evaluation is on the display). Renders every module at
    /// a form sweep to a PPM contact sheet: one row per module and point in time, the
    /// columns step Form from 0 to 1; each cell shows two modules side by side (x across,
    /// z up) through their middle slice.
    #[test]
    #[ignore = "writes a visual contact sheet for local bug-checking"]
    fn content_form_sweep_dump() {
        let palette = std::env::var("NOVA_PALETTE").unwrap_or_else(|_| PALETTES[0].name.into());
        let void: f32 = std::env::var("NOVA_VOID")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5);
        let heat: f32 = std::env::var("NOVA_HEAT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5);
        let forms: Vec<f32> = (0..=8).map(|step| step as f32 / 8.0).collect();
        let times = [2.0, 6.0, 15.0];
        let dim = (10usize, 5usize, 10usize);
        let cell = 10usize;
        let gap = 6usize;
        let rows = all_content().len() * times.len();
        let width = forms.len() * (dim.0 * cell + gap) + gap;
        let height = rows * (dim.2 * cell + gap) + gap;
        let mut bitmap = vec![12u8; width * height * 3];
        let previous = VoxelImage::new(dim);
        for (col, form) in forms.iter().enumerate() {
            let mut settings = AppState::default();
            assert!(settings.set_palette(&palette), "unknown palette {palette}");
            settings.set_heat(heat);
            settings.set_void(void);
            settings.set_form(*form);
            settings.set_flow(0.5);
            let mut state = RenderState::from(&settings);
            for (module, mut content) in all_content().into_iter().enumerate() {
                let mut image = VoxelImage::new(dim);
                state.set_reset(true);
                content.render(&state, 0.0, 0.04, &previous, &mut image);
                state.set_reset(false);
                let mut now = 0.0;
                for (t, &time) in times.iter().enumerate() {
                    while now < time {
                        content.render(&state, now, 0.04, &previous, &mut image);
                        now += 0.04;
                    }
                    let row = module * times.len() + t;
                    let y = dim.1 / 2;
                    for x in 0..dim.0 {
                        for z in 0..dim.2 {
                            let rgb = image.get(x, y, z).clamp(Vec3::ZERO, Vec3::ONE) * 255.0;
                            let px0 = gap + col * (dim.0 * cell + gap) + x * cell;
                            let py0 = gap + row * (dim.2 * cell + gap) + (dim.2 - 1 - z) * cell;
                            for dy in 0..cell - 1 {
                                for dx in 0..cell - 1 {
                                    let idx = ((py0 + dy) * width + (px0 + dx)) * 3;
                                    bitmap[idx] = rgb.x as u8;
                                    bitmap[idx + 1] = rgb.y as u8;
                                    bitmap[idx + 2] = rgb.z as u8;
                                }
                            }
                        }
                    }
                }
            }
        }
        let path = std::env::temp_dir().join("nova-content-sweep.ppm");
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        bytes.extend(bitmap);
        std::fs::write(&path, bytes).unwrap();
        println!(
            "Content sweep: {} (columns: form 0 .. 1, rows: modules × time)",
            path.display()
        );
    }

    /// Rough render cost per module, for comparing against the 20 ms Raspberry Pi
    /// budget (run with --release; the Pi is several times slower than a desktop).
    #[test]
    #[ignore = "prints render timings"]
    fn content_render_timing() {
        for dim in [(5, 5, 10), (10, 10, 10)] {
            let mut settings = AppState::default();
            settings.set_flow(1.0);
            let state = RenderState::from(&settings);
            let previous = VoxelImage::new(dim);
            for mut content in all_content() {
                let mut image = VoxelImage::new(dim);
                let frames = 500;
                let start = std::time::Instant::now();
                for frame in 0..frames {
                    content.render(&state, frame as f32 * 0.04, 0.04, &previous, &mut image);
                }
                let per_frame = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
                println!("{:>8} {dim:?}: {per_frame:.3} ms/frame", content.name());
            }
        }
    }
}
