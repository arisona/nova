use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::check_run_once;

use crate::app_state::AppState;
use crate::renderer::{RenderState, Renderer};
use crate::voxel_image::VoxelImage;

use super::ethernet::Interface;

pub fn run_nova_hardware(app_state: Arc<Mutex<AppState>>, renderer: Renderer) {
    check_run_once!("Nova hardware driver already running.");

    println!("Starting Nova hardware driver.");

    app_state
        .lock()
        .unwrap()
        .set_status((false, "Nova hardware starting up."));

    NovaHardware::new(app_state, renderer).run();
}

struct NovaHardware {
    app_state: Arc<Mutex<AppState>>,
    renderer: Renderer,

    is_running: bool,
    sequence_number: usize,

    module_status: HashMap<u8, Instant>,

    // TODO: once we have this all working: do we really need these?
    // (likely it's not needed, since we're not expecting any IP replies from Nova)
    local_ip: [u8; 4],
    local_port: u16,
}

impl NovaHardware {
    fn new(app_state: Arc<Mutex<AppState>>, renderer: Renderer) -> Self {
        let (local_ip, local_port) = Self::local_ip_and_port().unwrap();
        NovaHardware {
            app_state,
            renderer,
            is_running: false,
            sequence_number: 0,
            module_status: HashMap::new(),
            local_ip,
            local_port,
        }
    }

    fn run(&mut self) {
        // filter: accept nova packets (0x810) sent to me, and all broadcast packets
        // TODO:
        // __MY_MAC__ will be replaced with the interfaces mac, once it's known
        // it doesn't seem we need to filter for broadcast packets
        let filter = format!(
            "(ether proto {ETHER_TYPE_NOVA_SYNC} and ether dst __MY_MAC__) or ether broadcast"
        );

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
                        println!("Opened interface {interface_name}.");

                        interface = iface;

                        // Sucessfully opened interface
                        self.app_state.lock().unwrap().set_status((
                            false,
                            &format!("Resetting all modules at interface {interface_name}."),
                        ));

                        let image = VoxelImage::new(self.app_state.lock().unwrap().dim());
                        self.reset_modules(&mut interface, &modules, &image);
                        println!("Module reset complete.");
                        break;
                    }
                    Err(err) => {
                        self.app_state.lock().unwrap().set_status((
                            false,
                            &format!("Cannot open interface {interface_name}."),
                        ));
                        println!("Failed to open interface {interface_name}: {err}. Retrying...");
                        std::thread::sleep(INTERFACE_RETRY_PERIOD);
                    }
                }
            }

            // Main processing loop for opened interface
            let mut sync_time = Instant::now();
            let mut status_time = Instant::now();
            loop {
                // Make sure app_state is unlocked quickly otherwise webserver thread will starve
                // TODO: implement flip
                let (mut render_state, flip, interface_name, modules) = {
                    let app_state = self.app_state.lock().unwrap();
                    (
                        RenderState::from(&app_state),
                        app_state.is_flip_vertical(),
                        app_state.ethernet_interface().to_string(),
                        app_state.modules().clone(),
                    )
                };

                if &interface_name != interface.name() {
                    println!("Interface changed to {interface_name}.");
                    // return back to interface opening loop
                    break;
                }

                // Check if we need to request status from each module
                let now = Instant::now();
                if now >= status_time {
                    let packet =
                        Self::nova_packet(&BROADCAST_ADDR, &interface.address(), CMD_STATUS, 0, 0);
                    let _ = interface.send(packet);

                    let num_modules = modules.len();
                    let num_ready_modules = self
                        .module_status
                        .values()
                        .filter(|t| now.duration_since(**t) < Duration::from_millis(5000))
                        .count();
                    println!("Status update: {num_ready_modules} of {num_modules} ready.");
                    self.app_state.lock().unwrap().set_status((
                        num_modules == num_ready_modules,
                        &format!("{num_ready_modules} of {num_modules} modules ready."),
                    ));

                    status_time += STATUS_PERIOD;
                }

                // Handle received status packets
                while let Ok(packet) = interface.receive() {
                    self.handle_status_packet(&mut interface, &packet);
                }

                // Sync loop to hardware and check for render time budget
                let do_render = Self::wait_for_next_sync(&mut sync_time);

                // Send sync broadcast
                // TODO: this is inconsistent with the Java code, which sends sync/running or pll/stopped depending on run status
                let packet = Self::nova_packet(
                    &BROADCAST_ADDR,
                    &SYNC_ADDR,
                    CMD_SYNC,
                    STATUS_RUNNING,
                    self.sequence_number,
                );
                let _ = interface.send(packet);

                for (_, _, addr) in modules {
                    let packet = Self::udp_packet(
                        &interface.address(),
                        &self.local_ip,
                        self.local_port,
                        addr,
                        CMD_RGB,
                        self.sequence_number,
                        self.renderer.image(),
                    );
                    let _ = interface.send(packet);
                }

                if do_render {
                    self.renderer.render(&mut render_state);
                }

                // TODO: we need to review again how to deal with sequence numbers
                self.sequence_number = self.sequence_number.wrapping_add(1);
            }
        }
        // won't reach (we're running on the main thread)
    }

    fn handle_status_packet(&mut self, interface: &mut Interface, packet: &[u8]) {
        // Ignore if destination is broadcast
        // TODO: remove broadcast in filter, and forget about this
        if packet[0..6] == [0xff; 6] {
            return;
        }

        // Ignore if ethertype is not NOVA_SYNC
        // TODO: same, ignore, since our filter guarantees that this is a NOVA_SYNC packet
        let ethertype = u16::from_be_bytes([packet[12], packet[13]]);
        if ethertype != ETHER_TYPE_NOVA_SYNC {
            return;
        }

        if packet.len() < 17 {
            return;
        }

        // TODO: src should actually be my ethernet address, shouldn't it?
        let dst = &SYNC_ADDR;
        let src = &packet[0..6].try_into().unwrap();

        // TODO: what's unclear here, how does this work with multiple modules? will each of them send a
        // status packet? And if so, how do we handle this correctly?
        let command = packet[6 + 6 + 2 + 2];
        let reply;
        match command {
            CMD_START => {
                self.is_running = true;
                self.sequence_number = 0;
                reply = Self::nova_packet(dst, src, CMD_STATUS, STATUS_RUNNING, 511);
            }
            CMD_STOP => {
                self.is_running = false;
                self.sequence_number = 0;
                reply = Self::nova_packet(dst, src, CMD_STATUS, STATUS_STOPPED, 0);
            }
            CMD_STATUS => {
                if packet[20] == NOVA_IP[0] {
                    let module_address = packet[23];
                    println!("Module {module_address} is alive.");
                    self.module_status.insert(module_address, Instant::now());
                    return;
                } else {
                    reply = Self::nova_packet(
                        dst,
                        src,
                        CMD_STATUS,
                        if self.is_running {
                            STATUS_RUNNING
                        } else {
                            STATUS_STOPPED
                        },
                        self.sequence_number,
                    )
                }
            }
            _ => {
                eprintln!("Unknown command: {command}");
                return;
            }
        }
        let _ = interface.send(reply);
    }

    fn reset_modules(
        &mut self,
        interface: &mut Interface,
        modules: &[(usize, usize, u8)],
        image: &VoxelImage,
    ) {
        // cleanup state
        self.is_running = false;
        self.sequence_number = 0;
        self.module_status.clear();

        // the logic here is taken from the original java code, don't question it for now
        for _ in 0..4 {
            for &(_, _, address) in modules {
                let packet = Self::udp_packet(
                    &interface.address(),
                    &self.local_ip,
                    self.local_port,
                    address,
                    CMD_RESET,
                    0,
                    image,
                );
                let _ = interface.send(packet);
            }
            std::thread::sleep(Duration::from_millis(400));
        }
        for &(_, _, address) in modules {
            let packet = Self::udp_packet(
                &interface.address(),
                &self.local_ip,
                self.local_port,
                address,
                CMD_AUTOID,
                0,
                image,
            );
            let _ = interface.send(packet);
        }
        std::thread::sleep(Duration::from_millis(1000));
    }

    fn wait_for_next_sync(sync_time: &mut Instant) -> bool {
        // this needs testing as the Nova timing is quite critical
        let target = *sync_time + SYNC_PERIOD;
        *sync_time += SYNC_PERIOD;

        let now = Instant::now();
        if now > target {
            // missed sync: do not render and wait for next sync
            println!("Missed sync by {}us", (now - target).as_micros());
            return false;
        } else if now < target - SYNC_BUSY_WAIT_MARGIN {
            // os wait for as much as possible
            let sleep_duration = target
                .duration_since(now)
                .saturating_sub(SYNC_BUSY_WAIT_MARGIN);
            if sleep_duration != Duration::ZERO {
                std::thread::sleep(sleep_duration);
            }
        }
        while Instant::now() < target {
            // busy wait for the last bit to keep the timing
            std::thread::yield_now();
        }
        true
    }

    fn nova_packet(
        dst: &[u8; 6],
        src: &[u8; 6],
        command: u8,
        status: u8,
        sequence_num: usize,
    ) -> [u8; NOVA_PACKET_LEN] {
        let mut packet = [0u8; NOVA_PACKET_LEN];
        // ethernet header
        packet[0..6].copy_from_slice(dst);
        packet[6..12].copy_from_slice(src);
        packet[12] = (ETHER_TYPE_NOVA_SYNC >> 8) as u8;
        packet[13] = ETHER_TYPE_NOVA_SYNC as u8;
        // sync packet data
        packet[14] = (STATUS_DATA_LEN >> 8) as u8;
        packet[15] = STATUS_DATA_LEN as u8;
        packet[16] = command;
        packet[17] = status; // status
        packet[18] = (sequence_num >> 1) as u8;
        packet[19] = (sequence_num & 1) as u8;

        packet
    }

    fn udp_packet(
        interface_addr: &[u8; 6],
        local_ip: &[u8; 4],
        local_port: u16,
        module_address: u8,
        command: u8,
        sequence_num: usize,
        image: &VoxelImage,
    ) -> [u8; UDP_PACKET_LEN] {
        let mut packet = [0u8; UDP_PACKET_LEN];
        // ethernet header
        packet[0..5].copy_from_slice(&DMUX_ADDR_PREFIX);
        packet[5] = module_address;
        packet[6..12].copy_from_slice(interface_addr);
        packet[12] = (ETHER_TYPE_IP >> 8) as u8;
        packet[13] = ETHER_TYPE_IP as u8;

        // ip header
        packet[14] = IP_VERSION | 0x05;
        packet[15] = 0x00; // ECN / DSCP -- orignal value was 0xf0, which doesn't really make sense
        let ip_packet_len = IP_HEADER_LEN + UDP_HEADER_LEN + CHAINED_DATA_LEN;
        packet[16] = (ip_packet_len >> 8) as u8;
        packet[17] = ip_packet_len as u8;
        packet[18] = 0x32; // ID field
        packet[19] = 0x1c; // (according original code)
        packet[20] = 0x40; // fragment flags & offset
        packet[21] = 0x00; // (don't fragment, offset = 0)
        packet[22] = 0x80; // TTL (0x80 is common default)
        packet[23] = 0x11; // UDP
        packet[24] = 0x00; // checksum
        packet[25] = 0x00; // (calculated below)
        packet[26..30].copy_from_slice(local_ip);
        packet[30..33].copy_from_slice(&NOVA_IP_PREFIX);
        packet[33] = module_address;
        let checksum = Self::ip_checksum(&packet[14..34]);
        packet[24] = (checksum >> 8) as u8;
        packet[25] = checksum as u8;

        // udp header
        packet[34] = (local_port >> 8) as u8;
        packet[35] = local_port as u8;
        packet[36] = (NOVA_UDP_PORT >> 8) as u8;
        packet[37] = NOVA_UDP_PORT as u8;
        let udp_packet_len = UDP_HEADER_LEN + CHAINED_DATA_LEN;
        packet[38] = (udp_packet_len >> 8) as u8;
        packet[39] = udp_packet_len as u8;
        packet[40] = 0x00; // checksum
        packet[41] = 0x00; // (left as zero, which is okay for UDP)

        // udp payload
        Self::fill_udp_payload(&mut packet, command, sequence_num, image);

        packet
    }

    fn fill_udp_payload(
        packet: &mut [u8; UDP_PACKET_LEN],
        command: u8,
        sequence_num: usize,
        image: &VoxelImage,
    ) {
        for chain in 0..25 {
            let offset = UDP_PAYLOAD_OFFSET + chain * 44;
            packet[offset] = 0xc0;
            packet[offset + 1] = command;
            packet[offset + 2] = sequence_num as u8;
            packet[offset + 3] = chain as u8;

            let pixels = image.slice(0, chain); // row index 0, chain index = Y

            for i in 0..10 {
                let base = offset + 4 + i * 4;
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

    fn local_ip_and_port() -> std::io::Result<([u8; 4], u16)> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect("8.8.8.8:80")?;
        let local_addr = socket.local_addr()?;
        match local_addr.ip() {
            std::net::IpAddr::V4(v4) => Ok((v4.octets(), local_addr.port())),
            std::net::IpAddr::V6(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Expected IPv4 address, got IPv6",
            )),
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

    fn _print_sync_packet(packet: &[u8]) {
        if packet.len() < NOVA_PACKET_LEN {
            println!("Packet too short: {} bytes", packet.len());
            return;
        }

        let dst = &packet[0..6];
        let src = &packet[6..12];
        let ethertype = u16::from_be_bytes([packet[12], packet[13]]);
        let length = u16::from_be_bytes([packet[14], packet[15]]);
        let command = packet[16];
        let status = packet[17];
        let seq_hi = packet[18];
        let seq_lo = packet[19];
        let sequence = ((seq_hi as usize) << 1) | (seq_lo as usize);

        println!("ETH dst: {:02x?} src: {:02x?}", dst, src);
        println!(
            "type: 0x{:04x}, len: {}, cmd: 0x{:02x}, status: 0x{:02x}, seq: {}",
            ethertype, length, command, status, sequence
        );

        print!("dump:");
        for byte in &packet[20..] {
            print!(" {:02x}", byte);
        }
        println!();
    }
}

// Nova timing (20ms per frame, 50Hz)
const SYNC_PERIOD: Duration = Duration::from_millis(20);
const SYNC_BUSY_WAIT_MARGIN: Duration = Duration::from_millis(5);
const STATUS_PERIOD: Duration = Duration::from_millis(5000);
const INTERFACE_RETRY_PERIOD: Duration = Duration::from_millis(500);

// ethernet related constants
const ETHER_ADDR_LEN: usize = 6;
const ETHER_TYPE_LEN: usize = 2;
const ETHER_TYPE_IP: u16 = 0x0800;
const ETHER_TYPE_NOVA_SYNC: u16 = 0x0810;

const STATUS_DATA_LEN: usize = 46;
const PIXEL_DATA_LEN: usize = 5 * 5 * 10 * 4; // 5x5x10 pixels, 4 bytes per pixel (RGBA)
const CHAINED_DATA_LEN: usize = PIXEL_DATA_LEN + 4 * 25; // 25 chains of 4 extra bytes each

const NOVA_PACKET_LEN: usize = ETHER_ADDR_LEN + ETHER_ADDR_LEN + ETHER_TYPE_LEN + STATUS_DATA_LEN;

const UDP_PAYLOAD_OFFSET: usize = 42;
const UDP_HEADER_LEN: usize = 8;
const UDP_PACKET_LEN: usize = UDP_PAYLOAD_OFFSET + CHAINED_DATA_LEN;

const IP_VERSION: u8 = 0x40;
const IP_HEADER_LEN: usize = 20;
const NOVA_IP: [u8; 4] = [192, 168, 1, 0];
const NOVA_IP_PREFIX: [u8; 3] = [192, 168, 1];
const NOVA_UDP_PORT: u16 = 3210;

const BROADCAST_ADDR: [u8; 6] = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const SYNC_ADDR: [u8; 6] = [0x00, 0x20, 0xE3, 0x10, 0x01, 0x00];
const DMUX_ADDR_PREFIX: [u8; 5] = [0x00, 0x20, 0xE3, 0x10, 0x00];

// Sync command values
const CMD_SYNC: u8 = 0x00;
// const CMD_PLL: u8 = 0x01;
const CMD_START: u8 = 0x02;
const CMD_STOP: u8 = 0x03;
const CMD_STATUS: u8 = 0x04;

// Status flags
const STATUS_STOPPED: u8 = 0x00;
const STATUS_RUNNING: u8 = 0x01;

// DMUX command values
const CMD_RESET: u8 = 0x00;
const CMD_RGB: u8 = 0x02;
// const CMD_DOT_CORR: u8 = 0x04;
// const CMD_COLOR_CORR: u8 = 0x08;
// const CMD_BRIGHTNESS: u8 = 0x10;
// const CMD_OPMODE: u8 = 0x40;
const CMD_AUTOID: u8 = 0x70;

// FSS Power flags
// const FSS_POWER_OK3: u8 = 0x80;
// const FSS_POWER_OK2: u8 = 0x40;
// const FSS_POWER_OK1: u8 = 0x20;
// const FSS_POWER_ERASE_TIMEOUT: u8 = 0x10;
// const FSS_POWER_PROGRAM_TIMEOUT: u8 = 0x08;
// const FSS_POWER_ERASE_PENDING: u8 = 0x04;
// const FSS_POWER_PROGRAM_PENDING: u8 = 0x02;
// const FSS_POWER_RESTART_PENDING: u8 = 0x01;
