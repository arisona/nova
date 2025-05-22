use std::{borrow::Borrow, error::Error};

pub struct Interface {
    name: String,
    address: [u8; 6],
    capture: pcap::Capture<pcap::Active>,
}

#[allow(dead_code)]
impl Interface {
    pub fn new(name: &str, filter: Option<&str>) -> Result<Self, Box<dyn Error>> {
        let address = mac_address::mac_address_by_name(name)?
            .map(|mac| mac.bytes())
            .ok_or_else(|| format!("Interface {name} not found (mac)"))?;

        let device = pcap::Device::list()?
            .into_iter()
            .find(|d| d.name == name)
            .ok_or_else(|| format!("Interface {name} not found (pcap)"))?;

        let mut capture = pcap::Capture::from_device(device)?
            .immediate_mode(true)
            .promisc(true)
            .open()?;

        capture = capture.setnonblock()?;

        if let Some(f) = filter {
            let f = f.replace("__MY_MAC__", &mac_address_as_string(address));
            capture.filter(&f, true)?;
        }

        Ok(Self {
            name: name.to_string(),
            address,
            capture,
        })
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn receive(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let packet = self.capture.next_packet()?;
        Ok(packet.data.to_vec())
    }

    pub fn send<B: Borrow<[u8]>>(&mut self, packet: B) -> Result<(), Box<dyn Error>> {
        self.capture.sendpacket(packet)?;
        Ok(())
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

fn print_packet(text: &str, packet: &[u8], num_bytes: usize) {
    print!("{}({}): ", text, packet.len());
    for byte in &packet[..packet.len().min(num_bytes)] {
        print!("{:02x} ", byte);
    }
    println!();
}
