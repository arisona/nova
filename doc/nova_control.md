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

- Rust toolchain (Rust 1.89 or later) with Cargo
- `libpcap` development headers (for packet capture/send)
- Linux audio builds: ALSA development headers (`libasound2-dev` on Raspberry Pi OS/Debian) and `pkg-config`. macOS 14.2 or newer uses CoreAudio; Windows audio uses WASAPI (other Nova hardware dependencies still apply).
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

The Nova server starts a web server on port 8080. The Vite development server proxies `/api` to this port.

The build-time switches `ENABLE_SIMULATOR` and `ENABLE_AUDIO` in `server/src/main.rs` both default to `true`. Set `ENABLE_SIMULATOR` to `false` to use hardware output; both modes use the same content renderer. Set `ENABLE_AUDIO` to `false` to skip audio startup and hide Volume in the web client. The state API exposes this choice as `audio-enabled`, independently of device availability. Rebuild the server after changing either switch.

By default, the server reads its settings from `nova_settings.json` in the working directory. On first run, a default file is created.

---

## Web Interface

The built web client uses React and Material UI. It provides controls for:

- Selecting and ordering content modules
- Adjusting Brightness, Tone, Heat, Flow, and Form
- Adjusting independent audio Volume
- Toggling vertical flip
- Monitoring module status

Access it in your browser at `http://<server-host>:<webserver_port>/`.

### Artistic controls

Brightness is a final output multiplier, independent of content generation. Setting it to zero blacks out the display without stopping animation. Volume independently controls the complete audio output, including echo tails; zero mutes without stopping musical time. See the [canonical audio semantics](../README.md#audio) for the shared controls' musical interpretation.

Every content module uses the same expressive controls:

| Control | Meaning                                                                                                                       |
| ------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Tone    | Selects one of the curated Pantone palettes, in equal slices of the slider. Temporary until a palette picker replaces it.     |
| Heat    | Gray at 0, the palette as published at 0.5, up to twice its saturation at 1, preserving hue; independent of occupancy and speed. |
| Flow    | Animation rate. Zero freezes the current image; raising it resumes from the same phase.                                       |
| Form    | Structure, from horizontal layers through columns, blobs and patches to per-voxel grain. Also sets how many colors show at once. |
| Void    | Fraction of the volume left dark. Lit voxels keep the same brightness at any value.                                           |

Palettes live in `server/src/content/palettes.rs`; neighbouring palette colors are mixed in Oklab. The CSS chip next to the Tone slider still shows a single hue from the earlier tone model and does not preview the selected palette. Neither Heat nor Form normalizes total emitted light, so changes in color and occupied space can still affect perceived brightness.

### Content

Flux is currently the only content module. Its complete specification is the pseudocode at the top of `server/src/content/flux.rs`; the tuning constants live next to it. In short:

- One 4D simplex noise primitive covers the whole Form range. Form blends five fields that differ only in their per-axis frequencies: layers (Form 0), columns (0.25), blobs (0.5), patches (0.75) and grain (1). Frequencies are fixed, so moving Form never zooms the pattern, and blends are normalized to constant contrast.
- Brightness is computed by rank, so Void is exactly the fraction of dark voxels.
- The structure rises slowly and sways with a slow tide that now and then reverses its direction.
- A separate, slower noise picks palette positions: about two neighbouring colors for smooth structures, all colors for grain. The visible colors slowly rotate through the palette.
- All motion follows the Flow-integrated phase, so Flow zero freezes everything and changing Flow never jumps.

Features are sampled in voxel units, so additional modules show more of the same field rather than a stretched one.

Content selection is manual. Effect changes reset the selected animation and currently switch directly without a crossfade. The content chooser is hidden while only one module is enabled.

---

## Native Audio

Audio plays through the server machine's default output, not the browser. It runs in both simulator and hardware modes. Select the output through the OS (or ALSA configuration on a headless Pi) before launching Nova. New settings default to Volume zero. Raise it gradually with system/speaker volume low; saved Volume is restored on the next launch.

The synth uses CPAL output and FunDSP band-limited oscillators/resonant state-variable filters. No desktop sound server is required on a headless ALSA system. Device failures appear separately from display status; the service retries, and the lights and web API remain operational. If output is unavailable, check the OS default device, permissions for the account running Nova, and whether another process has exclusive access.

From the repository root:

```sh
cargo test --manifest-path server/Cargo.toml audio
cargo test --manifest-path server/Cargo.toml audio_listening_previews -- --ignored --nocapture
cargo test --manifest-path server/Cargo.toml native_audio_output_smoke -- --ignored --nocapture
```

The preview test writes three 30-second WAV files (`slow`, `bleeps`, `arpeggios`) under `nova-audio` in the OS temporary directory and prints their paths, peak levels and render times. Files are not normalized; they preserve actual synth headroom at Volume one. They require no output device. The native smoke test requires a device and stays muted. For Pi installation/performance checks, use a release build with visuals active; desktop tests cannot establish Pi callback headroom or installation sound quality.

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
  "brightness": 0.5,
  "volume": 0.0,
  "tone": 0.0,
  "heat": 0.5,
  "flow": 0.25,
  "form": 0.0,
  "flip_vertical": false,
  "enabled_content_indices": [0, 1, 2, 3],
  "selected_content_index": 0
}
```

- `modules`: list of `[x, y, address]` tuples.
- Other fields mirror UI controls.
- GET `/api/get-state` exposes `brightness`, `volume`, `tone`, `heat`, `flow`, and `form`. SET via GET `/api/{control}?value=<0..1>` persists a value.

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

Content writes unscaled RGB to `next`; the renderer keeps `prev` unscaled and applies Brightness only to a separate output image. Use the palettes in `palettes.rs` and the accumulated Flow-driven phase (`advance()`). Reset animation state when `should_reset()` is true, and do not use wall-clock elapsed time for motion that must freeze at zero Flow.

Run `cargo test` in `server` for content bounds and freeze/reset, Flow timing, settings validation, API, and audio safety checks. Run `npm run build` in `webapp` before building the server so its embedded UI matches the API.

For a diagnostic contact sheet, run `cargo test flux_form_sweep_dump -- --ignored --nocapture` in `server`. It writes `nova-flux-sweep.ppm` to the OS temporary directory: columns are Form 0 to 1 in steps of 0.125 (the five fields sit at 0, 0.25, 0.5, 0.75 and 1), rows are the palettes, each showing the middle slice of one module at Heat 0.5. These synthetic previews do not replace physical-display evaluation or Raspberry Pi profiling.

---

## Troubleshooting

- If modules do not respond, verify jumper settings and network IP.
- Use `tcpdump` or `wireshark` on the interface for raw packet inspection.
- Check logs for warnings about missed sync or interface errors.
