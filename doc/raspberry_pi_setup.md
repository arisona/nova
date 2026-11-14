# Raspberry Pi setup

How to set up a headless Raspberry Pi that builds Nova from source and drives the display. Basic Linux console experience is assumed. This guide uses `nova` as the host name and `pi` as the user; adjust the commands if you choose others.

## Flash Raspberry Pi OS

1. Install [Raspberry Pi Imager](https://www.raspberrypi.com/software/).
2. Choose your device (for example, Raspberry Pi 4) and **Raspberry Pi OS Lite (64-bit)**.
3. Before writing, set the host name, user and password, WLAN, and time zone, and enable SSH. Get this right: otherwise you cannot connect to the headless Pi.
4. Write the image, put the SD card into the Pi, and power it up.

## Connect and update

Once the Pi has booted, connect from your machine:

```sh
ssh pi@nova.local
```

If the host name does not resolve, look up the Pi's IP address on your router. `sudo raspi-config` changes further settings later, such as the WLAN.

Update the system and install the build dependencies:

```sh
sudo apt update
sudo apt full-upgrade
sudo reboot
# after reconnecting:
sudo apt install git libpcap-dev libasound2-dev pkg-config alsa-utils
```

Install Rust (as `pi`, not root):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"            # puts cargo on the path; new logins get it automatically
```

## Get the source

```sh
git clone https://github.com/arisona/nova.git ~/nova
```

The web app is already built into the repository, so the Pi needs no Node.js.

## Run Nova

Sending raw Ethernet frames needs the `CAP_NET_RAW` and `CAP_NET_ADMIN` capabilities. Choose one of the two options below. Both run from the home directory and therefore share `~/nova_settings.json`. Do not run both at once.

Once Nova is running, open `http://nova.local:8080`.

### Option A: run in place

Use this for experimenting and debugging. Build, grant the capabilities, and run in the foreground (stop with Ctrl+C):

```sh
cd ~/nova
cargo build --release
sudo setcap cap_net_raw,cap_net_admin+ep target/release/nova
cd ~ && ~/nova/target/release/nova
```

Each rebuild replaces the binary and drops its capabilities, so repeat `setcap` after building.

To serve on port 80 instead of 8080, also grant `cap_net_bind_service` and set `"webserver_port": 80` in `~/nova_settings.json`:

```sh
sudo setcap cap_net_raw,cap_net_admin,cap_net_bind_service+ep ~/nova/target/release/nova
```

### Option B: run as a service

Use this for installations. Nova starts at boot and restarts if it exits. Install the binary to `~/.cargo/bin` and create a systemd service that grants the capabilities:

```sh
cargo install --path ~/nova/host
sudo tee /etc/systemd/system/nova.service > /dev/null <<'EOF'
[Unit]
Description=Nova
After=network.target

[Service]
User=pi
WorkingDirectory=/home/pi
ExecStart=/home/pi/.cargo/bin/nova
Environment=RUST_LOG=info
AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl enable --now nova
```

To use port 80, add `CAP_NET_BIND_SERVICE` to `AmbientCapabilities` and set `webserver_port` as in option A:

```sh
cargo install --path ~/nova/host
sudo tee /etc/systemd/system/nova.service > /dev/null <<'EOF'
[Unit]
Description=Nova
After=network.target

[Service]
User=pi
WorkingDirectory=/home/pi
ExecStart=/home/pi/.cargo/bin/nova
Environment=RUST_LOG=info
AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN CAP_NET_BIND_SERVICE
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl enable --now nova
```

Managing the service:

- Logs: `journalctl -u nova -f`
- Update and restart: `cd ~/nova && git pull && cargo install --path host && sudo systemctl restart nova`
- Stop, for example before using option A: `sudo systemctl stop nova`; `sudo systemctl disable nova` also removes it from startup. Stopping resets the modules, so the display goes dark.

## Configure the voxel display

By default Nova uses `eth0` and one module with jumper address 1. Change the interface in the settings file or module address in the web app's settings. A layout of several modules is configured in `~/nova_settings.json`; see [Settings file](development.md#settings-file).

## Native audio output

Nova plays audio through ALSA; no PulseAudio or PipeWire is needed. Connect speakers through a USB DAC, an audio HAT, HDMI, or the analog output. `aplay -l` lists devices and `aplay -L` lists ALSA PCM names. Set the default output before starting Nova. A USB DAC with a stable card name is more reliable than card numbers.

If a sound server is installed, configure the ALSA default to go through it rather than competing for the device. The user running Nova needs audio permissions (`pi` is in the `audio` group by default).

Audio is off by default. To turn it on, set `"audio": true` in `~/nova_settings.json` (created on the first run) and restart Nova. Volume starts at zero; lower the speaker volume before raising it in the web app.

## Bluetooth audio output

For installations without a cable to the speakers, for example when Nova hangs high above the ground. [BlueALSA](https://github.com/arkq/bluez-alsa) makes a Bluetooth speaker the ALSA default, still without a sound server. Expect about 100–300 ms of latency, and little of the bass the sound is designed for. Wi-Fi and Bluetooth share one radio on the Pi; if the sound drops out, try a USB Bluetooth adapter.

Install BlueALSA, unblock Bluetooth (Raspberry Pi OS may start with it blocked, and `power on` then fails) and allow `pi` to control it, then log in again:

```sh
sudo apt install bluez bluez-alsa-utils libasound2-plugin-bluez
sudo systemctl enable --now bluetooth bluealsa
sudo rfkill unblock bluetooth
sudo usermod -aG bluetooth pi
```

Put the speaker into pairing mode, disconnected from other devices, and pair it once. Pair while the scan is still running: once it stops, unpaired devices disappear after a while ("not available"). If pairing fails, scan and try again. `trust` lets the speaker connect to the Pi without confirmation:

```sh
bluetoothctl
power on
agent on
scan on                         # wait for the speaker's name, note its address
pair XX:XX:XX:XX:XX:XX
scan off
trust XX:XX:XX:XX:XX:XX
connect XX:XX:XX:XX:XX:XX
exit
```

Other addresses that come and go, often without a name, are nearby phones and computers, whose Bluetooth Low Energy addresses change for privacy.

Make the speaker the ALSA default in `~/.asoundrc`, with its address:

```
defaults.bluealsa.device "XX:XX:XX:XX:XX:XX"
defaults.bluealsa.profile "a2dp"
pcm.!default { type plug slave.pcm "bluealsa" }
ctl.!default "bluealsa"
```

Check with `speaker-test -c 2 -t wav` (Ctrl+C stops it). Then set the address in `~/nova_settings.json`, `"bluetooth_speaker": "XX:XX:XX:XX:XX:XX"` (with `"audio": true`), and restart Nova. To return to the native output, remove `~/.asoundrc` and the address.

The web app now shows a Bluetooth button next to Settings: tap it to connect or disconnect the speaker. While the speaker is not connected, Nova reports the audio as unavailable and retries; within a few seconds of connecting, it plays and Volume appears. Nova never connects by itself, so it does not take the speaker from another device. A speaker usually plays from one device at a time, and reconnects to the last one when switched on. To move it between Nova and, say, your phone:

- To Nova: disconnect the speaker in the phone's Bluetooth settings, then tap the button in the web app.
- To the phone: tap the button to disconnect, then connect from the phone.

Without the web app, `bluetoothctl connect XX:XX:XX:XX:XX:XX` and `bluetoothctl disconnect XX:XX:XX:XX:XX:XX` do the same.
