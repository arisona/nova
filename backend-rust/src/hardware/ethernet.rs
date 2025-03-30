use std::{borrow::Borrow, error::Error};

use etherparse::{EtherType, Ethernet2Header, PacketBuilder};
use mac_address;
use pcap;

pub struct Interface {
    name: String,
    address: [u8; 6],
    capture: Option<pcap::Capture<pcap::Active>>,
}

impl Interface {
    pub fn new(name: &str, filter: Option<&str>) -> Self {
        let mut address = [0; 6];
        let mut capture = None;

        match (|| -> Result<(), Box<dyn Error>> {
            let addr = mac_address::mac_address_by_name(name)?
                .map(|mac| mac.bytes())
                .ok_or_else(|| format!("Interface {} not found (mac)", name))?;

            let dev = pcap::Device::list()?
                .into_iter()
                .find(|d| d.name == name)
                .ok_or_else(|| format!("Interface {} not found (pcap)", name))?;

            let mut cap = pcap::Capture::from_device(dev)?
                .immediate_mode(true)
                .promisc(true)
                .open()?;

            cap = cap.setnonblock()?;

            if let Some(f) = filter {
                let f = f.replace("__MY_MAC__", &mac_address_as_string(addr));
                cap.filter(&f, true)?;
            }

            address = addr;
            capture = Some(cap);
            Ok(())
        })() {
            _ => {}
        }

        Self {
            name: name.to_string(),
            address,
            capture,
        }
    }

    pub fn is_open(&self) -> bool {
        self.capture.is_some()
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn receive(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        match self.capture.as_mut() {
            Some(cap) => {
                let packet = cap.next_packet()?;
                Ok(packet.data.to_vec())
            }
            None => Err(format!("Interface {} not open", self.name).into()),
        }
    }

    pub fn send<B: Borrow<[u8]>>(&mut self, packet: B) -> Result<(), Box<dyn Error>> {
        match self.capture.as_mut() {
            Some(cap) => {
                cap.sendpacket(packet)?;
                Ok(())
            }
            None => Err(format!("Interface {} not open", self.name).into()),
        }
    }

    pub fn address(&self) -> [u8; 6] {
        self.address
    }

    pub fn address_as_string(&self) -> String {
        mac_address_as_string(self.address)
    }
}

fn mac_address_as_string(address: [u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        address[0], address[1], address[2], address[3], address[4], address[5]
    )
}
