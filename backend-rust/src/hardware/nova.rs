use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

use crate::check_run_once;

use crate::app_state::AppState;
use crate::renderer::renderer::{RenderState, Renderer};
use crate::renderer::voxel_image::VoxelImage;

use super::ethernet::Interface;

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn run_nova_hardware(app_state: Arc<Mutex<AppState>>, mut renderer: Renderer) {
    println!("Starting Nova hardware driver.");

    check_run_once!(RUNNING, "Nova hardware driver already running.");

    let frame_duration = std::time::Duration::from_millis(40); // 25 frames per second
    let mut time = std::time::Instant::now();
    let mut interface: Option<Interface> = None;
    let mut image = VoxelImage::new(app_state.lock().unwrap().dim());

    loop {
        // Make sure app_state is unlocked quickly otherwise webserver thread will starve
        let (interface_name, render_state, modules) = {
            let app_state = app_state.lock().unwrap();
            (
                app_state.ethernet_interface().to_string(),
                RenderState::from(&app_state),
                app_state.modules().clone(),
            )
        };

        // Check if the interface has changed
        if interface
            .as_ref()
            .is_some_and(|i| i.name() != &interface_name)
        {
            interface = None;
        }

        // Check if we need to open the interface
        if interface.is_none() {
            // notes:
            // __MY_MAC__ will be replaced with the interfaces mac, once it's known
            // it doesn't seem we need to filter for broadcast packets
            let filter = format!(
                "ether proto {} and ether dst __MY_MAC__ or ether broadcast",
                ETHERTYPE_NOVA_SYNC
            );
            match Interface::new(
                &interface_name,
                Some(filter.as_str()),
                Some(handle_sync_packet),
            ) {
                Ok(iface) => {
                    println!("Opened interface {}", interface_name);
                    interface = Some(iface);
                }
                Err(err) => {
                    eprintln!(
                        "Failed to open interface {}: {}. Retrying...",
                        interface_name, err
                    );
                    interface = None;
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                }
            }
        }

        assert!(
            interface.is_some(),
            "Interface should be opened at this point."
        );

        // Check if the interface is open *and* not in error state
        // TODO

        // Check if we need to request status from each module
        // TODO

        // Main processing loop if everything is full operational
        //println!("Processing frame...");
        let delta = std::time::Instant::now().duration_since(time);
        renderer.render(&render_state, &mut image, delta.as_secs_f32());

        create_and_queue_packets(interface.as_ref().unwrap(), &modules, &image);
        time = std::time::Instant::now();

        // TODO: we need to compensate the time used for rendering here
        std::thread::sleep(frame_duration);
    }
    // won't reach (we're running on the main thread)
}

fn create_and_queue_packets(
    interface: &Interface,
    modules: &[(usize, usize, u8)],
    image: &VoxelImage,
) {
    let mut module_buffer =
        vec![0u8; AppState::MODULE_X_RES * AppState::MODULE_Y_RES * AppState::MODULE_Z_RES * 4];

    for &(mx, my, addr) in modules {
        let mut offset = 0;

        for x in 0..AppState::MODULE_X_RES {
            let x = mx * AppState::MODULE_X_RES + x;
            for y in 0..AppState::MODULE_Y_RES {
                let y = my * AppState::MODULE_Y_RES + y;
                for z in 0..AppState::MODULE_Z_RES {
                    let (r, g, b) = image.get(x, y, z);
                    let r = (r.clamp(0.0, 1.0) * 1023.0).round() as u32;
                    let g = (g.clamp(0.0, 1.0) * 1023.0).round() as u32;
                    let b = (b.clamp(0.0, 1.0) * 1023.0).round() as u32;

                    let packed = (r << 20) | (g << 10) | b;

                    module_buffer[offset + 0] = (packed >> 24) as u8;
                    module_buffer[offset + 1] = (packed >> 16) as u8;
                    module_buffer[offset + 2] = (packed >> 8) as u8;
                    module_buffer[offset + 3] = packed as u8;

                    offset += 4;
                }
            }
        }

        // TODO: Send `module_buffer` for this module to the hardware using `interface` and `addr`.
    }
}

static NOVA_IS_RUNNING: AtomicBool = AtomicBool::new(false);
static NOVA_SEQ_NUM: AtomicUsize = AtomicUsize::new(0);

fn handle_sync_packet(packet: &[u8]) -> Option<Vec<u8>> {
    // Ignore if destination is broadcast
    if &packet[0..6] == [0xff; 6] {
        return None;
    }

    // Ignore if ethertype is not NOVA_SYNC
    let ethertype = u16::from_be_bytes([packet[12], packet[13]]);
    if ethertype != ETHERTYPE_NOVA_SYNC {
        return None;
    }

    if packet.len() < 17 {
        return None;
    }

    let mut status_packet = [0u8; 6 + 6 + 2 + STATUS_DATA_LEN];
    status_packet[0..6].copy_from_slice(&SYNC_ADDR);
    status_packet[6..12].copy_from_slice(&packet[0..6]);
    status_packet[12] = (ETHERTYPE_NOVA_SYNC >> 8) as u8;
    status_packet[13] = ETHERTYPE_NOVA_SYNC as u8;
    status_packet[14] = (STATUS_DATA_LEN >> 8) as u8;
    status_packet[15] = STATUS_DATA_LEN as u8;
    status_packet[16] = CMD_STATUS;

    let command = packet[6 + 6 + 2 + 2];
    match command {
        CMD_START => {
            NOVA_IS_RUNNING.store(true, std::sync::atomic::Ordering::Relaxed);
            NOVA_SEQ_NUM.store(0, std::sync::atomic::Ordering::Relaxed);
            status_packet[17] = STATUS_RUNNING;
            status_packet[18] = (511 >> 1) as u8;
            status_packet[19] = (511 & 1) as u8;
        }
        CMD_STOP => {
            NOVA_IS_RUNNING.store(false, std::sync::atomic::Ordering::Relaxed);
            NOVA_SEQ_NUM.store(0, std::sync::atomic::Ordering::Relaxed);
            status_packet[17] = STATUS_STOPPED;
            status_packet[18] = 0;
            status_packet[19] = 0;
        }
        CMD_STATUS => {
            if packet[20] == IP[0] {
                // TODO: set alive status of this module
                // we need to find a way where to put this and make it available to the web server
                let module_address = packet[23];
            } else {
                // send status reply: running ? STAT_RUN : STAT_STOP, seq_num
                let seq = NOVA_SEQ_NUM.load(std::sync::atomic::Ordering::Relaxed);
                status_packet[17] = if NOVA_IS_RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                    STATUS_RUNNING
                } else {
                    STATUS_STOPPED
                };
                status_packet[18] = (seq >> 1) as u8;
                status_packet[19] = (seq & 1) as u8;
            }
        }
        _ => {
            eprintln!("Unknown command: {}", command);
        }
    }
    // TODO: return status reply to be sent
    return None;
}

// ethernet related constants
const ADDR_LEN: usize = 6;

const ETHERTYPE_LEN: usize = 2;
const ETHERTYPE_IP: u16 = 0x0800;
const ETHERTYPE_NOVA_SYNC: u16 = 0x0810;

const UDP_PAYLOAD_OFFSET: usize = 42;

const IP: [u8; 4] = [192, 168, 1, 0];
const IP_PREFIX: &str = "192.168.1.";

const SYNC_ADDR: [u8; 6] = [0x00, 0x20, 0xE3, 0x10, 0x01, 0x00];
const DMUX_ADDR: [u8; 6] = [0x00, 0x20, 0xE3, 0x10, 0x00, 0x00];

const STATUS_DATA_LEN: usize = 46;

// Sync command values
const CMD_SYNC: u8 = 0x00;
const CMD_PLL: u8 = 0x01;
const CMD_START: u8 = 0x02;
const CMD_STOP: u8 = 0x03;
const CMD_STATUS: u8 = 0x04;

// DMUX command values
const CMD_RESET: u8 = 0x00;
const CMD_RGB: u8 = 0x02;
const CMD_DOT_CORR: u8 = 0x04;
const CMD_COLOR_CORR: u8 = 0x08;
const CMD_BRIGHTNESS: u8 = 0x10;
const CMD_OPMODE: u8 = 0x40;
const CMD_AUTOID: u8 = 0x70;

// Status flags
const STATUS_STOPPED: u8 = 0x00;
const STATUS_RUNNING: u8 = 0x01;

// FSS Power flags
const FSS_POWER_OK3: u8 = 0x80;
const FSS_POWER_OK2: u8 = 0x40;
const FSS_POWER_OK1: u8 = 0x20;
const FSS_POWER_ERASE_TIMEOUT: u8 = 0x10;
const FSS_POWER_PROGRAM_TIMEOUT: u8 = 0x08;
const FSS_POWER_ERASE_PENDING: u8 = 0x04;
const FSS_POWER_PROGRAM_PENDING: u8 = 0x02;
const FSS_POWER_RESTART_PENDING: u8 = 0x01;
