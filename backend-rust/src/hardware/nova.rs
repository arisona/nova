use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pcap::Device;

use crate::app_state::AppState;
use crate::renderer::renderer::Renderer;
use crate::renderer::voxel_image::VoxelImage;

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn run_nova_hardware(state: Arc<Mutex<AppState>>, mut renderer: Renderer) {
    println!("Starting Nova hardware driver.");

    if RUNNING.swap(true, Ordering::Relaxed) {
        println!("Nova hardware driver already running.");
        return;
    }

    let frame_duration = std::time::Duration::from_millis(40); // 25 frames per second

    let mut ethernet_interface: String = String::new();
    let mut module0_address: String = String::new();

    let mut image = VoxelImage::new(state.lock().unwrap().dim());
    let mut time = std::time::Instant::now();
    loop {
        {
            let now = std::time::Instant::now();
            let delta = now.duration_since(time);
            renderer.render(&state.lock().unwrap(), &mut image, delta.as_secs_f32());
            time = std::time::Instant::now();
        }
        // TODO: we need to compensate the time used for rendering here
        std::thread::sleep(frame_duration);
    }
    // won't reach (we're running on the main thread)
}
