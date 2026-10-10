use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::app_state::{AppState, Status};
use crate::calibration::Calibration;
use crate::check_run_once;
use crate::ethernet::Interface;
use crate::renderer::{RenderState, Renderer};
use crate::voxel_image::VoxelImage;

/// Drives the Nova hardware on the calling thread. Returns when a shutdown is requested,
/// after resetting the modules so the display stays dark.
pub fn run(state: Arc<Mutex<AppState>>, renderer: Renderer) {
    check_run_once!("Nova hardware driver already running.");

    log::info!("Starting Nova hardware driver.");

    state
        .lock()
        .unwrap()
        .set_status(Status::Ok("Nova hardware starting up.".to_string()));

    NovaHardware::new(state, renderer).run();
}

// Legacy note: the 50Hz loop runs in two cycles: one to send pixels and another to shift pixels on the hardware.
#[derive(Debug, PartialEq)]
enum SyncMode {
    SendPixels,
    ShiftPixels,
}

struct NovaHardware {
    state: Arc<Mutex<AppState>>,
    renderer: Renderer,

    // When each module last responded to a status request
    last_reply: HashMap<u8, Instant>,
}

impl NovaHardware {
    fn new(state: Arc<Mutex<AppState>>, renderer: Renderer) -> Self {
        Self {
            state,
            renderer,
            last_reply: HashMap::new(),
        }
    }

    fn run(&mut self) {
        // Accept nova packets (0x810) sent to me (__MY_MAC__ will be replaced with the interface mac)
        let filter = format!("ether proto {ETHER_TYPE_NOVA} and ether dst __MY_MAC__");

        loop {
            let mut interface;
            let responding_modules_at_reset;

            // Retry loop for opening the interface
            loop {
                let (interface_name, modules, shutdown_requested) = {
                    let state = self.state.lock().unwrap();
                    (
                        state.ethernet_interface().to_string(),
                        state.modules().to_vec(),
                        state.shutdown_requested(),
                    )
                };

                if shutdown_requested {
                    log::info!("Shutting down without an open interface.");
                    return;
                }

                match Interface::new(&interface_name, Some(filter.as_str())) {
                    Ok(iface) => {
                        log::info!("Opened interface {interface_name}.");
                        interface = iface;
                        responding_modules_at_reset =
                            Self::responding_modules(&self.last_reply, &modules, Instant::now());
                        if !responding_modules_at_reset.is_empty() {
                            self.state.lock().unwrap().set_status(Status::Ok(format!(
                                "Resetting modules at interface {interface_name}."
                            )));
                            let image = VoxelImage::new(self.state.lock().unwrap().dim());
                            self.reset_modules(
                                &mut interface,
                                &responding_modules_at_reset,
                                &image,
                            );
                        }
                        break;
                    }
                    Err(err) => {
                        self.state.lock().unwrap().set_status(Status::Err(format!(
                            "Cannot open interface {interface_name}.",
                        )));
                        log::warn!("Failed to open interface {interface_name}: {err}. Retrying...");
                        std::thread::sleep(INTERFACE_RETRY_PERIOD);
                    }
                }
            }

            // Main processing loop for opened interface
            let mut sequence_number = 0;
            let mut sync_mode = SyncMode::SendPixels;
            let mut sync_time = Instant::now();
            let mut status_time = Instant::now();
            loop {
                // Make sure state is unlocked quickly otherwise webserver thread will starve
                let (
                    interface_name,
                    modules,
                    mut render_state,
                    flip,
                    calibration,
                    reset_requested,
                    shutdown_requested,
                ) = {
                    let mut state = self.state.lock().unwrap();
                    (
                        state.ethernet_interface().to_string(),
                        state.modules().to_vec(),
                        RenderState::from(&*state),
                        state.flip_vertical(),
                        state.calibration(),
                        state.take_hardware_reset_request(),
                        state.shutdown_requested(),
                    )
                };

                if shutdown_requested {
                    log::info!("Shutting down.");
                    let image = VoxelImage::new(self.state.lock().unwrap().dim());
                    self.reset_modules(&mut interface, &responding_modules_at_reset, &image);
                    return;
                }

                if reset_requested {
                    log::info!("Hardware reset requested.");
                    break;
                }

                if interface_name != interface.name() {
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
                    let num_responding =
                        Self::responding_modules(&self.last_reply, &modules, now).len();
                    log::debug!("Status update: {num_responding} of {num_modules} responding.");
                    self.state
                        .lock()
                        .unwrap()
                        .set_status(if num_modules == num_responding {
                            Status::Ok(format!("{num_responding} of {num_modules} modules ready."))
                        } else {
                            Status::Err(format!("{num_responding} of {num_modules} modules ready."))
                        });
                }

                // Handle received status packets
                while let Ok(packet) = interface.receive() {
                    self.handle_status_packet(&packet, &modules);
                }

                // A module that starts responding has not been reset yet, and one that stops may be
                // restarting. Configuration changes show up here too.
                let responding_modules =
                    Self::responding_modules(&self.last_reply, &modules, Instant::now());
                if responding_modules != responding_modules_at_reset {
                    log::info!(
                        "Responding modules changed from {:?} to {:?}.",
                        Self::addresses(&responding_modules_at_reset),
                        Self::addresses(&responding_modules)
                    );
                    // Reopen the interface to reset the responding modules
                    break;
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
                        for &module in &modules {
                            let packet = Self::udp_packet(
                                &interface.address(),
                                module,
                                UDP_CMD_RGB,
                                sequence_number.wrapping_add(1),
                                self.renderer.image(),
                                flip,
                                &calibration,
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
    }

    fn handle_status_packet(&mut self, packet: &[u8], modules: &[(usize, usize, u8)]) {
        if packet.len() < NOVA_PACKET_LEN {
            log::warn!("Packet too short: {}", packet.len());
            return;
        }

        let command = packet[6 + 6 + 2 + 2];
        if command != NOVA_CMD_STATUS {
            log::warn!("Unexpected status packet command: {command}");
            return;
        }

        if packet[20..23] != NOVA_IP_PREFIX {
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

        log::debug!("Module at address {module_address} responded.");
        self.last_reply.insert(module_address, Instant::now());
    }

    /// The configured modules that responded to a recent status request.
    fn responding_modules(
        last_reply: &HashMap<u8, Instant>,
        modules: &[(usize, usize, u8)],
        now: Instant,
    ) -> Vec<(usize, usize, u8)> {
        modules
            .iter()
            .copied()
            .filter(|(_, _, address)| {
                last_reply
                    .get(address)
                    .is_some_and(|&t| now.duration_since(t) < RESPONSE_TIMEOUT)
            })
            .collect()
    }

    fn addresses(modules: &[(usize, usize, u8)]) -> Vec<u8> {
        modules.iter().map(|&(_, _, address)| address).collect()
    }

    fn reset_modules(
        &self,
        interface: &mut Interface,
        modules: &[(usize, usize, u8)],
        image: &VoxelImage,
    ) {
        if modules.is_empty() {
            return;
        }
        log::info!("Resetting modules {:?}.", Self::addresses(modules));
        // Legacy note: the logic here is taken from the original java code
        let mac = &interface.address();
        for _ in 0..4 {
            for &module in modules {
                let packet = Self::udp_packet(
                    mac,
                    module,
                    UDP_CMD_RESET,
                    0,
                    image,
                    false,
                    &Calibration::IDENTITY,
                );
                let _ = interface.send(packet);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        for &module in modules {
            let packet = Self::udp_packet(
                mac,
                module,
                UDP_CMD_AUTOID,
                0,
                image,
                false,
                &Calibration::IDENTITY,
            );
            let _ = interface.send(packet);
        }
        std::thread::sleep(Duration::from_millis(200));
        log::info!("Module reset complete.");
    }

    /// Waits for the next sync slot, every SYNC_PERIOD from the first. Returns false if the
    /// loop fell behind: the missed slots are skipped, never sent late, and the caller does
    /// not render this time.
    fn wait_for_next_sync(sync_time: &mut Instant) -> bool {
        // Legacy note: Nova sync timing is critical
        *sync_time += SYNC_PERIOD;

        let now = Instant::now();
        let mut on_time = true;
        if now > *sync_time {
            // Missed sync: skip to the next slot, so syncs stay SYNC_PERIOD apart
            let late = now - *sync_time;
            let mut missed = 0;
            while *sync_time < now {
                *sync_time += SYNC_PERIOD;
                missed += 1;
            }
            log::warn!(
                "Missed {missed} sync(s), the first by {}us.",
                late.as_micros()
            );
            on_time = false;
        }

        if *sync_time > now + SYNC_BUSY_WAIT_MARGIN {
            // Thread sleep wait for as much as possible
            std::thread::sleep(*sync_time - now - SYNC_BUSY_WAIT_MARGIN);
        }
        while Instant::now() < *sync_time {
            // Busy wait for the last bit to keep the timing
            std::thread::yield_now();
        }
        on_time
    }

    fn nova_packet(
        dst: &[u8; 6],
        src: &[u8; 6],
        command: u8,
        sequence_num: usize,
        shift_pixels: bool,
    ) -> [u8; NOVA_PACKET_LEN] {
        // Legacy note: the original code used to send status (running / stopped). Does not seem necessary
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

    /// A data packet for the module at grid position (x, y) with the given address.
    fn udp_packet(
        src: &[u8; 6],
        module: (usize, usize, u8),
        command: u8,
        sequence_num: usize,
        image: &VoxelImage,
        flip: bool,
        calibration: &Calibration,
    ) -> [u8; UDP_PACKET_LEN] {
        let (module_x, module_y, module_address) = module;
        let mut packet = [0u8; UDP_PACKET_LEN];
        // Ethernet header
        packet[0..5].copy_from_slice(&NOVA_MAC_PREFIX);
        packet[5] = module_address;
        packet[6..12].copy_from_slice(src);
        packet[12] = (ETHER_TYPE_IP >> 8) as u8;
        packet[13] = ETHER_TYPE_IP as u8;

        // IP header
        packet[14] = IP_VERSION | 0x05;
        packet[15] = 0x00; // ECN / DSCP -- original value was 0xf0, which doesn't really make sense
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
        let origin = (
            module_x * AppState::MODULE_X_RES,
            module_y * AppState::MODULE_Y_RES,
        );
        Self::fill_udp_payload(
            &mut packet,
            command,
            sequence_num,
            image,
            origin,
            flip,
            calibration,
        );

        packet
    }

    /// One chain per voxel column of the module whose corner is at `origin` in the image.
    /// Chains run through the module's columns y-first: chain = x · MODULE_Y_RES + y.
    fn fill_udp_payload(
        packet: &mut [u8; UDP_PACKET_LEN],
        command: u8,
        sequence_num: usize,
        image: &VoxelImage,
        origin: (usize, usize),
        flip: bool,
        calibration: &Calibration,
    ) {
        for chain in 0..CHAINS {
            let offset = UDP_PAYLOAD_OFFSET + chain * CHAIN_DATA_LEN;
            packet[offset] = 0xc0;
            packet[offset + 1] = command;
            packet[offset + 2] = sequence_num as u8;
            packet[offset + 3] = chain as u8;

            let x = origin.0 + chain / AppState::MODULE_Y_RES;
            let y = origin.1 + chain % AppState::MODULE_Y_RES;
            let pixels = image.slice(x, y);

            for i in 0..CHAIN_LEN {
                let base = offset + 4 + i * 4;
                let i = if flip { CHAIN_LEN - 1 - i } else { i };
                let r = (calibration.apply(0, pixels[i * 3]) * 1023.0).round() as u32;
                let g = (calibration.apply(1, pixels[i * 3 + 1]) * 1023.0).round() as u32;
                let b = (calibration.apply(2, pixels[i * 3 + 2]) * 1023.0).round() as u32;

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
// A module counts as responding if it responded to the latest status request. The margin over
// STATUS_PERIOD absorbs loop jitter, which would otherwise briefly drop a module and reset all.
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(7500);
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
const UDP_CHAINED_DATA_LEN: usize = CHAINS * CHAIN_DATA_LEN;

// One chain per voxel column of a module, 4 bytes per pixel plus a 4-byte chain header
const CHAINS: usize = AppState::MODULE_X_RES * AppState::MODULE_Y_RES;
const CHAIN_LEN: usize = AppState::MODULE_Z_RES;
const CHAIN_DATA_LEN: usize = 4 + CHAIN_LEN * 4;

// Nova specific addresses and address prefixes
const BROADCAST_MAC: [u8; 6] = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const NOVA_MAC_PREFIX: [u8; 5] = [0x00, 0x20, 0xe3, 0x10, 0x00];

const LOCAL_IP: [u8; 4] = [127, 0, 0, 1];
const LOCAL_UDP_PORT: u16 = 1234;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// An image whose voxels all have distinct colors.
    fn numbered_image(dim: (usize, usize, usize)) -> VoxelImage {
        let mut image = VoxelImage::new(dim);
        let count = (dim.0 * dim.1 * dim.2) as f32;
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    let n = ((x * dim.1 + y) * dim.2 + z) as f32 / count;
                    image.set(x, y, z, glam::vec3(n, 1.0 - n, 0.5));
                }
            }
        }
        image
    }

    fn payload(module: (usize, usize, u8), image: &VoxelImage, flip: bool) -> Vec<u8> {
        let packet = NovaHardware::udp_packet(
            &[0; 6],
            module,
            UDP_CMD_RGB,
            7,
            image,
            flip,
            &Calibration::IDENTITY,
        );
        packet[UDP_PAYLOAD_OFFSET..].to_vec()
    }

    #[test]
    fn module_payload_reads_its_own_grid_cell() {
        // Two modules side by side along x, and one behind the first along y.
        let dim = (10, 10, AppState::MODULE_Z_RES);
        let image = numbered_image(dim);
        for flip in [false, true] {
            let mut cell = VoxelImage::new((5, 5, AppState::MODULE_Z_RES));
            for (module_x, module_y) in [(0, 0), (1, 0), (0, 1)] {
                for x in 0..5 {
                    for y in 0..5 {
                        for z in 0..AppState::MODULE_Z_RES {
                            let voxel = image.get(module_x * 5 + x, module_y * 5 + y, z);
                            cell.set(x, y, z, voxel);
                        }
                    }
                }
                assert_eq!(
                    payload((module_x, module_y, 1), &image, flip),
                    payload((0, 0, 1), &cell, flip),
                );
            }
        }
    }

    #[test]
    fn responding_modules_answered_recently() {
        let start = Instant::now();
        let modules = [(0, 0, 1), (1, 0, 2)];
        let mut last_reply = HashMap::new();
        let responding = |last_reply: &HashMap<u8, Instant>, t| {
            NovaHardware::addresses(&NovaHardware::responding_modules(last_reply, &modules, t))
        };

        // Server first: nothing answers, nothing responds
        assert!(responding(&last_reply, start).is_empty());

        // A module starts responding
        last_reply.insert(1, start);
        assert_eq!(responding(&last_reply, start), [1]);

        // It keeps responding until the next reply, even if that comes late
        let late = start + STATUS_PERIOD + Duration::from_millis(100);
        assert_eq!(responding(&last_reply, late), [1]);

        // A missed reply stops it responding
        assert!(responding(&last_reply, start + STATUS_PERIOD * 2).is_empty());

        // Modules no longer configured do not count
        last_reply.insert(3, start);
        assert_eq!(responding(&last_reply, start), [1]);
    }

    #[test]
    fn syncs_on_time_keep_their_slots() {
        let start = Instant::now();
        let mut sync_time = start;
        assert!(NovaHardware::wait_for_next_sync(&mut sync_time));
        assert_eq!(sync_time, start + SYNC_PERIOD);
        assert!(Instant::now() >= sync_time);
    }

    #[test]
    fn missed_syncs_are_skipped_not_sent_late() {
        // The loop stalled for three and a half periods
        let start = Instant::now();
        let mut sync_time = start - SYNC_PERIOD * 7 / 2;
        let first = sync_time;
        assert!(!NovaHardware::wait_for_next_sync(&mut sync_time));
        // The next sync is the first slot after the stall, still on the schedule
        assert_eq!(sync_time, first + SYNC_PERIOD * 4);
        assert!(Instant::now() >= sync_time);
        // and the one after that follows a full period later
        assert!(NovaHardware::wait_for_next_sync(&mut sync_time));
        assert_eq!(sync_time, first + SYNC_PERIOD * 5);
    }

    #[test]
    fn single_module_payload_is_unchanged() {
        // The single-module layout: chain c is the c-th column of the flat image.
        let image = numbered_image((5, 5, AppState::MODULE_Z_RES));
        let payload = payload((0, 0, 1), &image, true);
        for chain in 0..CHAINS {
            let data = &payload[chain * CHAIN_DATA_LEN..][..CHAIN_DATA_LEN];
            assert_eq!(data[..4], [0xc0, UDP_CMD_RGB, 7, chain as u8]);
            let pixels = image.slice(0, chain);
            for i in 0..CHAIN_LEN {
                let pixel = &pixels[(CHAIN_LEN - 1 - i) * 3..][..3];
                let [r, g, b] = [0, 1, 2].map(|c| (pixel[c] * 1023.0).round() as u32);
                let packed = (r << 20) | (g << 10) | b;
                assert_eq!(data[4 + i * 4..][..4], packed.to_be_bytes());
            }
        }
    }
}
