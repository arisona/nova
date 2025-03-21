use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use etherparse::{EtherType, Ethernet2Header, PacketBuilder};
use mac_address;
use pcap;

const MAX_SEND_QUEUE_SIZE: usize = 100;
const NO_RX_TX_SLEEP_DURATION: Duration = Duration::from_micros(1000);

pub struct Interface {
    name: String,
    address: [u8; 6],
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    tx: SyncSender<Vec<u8>>,
}

impl Interface {
    pub fn new<F>(
        name: &str,
        filter: Option<&str>,
        callback: Option<F>,
    ) -> Result<Self, Box<dyn Error>>
    where
        F: Fn(&[u8]) -> Option<Vec<u8>> + Send + 'static,
    {
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

        if let Some(f) = filter {
            let f = f.replace("__MY_MAC__", &mac_address_as_string(address));
            println!("Setting filter: {}", f);
            capture.filter(&f, true)?;
        }

        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(MAX_SEND_QUEUE_SIZE);
        let running = Arc::new(AtomicBool::new(true));
        let run = Arc::clone(&running);

        let handle = thread::spawn(move || {
            while run.load(Ordering::Relaxed) {
                let mut request_sleep = true;

                // Receive packets
                match capture.next_packet() {
                    Ok(packet) => {
                        if let Some(callback) = &callback {
                            if let Some(response) = callback(packet.data) {
                                let _ = capture.sendpacket(response);
                                // TODO: error handling
                            }
                        }
                        request_sleep = false;
                    }
                    // TODO: error handling
                    Err(_) => { /* ignore */ }
                }

                // Send queued packets
                match rx.try_recv() {
                    Ok(packet) => {
                        let _ = capture.sendpacket(packet);
                        // TODO: error handling?
                        request_sleep = false;
                    }
                    Err(mpsc::TryRecvError::Empty) => { /* nothing to send */ }
                    Err(mpsc::TryRecvError::Disconnected) => { /* ignore */ }
                }

                if request_sleep {
                    thread::sleep(NO_RX_TX_SLEEP_DURATION);
                }
            }
        });

        Ok(Self {
            name: name.to_string(),
            address,
            running,
            handle: Some(handle),
            tx,
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

    pub fn send_packet(&self, packet: Vec<u8>) -> Result<(), Box<dyn Error>> {
        // TODO: error handling
        let _ = self.tx.try_send(packet);
        Ok(())
    }

    fn close(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().expect(&format!(
                "Interface {}: failed to join capture thread",
                self.name
            ));
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
