//! The Bluetooth speaker set in the settings file: connects and disconnects it through
//! `bluetoothctl` when the web app asks, and checks every few seconds whether it is
//! connected, so the web app shows it either way. Pairing and the ALSA default are set up
//! by hand (see the Raspberry Pi setup); the sound finds the speaker by itself once it is
//! connected.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

/// What the web app can ask of the speaker.
#[derive(Debug, Clone, Copy)]
pub enum Request {
    Connect,
    Disconnect,
}

/// The speaker's service. Its thread runs as long as Nova.
pub struct Speaker {
    requests: Sender<Request>,
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    connected: AtomicBool,
    // A request is being carried out
    busy: AtomicBool,
}

impl Speaker {
    /// Starts the service thread for the speaker at `address`.
    pub fn start(address: String) -> std::io::Result<Self> {
        log::info!("Using Bluetooth speaker {address}.");
        let (requests, received) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let thread_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("nova-speaker".into())
            .spawn(move || {
                let mut handled = false;
                loop {
                    thread_shared
                        .connected
                        .store(is_connected(&address), Ordering::Relaxed);
                    // Cleared only after checking, so the web app never shows the old state.
                    if handled {
                        thread_shared.busy.store(false, Ordering::Relaxed);
                    }
                    handled = match received.recv_timeout(CHECK_PERIOD) {
                        Ok(request) => {
                            carry_out(request, &address);
                            true
                        }
                        Err(RecvTimeoutError::Timeout) => false,
                        Err(RecvTimeoutError::Disconnected) => return,
                    };
                }
            })?;
        Ok(Self { requests, shared })
    }

    /// Asks to connect or disconnect the speaker. Ignored while a request is being carried out.
    pub fn request(&self, request: Request) {
        if !self.shared.busy.swap(true, Ordering::Relaxed) {
            let _ = self.requests.send(request);
        }
    }

    pub fn connected(&self) -> bool {
        self.shared.connected.load(Ordering::Relaxed)
    }

    pub fn busy(&self) -> bool {
        self.shared.busy.load(Ordering::Relaxed)
    }
}

fn carry_out(request: Request, address: &str) {
    let command = match request {
        Request::Connect => "connect",
        Request::Disconnect => "disconnect",
    };
    log::info!("Bluetooth speaker: {command}");
    match bluetoothctl(command, address) {
        Ok(_) => log::info!("Bluetooth speaker: {command} done"),
        Err(error) => log::warn!("Bluetooth speaker: {command} failed: {error}"),
    }
}

fn is_connected(address: &str) -> bool {
    bluetoothctl("info", address)
        .is_ok_and(|info| info.lines().any(|line| line.trim() == "Connected: yes"))
}

/// Runs `bluetoothctl <command> <address>`; its output, or why it failed.
fn bluetoothctl(command: &str, address: &str) -> Result<String, String> {
    let output = Command::new("bluetoothctl")
        .args(["--timeout", BLUETOOTHCTL_TIMEOUT_SECONDS, command, address])
        .output()
        .map_err(|error| format!("cannot run bluetoothctl ({error})"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        // bluetoothctl explains failures on its last line, such as "Failed to connect: …".
        Err(stdout
            .lines()
            .last()
            .unwrap_or("no output")
            .trim()
            .to_string())
    }
}

/// Whether `address` is a Bluetooth device address, such as `00:11:22:AA:BB:CC`.
pub fn is_address(address: &str) -> bool {
    let parts: Vec<&str> = address.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()))
}

const CHECK_PERIOD: Duration = Duration::from_secs(2);
const BLUETOOTHCTL_TIMEOUT_SECONDS: &str = "15";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_six_hex_pairs() {
        assert!(is_address("00:11:22:aa:BB:cc"));
        assert!(!is_address(""));
        assert!(!is_address("00:11:22:AA:BB"));
        assert!(!is_address("00:11:22:AA:BB:CG"));
        assert!(!is_address("00:11:22:AA:BB:CC:DD"));
        assert!(!is_address("--help"));
    }
}
