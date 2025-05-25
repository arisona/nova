use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::check_run_once;

use crate::app_state::{AppState, Status};
use crate::ethernet::Interface;
use crate::renderer::{RenderState, Renderer};
use crate::voxel_image::VoxelImage;

pub fn run_nova_hardware(app_state: Arc<Mutex<AppState>>, renderer: Renderer) {
    check_run_once!("Nova hardware driver already running.");

    log::info!("Starting Nova hardware driver.");

    app_state
        .lock()
        .unwrap()
        .set_status(Status::Ok("Nova hardware starting up.".to_string()));

    NovaHardware::new(app_state, renderer).run();
}

// Legacy note: the 50Hz loop runs in two cycles: one to send pixels and another to shift pixels on the hardware.
#[derive(Debug, PartialEq)]
enum SyncMode {
    SendPixels,
    ShiftPixels,
}

struct NovaHardware {
    app_state: Arc<Mutex<AppState>>,
    renderer: Renderer,

    ready_modules: HashMap<u8, Instant>,
}

impl NovaHardware {
    fn new(app_state: Arc<Mutex<AppState>>, renderer: Renderer) -> Self {
        NovaHardware {
            app_state,
            renderer,
            ready_modules: HashMap::new(),
        }
    }

    fn run(&mut self) {
        // Accept nova packets (0x810) sent to me (__MY_MAC__ will be replaced with the interface mac)
        let filter = format!("ether proto {ETHER_TYPE_NOVA} and ether dst __MY_MAC__");

        loop {
            let mut interface;

            // Retry loop for opening the interface
            loop {
                let (interface_name, modules) = {
                    let app_state = self.app_state.lock().unwrap();
                    (
                        app_state.ethernet_interface().to_string(),
                        app_state.modules().clone(),
                    )
                };

                match Interface::new(&interface_name, Some(filter.as_str())) {
                    Ok(iface) => {
                        log::info!("Opened interface {interface_name}.");

                        interface = iface;

                        // Sucessfully opened interface
                        self.app_state
                            .lock()
                            .unwrap()
                            .set_status(Status::Ok(format!(
                                "Resetting all modules at interface {interface_name}."
                            )));

                        let image = VoxelImage::new(self.app_state.lock().unwrap().dim());
                        self.reset_modules(&mut interface, &modules, &image);
                        log::info!("Module reset complete.");
                        break;
                    }
                    Err(err) => {
                        self.app_state
                            .lock()
                            .unwrap()
                            .set_status(Status::Err(format!(
                                "Cannot open interface {interface_name}.",
                            )));
                        log::warn!("Failed to open interface {interface_name}: {err}. Retrying...");
                        std::thread::sleep(INTERFACE_RETRY_PERIOD);
                    }
                }
            }

            // Main processing loop for opened interface
            self.ready_modules.clear();

            let mut sequence_number = 0;
            let mut sync_mode = SyncMode::SendPixels;
            let mut sync_time = Instant::now();
            let mut status_time = Instant::now();
            loop {
                // Make sure app_state is unlocked quickly otherwise webserver thread will starve
                let (interface_name, modules, mut render_state, flip) = {
                    let app_state = self.app_state.lock().unwrap();
                    (
                        app_state.ethernet_interface().to_string(),
                        app_state.modules().clone(),
                        RenderState::from(&app_state),
                        app_state.is_flip_vertical(),
                    )
                };

                if &interface_name != interface.name() {
                    log::info!("Interface changed to {interface_name}.");
                    // Return back to interface opening loop
                    break;
                }

                // Check if we need to request status from each module
                let now = Instant::now();
                if now >= status_time {
                    log::debug!("Requesting status from all modules.");
                    let packet = Self::nova_packet(
                        &BROADCAST_MAC,
                        &interface.address(),
                        NOVA_CMD_STATUS,
                        0,
                        false,
                    );
                    let _ = interface.send(packet);
                    status_time += STATUS_PERIOD;

                    // Update the app state with the latest module status
                    let num_modules = modules.len();
                    let num_ready_modules = self
                        .ready_modules
                        .values()
                        .filter(|t| now.duration_since(**t) < Duration::from_millis(5000))
                        .count();
                    log::debug!("Status update: {num_ready_modules} of {num_modules} ready.");
                    self.app_state.lock().unwrap().set_status(
                        if num_modules == num_ready_modules {
                            Status::Ok(format!(
                                "{num_ready_modules} of {num_modules} modules ready."
                            ))
                        } else {
                            Status::Err(format!(
                                "{num_ready_modules} of {num_modules} modules ready."
                            ))
                        },
                    );
                }

                // Handle received status packets
                while let Ok(packet) = interface.receive() {
                    self.handle_status_packet(&packet, &modules);
                }

                // Sync loop to hardware and check for render time budget
                let do_render = Self::wait_for_next_sync(&mut sync_time);

                // Send sync broadcast and do not send any pixel data if we are in shift mode
                let packet = Self::nova_packet(
                    &BROADCAST_MAC,
                    &interface.address(),
                    NOVA_CMD_SYNC,
                    sequence_number,
                    sync_mode == SyncMode::ShiftPixels,
                );
                let _ = interface.send(packet);

                // Send or shift pixels depending on the sync mode. Render only if we didn't miss the sync.
                // Legacy note: the original code used MODULE_QUEUE_SIZE = 4 to send rgb data with sequence number + 4 ahead.
                // This does not seem necessary, and we are just sending the current sequence number + 1.
                match sync_mode {
                    SyncMode::SendPixels => {
                        if do_render {
                            self.renderer.render(&mut render_state);
                        }
                        for (_, _, addr) in modules {
                            let packet = Self::udp_packet(
                                &interface.address(),
                                addr,
                                UDP_CMD_RGB,
                                sequence_number.wrapping_add(1),
                                self.renderer.image(),
                                flip,
                            );
                            let _ = interface.send(packet);
                        }
                        sync_mode = SyncMode::ShiftPixels;
                    }
                    SyncMode::ShiftPixels => {
                        sequence_number = sequence_number.wrapping_add(1);
                        sync_mode = SyncMode::SendPixels;
                    }
                }
            }
        }
        // Won't reach (we're running on the main thread)
    }

    fn handle_status_packet(&mut self, packet: &[u8], modules: &[(usize, usize, u8)]) {
        if packet.len() < NOVA_PACKET_LEN {
            log::warn!("Packet too short: {}", packet.len());
            return;
        }

        let command = packet[6 + 6 + 2 + 2];
        if command != NOVA_CMD_STATUS {
            log::warn!("Unexpected status packet command: {}", command);
            return;
        }

        if packet[20] != NOVA_IP[0] {
            log::warn!(
                "Unexpected IP address: {}.{}.{}.{}",
                packet[20],
                packet[21],
                packet[22],
                packet[23]
            );
            return;
        }

        let module_address = packet[23];

        // make sure this is actually coming from a module in our configuration
        if !modules.iter().any(|&(_, _, addr)| addr == module_address) {
            log::warn!("Received status from unknown module at address {module_address}");
            return;
        }

        log::debug!("Module at address {module_address} is ready.");
        self.ready_modules.insert(module_address, Instant::now());
    }

    fn reset_modules(
        &self,
        interface: &mut Interface,
        modules: &[(usize, usize, u8)],
        image: &VoxelImage,
    ) {
        // Legacy note: the logic here is taken from the original java code
        let mac = &interface.address();
        for _ in 0..4 {
            for &(_, _, address) in modules {
                let packet = Self::udp_packet(mac, address, UDP_CMD_RESET, 0, image, false);
                let _ = interface.send(packet);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        for &(_, _, address) in modules {
            let packet = Self::udp_packet(mac, address, UDP_CMD_AUTOID, 0, image, false);
            let _ = interface.send(packet);
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    fn wait_for_next_sync(sync_time: &mut Instant) -> bool {
        // Legacy note: Nova sync timing is critical
        let target = *sync_time + SYNC_PERIOD;
        *sync_time += SYNC_PERIOD;

        let now = Instant::now();
        if now > target {
            // Missed sync: do not render and wait for next sync
            log::warn!("Missed sync by {}us", (now - target).as_micros());
            return false;
        } else if now < target - SYNC_BUSY_WAIT_MARGIN {
            // Thread sleep wait for as much as possible
            let sleep_duration = target
                .duration_since(now)
                .saturating_sub(SYNC_BUSY_WAIT_MARGIN);
            if sleep_duration != Duration::ZERO {
                std::thread::sleep(sleep_duration);
            }
        }
        while Instant::now() < target {
            // Busy wait for the last bit to keep the timing
            std::thread::yield_now();
        }
        true
    }

    fn nova_packet(
        dst: &[u8; 6],
        src: &[u8; 6],
        command: u8,
        sequence_num: usize,
        shift_pixels: bool,
    ) -> [u8; NOVA_PACKET_LEN] {
        // doc not: the original code used to send status (running / stopped). does not seem necessary
        let status = 0;

        let mut packet = [0u8; NOVA_PACKET_LEN];

        // ethernet header
        packet[0..6].copy_from_slice(dst);
        packet[6..12].copy_from_slice(src);
        packet[12] = (ETHER_TYPE_NOVA >> 8) as u8;
        packet[13] = ETHER_TYPE_NOVA as u8;
        // sync packet data
        packet[14] = (NOVA_DATA_LEN >> 8) as u8;
        packet[15] = NOVA_DATA_LEN as u8;
        packet[16] = command;
        packet[17] = status;
        packet[18] = sequence_num as u8;
        packet[19] = if shift_pixels { 1 } else { 0 };

        packet
    }

    fn udp_packet(
        src: &[u8; 6],
        module_address: u8,
        command: u8,
        sequence_num: usize,
        image: &VoxelImage,
        flip: bool,
    ) -> [u8; UDP_PACKET_LEN] {
        let mut packet = [0u8; UDP_PACKET_LEN];
        // Ethernet header
        packet[0..5].copy_from_slice(&NOVA_MAC_PREFIX);
        packet[5] = module_address;
        packet[6..12].copy_from_slice(src);
        packet[12] = (ETHER_TYPE_IP >> 8) as u8;
        packet[13] = ETHER_TYPE_IP as u8;

        // IP header
        packet[14] = IP_VERSION | 0x05;
        packet[15] = 0x00; // ECN / DSCP -- orignal value was 0xf0, which doesn't really make sense
        let ip_packet_len = IP_HEADER_LEN + UDP_HEADER_LEN + UDP_CHAINED_DATA_LEN;
        packet[16] = (ip_packet_len >> 8) as u8;
        packet[17] = ip_packet_len as u8;
        packet[18] = 0x32; // ID field (constant 0x321c)
        packet[19] = 0x1c; // ID field (constant 0x321c)
        packet[20] = 0x40; // Fragment flags & offset
        packet[21] = 0x00; // Don't fragment, offset = 0
        packet[22] = 0x80; // TTL (0x80 is common default)
        packet[23] = 0x11; // UDP
        packet[24] = 0x00; // Checksum (calculated below)
        packet[25] = 0x00; // Checksum (calculated below)
        packet[26..30].copy_from_slice(&LOCAL_IP);
        packet[30..33].copy_from_slice(&NOVA_IP_PREFIX);
        packet[33] = module_address;
        let checksum = Self::ip_checksum(&packet[14..34]);
        packet[24] = (checksum >> 8) as u8;
        packet[25] = checksum as u8;

        // UDP header
        packet[34] = (LOCAL_UDP_PORT >> 8) as u8;
        packet[35] = LOCAL_UDP_PORT as u8;
        packet[36] = (NOVA_UDP_PORT >> 8) as u8;
        packet[37] = NOVA_UDP_PORT as u8;
        let udp_packet_len = UDP_HEADER_LEN + UDP_CHAINED_DATA_LEN;
        packet[38] = (udp_packet_len >> 8) as u8;
        packet[39] = udp_packet_len as u8;
        packet[40] = 0x00; // Checksum (zero for UDP)
        packet[41] = 0x00; // Checksum (zero for UDP)

        // UDP payload
        Self::fill_udp_payload(&mut packet, command, sequence_num, image, flip);

        packet
    }

    fn fill_udp_payload(
        packet: &mut [u8; UDP_PACKET_LEN],
        command: u8,
        sequence_num: usize,
        image: &VoxelImage,
        flip: bool,
    ) {
        for chain in 0..25 {
            let offset = UDP_PAYLOAD_OFFSET + chain * 44;
            packet[offset] = 0xc0;
            packet[offset + 1] = command;
            packet[offset + 2] = sequence_num as u8;
            packet[offset + 3] = chain as u8;

            let pixels = image.slice(0, chain); // Row index 0, chain index = Y

            for i in 0..10 {
                let base = offset + 4 + i * 4;
                let i = if flip { 9 - i } else { i };
                let r = (pixels[i * 3].clamp(0.0, 1.0) * 1023.0).round() as u32;
                let g = (pixels[i * 3 + 1].clamp(0.0, 1.0) * 1023.0).round() as u32;
                let b = (pixels[i * 3 + 2].clamp(0.0, 1.0) * 1023.0).round() as u32;

                let packed = (r << 20) | (g << 10) | b;
                packet[base] = (packed >> 24) as u8;
                packet[base + 1] = (packed >> 16) as u8;
                packet[base + 2] = (packed >> 8) as u8;
                packet[base + 3] = packed as u8;
            }
        }
    }

    fn ip_checksum(data: &[u8]) -> u16 {
        let mut sum = 0u32;
        for chunk in data.chunks(2) {
            let word = if chunk.len() == 2 {
                u16::from_be_bytes([chunk[0], chunk[1]]) as u32
            } else {
                (chunk[0] as u32) << 8
            };
            sum = sum.wrapping_add(word);
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        !sum as u16
    }
}

// Nova timing (20ms per frame, 50Hz)
const SYNC_PERIOD: Duration = Duration::from_millis(20);
const SYNC_BUSY_WAIT_MARGIN: Duration = Duration::from_millis(5);
const STATUS_PERIOD: Duration = Duration::from_millis(5000);
const INTERFACE_RETRY_PERIOD: Duration = Duration::from_millis(500);

// Ethernet / IP / UDP related constants
const ETHER_ADDR_LEN: usize = 6;
const ETHER_TYPE_LEN: usize = 2;
const ETHER_TYPE_IP: u16 = 0x0800;
const ETHER_TYPE_NOVA: u16 = 0x0810;

const NOVA_PACKET_LEN: usize = ETHER_ADDR_LEN + ETHER_ADDR_LEN + ETHER_TYPE_LEN + NOVA_DATA_LEN;
const NOVA_DATA_LEN: usize = 46;

const IP_VERSION: u8 = 0x40;
const IP_HEADER_LEN: usize = 20;

const UDP_HEADER_LEN: usize = 8;
const UDP_PACKET_LEN: usize = UDP_PAYLOAD_OFFSET + UDP_CHAINED_DATA_LEN;
const UDP_PAYLOAD_OFFSET: usize = 42;
const UDP_CHAINED_DATA_LEN: usize = 25 * (10 * 4 + 4); // 25 chains of 10 pixels, 4 bytes per pixel + 4 extra bytes

// Nova specific addresses and address prefixes
const BROADCAST_MAC: [u8; 6] = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const NOVA_MAC_PREFIX: [u8; 5] = [0x00, 0x20, 0xe3, 0x10, 0x00];

const LOCAL_IP: [u8; 4] = [127, 0, 0, 1];
const LOCAL_UDP_PORT: u16 = 1234;

const NOVA_IP: [u8; 4] = [192, 168, 1, 0];
const NOVA_IP_PREFIX: [u8; 3] = [192, 168, 1];
const NOVA_UDP_PORT: u16 = 3210;

// Nova packet command values
const NOVA_CMD_SYNC: u8 = 0x00;
// const NOVA_CMD_PLL: u8 = 0x01;
// const NOVA_CMD_START: u8 = 0x02;
// const NOVA_CMD_STOP: u8 = 0x03;
const NOVA_CMD_STATUS: u8 = 0x04;

// Status flags
// const NOVA_STATUS_STOPPED: u8 = 0x00;
// const NOVA_STATUS_RUNNING: u8 = 0x01;

// UDP packet command values
const UDP_CMD_RESET: u8 = 0x00;
const UDP_CMD_RGB: u8 = 0x02;
// const UDP_CMD_DOT_CORR: u8 = 0x04;
// const UDP_CMD_COLOR_CORR: u8 = 0x08;
// const UDP_CMD_BRIGHTNESS: u8 = 0x10;
// const UDP_CMD_OPMODE: u8 = 0x40;
const UDP_CMD_AUTOID: u8 = 0x70;

// FSS Power flags
// const FSS_POWER_RESTART_PENDING: u8 = 0x01;
// const FSS_POWER_PROGRAM_PENDING: u8 = 0x02;
// const FSS_POWER_ERASE_PENDING: u8 = 0x04;
// const FSS_POWER_PROGRAM_TIMEOUT: u8 = 0x08;
// const FSS_POWER_ERASE_TIMEOUT: u8 = 0x10;
// const FSS_POWER_OK1: u8 = 0x20;
// const FSS_POWER_OK2: u8 = 0x40;
// const FSS_POWER_OK3: u8 = 0x80;
