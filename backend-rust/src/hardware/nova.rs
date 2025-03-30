use std::net::UdpSocket;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::check_run_once;

use crate::app_state::AppState;
use crate::renderer::renderer::{RenderState, Renderer};
use crate::renderer::voxel_image::VoxelImage;

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
    interface: Interface,

    is_running: bool,
    sequence_number: usize,

    // TODO: once we have this all working: do we really need these?
    local_ip_addr: [u8; 4],
    local_port: u16,
}

impl NovaHardware {
    fn new(app_state: Arc<Mutex<AppState>>, renderer: Renderer) -> Self {
        let (local_ip_addr, local_port) = local_ip_and_port().unwrap();

        NovaHardware {
            app_state,
            renderer,
            interface: Interface::new("", None),
            is_running: false,
            sequence_number: 0,
            local_ip_addr,
            local_port,
        }
    }

    fn run(&mut self) {
        // TODO::
        // __MY_MAC__ will be replaced with the interfaces mac, once it's known
        // it doesn't seem we need to filter for broadcast packets
        let filter = format!(
            "ether proto {} and ether dst __MY_MAC__ or ether broadcast",
            ETHER_TYPE_NOVA_SYNC
        );

        // TODO: need to check this, I think it's actually 40 fps @ 25ms each
        let frame_duration = std::time::Duration::from_millis(40); // 25 frames per second

        let mut time = std::time::Instant::now();
        let mut image = VoxelImage::new(self.app_state.lock().unwrap().dim());

        loop {
            // Make sure app_state is unlocked quickly otherwise webserver thread will starve
            let (interface_name, render_state, modules) = {
                let app_state = self.app_state.lock().unwrap();
                (
                    app_state.ethernet_interface().to_string(),
                    RenderState::from(&app_state),
                    app_state.modules().clone(),
                )
            };

            // Try to open the interface
            if !self.interface.is_open() || self.interface.name() != &interface_name {
                self.interface = Interface::new(&interface_name, Some(filter.as_str()));
                if !self.interface.is_open() {
                    eprintln!("Failed to open interface {}. Retrying...", interface_name);
                    std::thread::sleep(INTERFACE_ERROR_SLEEP_DURATION);
                    continue;
                }
                println!("Opened interface {}", interface_name);
                self.reset_modules(&modules);
            }

            assert!(self.interface.is_open(), "Interface not open.");

            // Check if we need to request status from each module
            // TODO

            // Handle received status packets
            while let Ok(packet) = self.interface.receive() {
                self.handle_status_packet(&packet);
            }

            // Main processing loop if everything is full operational
            println!("Processing frame...");
            let delta = std::time::Instant::now().duration_since(time);
            self.renderer
                .render(&render_state, &mut image, delta.as_secs_f32());

            self.create_and_queue_packets(&modules, &image);
            time = std::time::Instant::now();

            // TODO: we need to compensate the time used for rendering here
            std::thread::sleep(frame_duration);
        }
        // won't reach (we're running on the main thread)
    }

    fn create_and_queue_packets(&mut self, modules: &[(usize, usize, u8)], image: &VoxelImage) {
        let mut module_buffer = vec![0u8; PIXEL_DATA_LEN];

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
            //self.interface.unwrap().send_packet();
        }
    }

    fn handle_status_packet(&mut self, packet: &[u8]) {
        // Ignore if destination is broadcast
        // TODO: remove broadcast in filter, and forget about this
        if &packet[0..6] == [0xff; 6] {
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

        // TODO: check, this should actually be my ethernet address, shouldn't it?
        let src: [u8; 6] = packet[0..6].try_into().unwrap();

        // TODO: what's unclear here, how does this work with multiple modules? will each of them send a
        // status packet? And if so, how do we handle this correctly?
        let command = packet[6 + 6 + 2 + 2];
        let reply;
        match command {
            CMD_START => {
                self.is_running = true;
                self.sequence_number = 0;
                reply = self.sync_packet(&SYNC_ADDR, &src, CMD_STATUS, STATUS_RUNNING, 511);
            }
            CMD_STOP => {
                self.is_running = false;
                self.sequence_number = 0;
                reply = self.sync_packet(&SYNC_ADDR, &src, CMD_STATUS, STATUS_STOPPED, 0);
            }
            CMD_STATUS => {
                if packet[20] == NOVA_IP[0] {
                    // TODO: set alive status of this module
                    // we need to find a way where to put this and make it available to the web server
                    let module_address = packet[23];
                    return;
                } else {
                    reply = self.sync_packet(
                        &SYNC_ADDR,
                        &src,
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
                eprintln!("Unknown command: {}", command);
                return;
            }
        }
        let _ = self.interface.send(reply);
    }

    fn reset_modules(&mut self, modules: &[(usize, usize, u8)]) {
        for &(mx, my, address) in modules {
            let packet = self.udp_packet(address, CMD_RESET, 0, &BLACK_PIXELS);
            let _ = self.interface.send(packet);
        }
    }

    fn sync_packet(
        &self,
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
        &self,
        module_address: u8,
        command: u8,
        sequence_num: usize,
        pixel_data: &[u8; 5 * 5 * 10 * 4],
    ) -> [u8; UDP_PACKET_LEN] {
        let mut packet = [0u8; UDP_PACKET_LEN];
        // ethernet header
        packet[0..5].copy_from_slice(&DMUX_ADDR_PREFIX);
        packet[5] = module_address;
        packet[6..12].copy_from_slice(&self.interface.address());
        packet[12] = (ETHER_TYPE_IP >> 8) as u8;
        packet[13] = ETHER_TYPE_IP as u8;
        // ip header
        packet[14] = IP_VERSION | 0x05;
        packet[15] = 0x00; // ECN / DSCP -- orignal value was 0xf0, which doesn't really make sense
        let ip_packet_len = IP_HEADER_LEN + UDP_HEADER_LEN + CHAINED_DATA_LEN;
        packet[16] = (ip_packet_len >> 8) as u8;
        packet[17] = ip_packet_len as u8;
        packet[18] = 0x32;
        packet[19] = 0x1c;
        packet[20] = 0x40;
        packet[21] = 0x00;
        packet[22] = 0x80;
        packet[23] = 0x11; // UDP
        packet[24] = 0x00; // checksum
        packet[25] = 0x00; // checksum
        packet[26..30].copy_from_slice(&self.local_ip_addr);
        packet[30..33].copy_from_slice(&NOVA_IP_PREFIX);
        packet[33] = module_address;
        let checksum = ip_checksum(&packet[14..34]);
        packet[24] = (checksum >> 8) as u8;
        packet[25] = checksum as u8;
        // udp header
        packet[34] = (self.local_port >> 8) as u8;
        packet[35] = self.local_port as u8;
        packet[36] = (NOVA_UDP_PORT >> 8) as u8;
        packet[37] = NOVA_UDP_PORT as u8;
        let udp_packet_len = UDP_HEADER_LEN + CHAINED_DATA_LEN;
        packet[36] = (udp_packet_len >> 8) as u8;
        packet[37] = udp_packet_len as u8;
        packet[38] = 0x00; // checksum
        packet[39] = 0x00; // checksum

        packet
    }
}

// nova packets

// udp/ip network utilities

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

// timing constants
const INTERFACE_ERROR_SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(500);

// ethernet related constants
const ETHER_ADDR_LEN: usize = 6;
const ETHER_TYPE_LEN: usize = 2;
const ETHER_TYPE_IP: u16 = 0x0800;
const ETHER_TYPE_NOVA_SYNC: u16 = 0x0810;

const STATUS_DATA_LEN: usize = 46;
const NOVA_PACKET_LEN: usize = ETHER_ADDR_LEN + ETHER_ADDR_LEN + ETHER_TYPE_LEN + STATUS_DATA_LEN;

const PIXEL_DATA_LEN: usize = 5 * 5 * 10 * 4; // 5x5x10 pixels, 4 bytes per pixel (RGBA)
const CHAINED_DATA_LEN: usize = PIXEL_DATA_LEN + 4 * 25; // 25 chains of 4 extra bytes each

const UDP_PAYLOAD_OFFSET: usize = 42;
const UDP_HEADER_LEN: usize = 8;
const UDP_PACKET_LEN: usize = UDP_PAYLOAD_OFFSET + CHAINED_DATA_LEN;
const NOVA_UDP_PORT: u16 = 3210;

const BLACK_PIXELS: [u8; PIXEL_DATA_LEN] = [0; PIXEL_DATA_LEN];

const IP_VERSION: u8 = 0x40;
const IP_HEADER_LEN: usize = 20;
const NOVA_IP: [u8; 4] = [192, 168, 1, 0];
const NOVA_IP_PREFIX: [u8; 3] = [192, 168, 1];

const BROADCAST_ADDR: [u8; 6] = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const SYNC_ADDR: [u8; 6] = [0x00, 0x20, 0xE3, 0x10, 0x01, 0x00];
const DMUX_ADDR_PREFIX: [u8; 5] = [0x00, 0x20, 0xE3, 0x10, 0x00];

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
