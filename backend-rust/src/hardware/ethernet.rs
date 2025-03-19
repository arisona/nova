use std::error::Error;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use etherparse::{EtherType, Ethernet2Header, PacketBuilder};

const CAPTURE_SLEEP: Duration = Duration::from_micros(1000);

pub struct Interface {
    name: String,
    address: [u8; 6],
    capture: Arc<Mutex<pcap::Capture<pcap::Active>>>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Interface {
    pub fn new(name: &str) -> Result<Self, Box<dyn Error>> {
        let address = mac_address::mac_address_by_name(name)?
            .map(|mac| mac.bytes())
            .ok_or_else(|| format!("Interface {} not found (mac)", name))?;

        let device = pcap::Device::list()?
            .into_iter()
            .find(|d| d.name == name)
            .ok_or_else(|| format!("Interface {} not found (pcap)", name))?;

        let mut capture = pcap::Capture::from_device(device)?
            .immediate_mode(true)
            .promisc(true)
            .open()?;
        capture = capture.setnonblock()?;

        println!("Interface {} opened", name);
        Ok(Self {
            name: name.to_string(),
            address: address,
            capture: Arc::new(Mutex::new(capture)),
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        })
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn mac_address(&self) -> [u8; 6] {
        self.address
    }

    pub fn mac_address_as_string(&self) -> String {
        mac_address_as_string(self.address)
    }

    pub fn set_filter(&mut self, filter: &str) -> Result<(), Box<dyn Error>> {
        self.capture.lock().unwrap().filter(filter, true)?;
        Ok(())
    }

    pub fn set_capture<F>(&mut self, callback: F) -> Result<(), Box<dyn Error>>
    where
        F: Fn(&[u8]) + Send + 'static,
    {
        if self.running.swap(true, Ordering::Relaxed) {
            return Err(format!("Interface {} capture already set", self.name).into());
        }

        let capture = Arc::clone(&self.capture);
        let running = Arc::clone(&self.running);
        let handle = thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                let mut cap = capture.lock().unwrap();
                if let Ok(packet) = cap.next_packet() {
                    callback(packet.data);
                } else {
                    thread::sleep(CAPTURE_SLEEP);
                }
            }
        });
        self.handle = Some(handle);
        Ok(())
    }

    pub fn send_packet(&mut self, packet: Vec<u8>) -> Result<(), Box<dyn Error>> {
        self.capture.lock().unwrap().sendpacket(packet)?;
        Ok(())
    }

    fn close(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().expect("Failed to join capture thread");
        }
        println!("Interface {} closed", self.name);
    }
}

impl Drop for Interface {
    fn drop(&mut self) {
        self.close();
    }
}

fn mac_address_as_string(address: [u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        address[0], address[1], address[2], address[3], address[4], address[5]
    )
}
