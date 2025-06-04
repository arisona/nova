# Nova Control Server Documentation

The Nova Control server is a Rust application that drives Nova voxel hardware by sending raw Ethernet frames. It replaces the original Java-based implementation and provides the same web interface for content management and control.

## Table of Contents

1. [Development Setup](#development-setup)
2. [Building and Running](#building-and-running)
3. [Web Interface](#web-interface)
4. [Configuration](#configuration)
5. [Hardware Addressing](#hardware-addressing)
6. [Content Extensions](#content-extensions)
7. [Troubleshooting](#troubleshooting)

---

## Development Setup

### Prerequisites

- Rust toolchain (Rust 1.87 or later) with Cargo
- `libpcap` development headers (for packet capture/send)
- Node.js (v16+) and npm (for web UI development)
- A code editor or IDE (VS Code preferred)

### Project Structure

```
├── server/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # Entry point
│       ├── app_state.rs      # Configuration and state management
│       ├── ethernet.rs       # pcap-based packet I/O
│       ├── nova.rs           # Hardware driver loop
│       ├── renderer.rs       # Frame rendering logic
│       ├── voxel_image.rs    # Image buffer abstraction
│       └── web_server.rs     # HTTP API and static file serving
└── webapp/
    ├── package.json
    └── src/                  # React + Material UI source
```

1. Clone the repository and enter the project root.
2. Build and install the web app:
   ```bash
   cd webapp
   npm install
   npm run build
   ```
3. Build and install the Rust server:
   ```bash
   rustup update
   cd server
   cargo build
   ```

---

## Building and Running

### Web App

During development:

```bash
cd webapp
npm run dev
```

This launches a hot-reloading server.

To build for production:

```bash
npm run build
```

The static bundle is copied into `server/src/www` on build.

### Nova Server

To compile and run the Nova server in release mode:

```bash
cd server
cargo run --release
```

The Nova server starts a web server on port 8080.

By default, the server reads its settings from `nova_settings.json` in the working directory. On first run, a default file is created.

---

## Web Interface

The built web client uses React and Material UI. It provides controls for:

- Selecting and ordering content modules
- Adjusting hue, saturation, brightness, speed, and cycle duration
- Toggling vertical flip
- Monitoring module status

Access it in your browser at `http://<server-host>:<webserver_port>/`.

---

## Configuration

All settings are stored in `nova_settings.json`. Example:

```json
{
  "ethernet_interface": "eth0",
  "webserver_port": 8080,
  "modules": [
    [0, 0, 1],
    [0, 1, 2],
    [1, 0, 4]
  ],
  "hue": 0.0,
  "saturation": 1.0,
  "brightness": 0.5,
  "speed": 0.1,
  "flip_vertical": false,
  "cycle_duration": 0.0,
  "enabled_content_indices": [0, 1],
  "selected_content_index": 0
}
```

- `modules`: list of `[x, y, address]` tuples.
- Other fields mirror UI controls.

---

## Hardware Addressing

Module MAC and IP addresses are derived from jumpers:

- MAC: `00:20:e3:10:00:<address>`
- IP: `192.168.1.<address>`
- Where `<address>` is given by the jumper setting on the hardware module

You can ping modules directly after assigning a static IP to your interface.

---

## Content Extensions

Content is implemented as Rust types that implement the `Content` trait:

```rust
pub trait Content {
    fn name(&self) -> &str;
    fn render(
        &mut self,
        state: &RenderState,
        elapsed: f32,
        delta: f32,
        prev: &VoxelImage,
        next: &mut VoxelImage,
    );
}
```

To add a new effect:

1. Create a struct in `server/src/content/`.
2. Implement `Content` for it.
3. Register it in `get_all_content()`.

For examples, refer to existing content in `server/src/content/`.

Ensure `render()` completes within 20 ms to avoid underruns.

---

## Troubleshooting

- If modules do not respond, verify jumper settings and network IP.
- Use `tcpdump` or `wireshark` on the interface for raw packet inspection.
- Check logs for warnings about missed sync or interface errors.
