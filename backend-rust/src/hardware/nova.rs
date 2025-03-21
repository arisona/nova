use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::check_run_once;

use crate::app_state::AppState;
use crate::renderer::renderer::{RenderState, Renderer};
use crate::renderer::voxel_image::VoxelImage;

use super::ethernet::Interface;

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn run_nova_hardware(state: Arc<Mutex<AppState>>, mut renderer: Renderer) {
    println!("Starting Nova hardware driver.");

    check_run_once!(RUNNING, "Nova hardware driver already running.");

    let frame_duration = std::time::Duration::from_millis(40); // 25 frames per second
    let mut image = VoxelImage::new(state.lock().unwrap().dim());

    loop {
        let interface;

        // Retry loop for opening the interface
        let mut interface_name = String::new();
        loop {
            let new_name = state.lock().unwrap().ethernet_interface().to_string();
            if new_name != interface_name {
                println!("Opening interface {}", new_name);
                interface_name = new_name;
                match open_interface(&interface_name) {
                    Ok(iface) => {
                        interface = iface;
                        break;
                    }
                    Err(err) => {
                        println!(
                            "Failed to open interface {}: {}. Retrying...",
                            interface_name, err
                        );
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Main processing loop for opened interface
        let mut time = std::time::Instant::now();
        loop {
            {
                let app_state = state.lock().unwrap();
                if app_state.ethernet_interface() != interface.name() {
                    println!("Interface changed, restarting.");
                    break;
                }

                let state = RenderState::from(&app_state);
                let now = std::time::Instant::now();
                let delta = now.duration_since(time);
                renderer.render(&state, &mut image, delta.as_secs_f32());
                time = std::time::Instant::now();
            }

            // TODO: we need to compensate the time used for rendering here
            std::thread::sleep(frame_duration);
        }
    }
    // won't reach (we're running on the main thread)
}

const ETHERTYPE_IP: u16 = 0x0800;
const ETHERTYPE_NOVA_SYNC: u16 = 0x0810;

fn open_interface(name: &str) -> Result<Interface, Box<dyn std::error::Error>> {
    let mut interface = Interface::new(name)?;
    let filter = format!(
        "ether proto {} and ether dst {} or ether broadcast",
        ETHERTYPE_NOVA_SYNC,
        interface.mac_address_as_string()
    );
    interface.set_filter(&filter)?;
    interface.set_capture(handle_packet)?;
    Ok(interface)
}

fn handle_packet(data: &[u8]) {
    let ethertype = u16::from_be_bytes([data[12], data[13]]);
    match ethertype {
        ETHERTYPE_NOVA_SYNC => {
            println!("Received Nova sync packet");
        }
        _ => {
            println!("Received unknown packet with ethertype 0x{:04x}", ethertype);
        }
    }
}
