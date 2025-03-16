#![cfg(not(feature = "use_simulator"))]

use std::sync::{Arc, Mutex};

use super::super::app_state::AppState;
use super::super::renderer::renderer::Renderer;
use super::super::renderer::voxel_image::VoxelImage;

pub fn run_nova_hardware(state: Arc<Mutex<AppState>>, renderer: Arc<Renderer>) {
    println!("Starting Nova hardware driver.");

    let frame_duration = std::time::Duration::from_millis(40); // 25 frames per second

    let mut image = VoxelImage::new(8, 8, 8);
    let mut time = std::time::Instant::now();
    loop {
        {
            let now = std::time::Instant::now();
            let delta = now.duration_since(time);
            renderer.render_frame(&state.lock().unwrap(), &mut image, delta.as_secs_f32());
            time = std::time::Instant::now();
        }
        // TODO: we need to compensate the time used for rendering here
        std::thread::sleep(frame_duration);
    }
    // won't reach (we're running on the main thread)
}
