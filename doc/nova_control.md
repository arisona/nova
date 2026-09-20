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

Every content family uses the same four expressive controls:

| Control | Meaning                                                                                                                      |
| ------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Tone    | Steers the dominant hue of an authored palette; the endpoints wrap around the hue wheel.                                     |
| Heat    | Rich color by roughly 0.5, followed by smoothly increasing palette contrast and accents; independent of occupancy and speed. |
| Flow    | Animation rate. Zero freezes the current image; raising it resumes from the same phase.                                      |
| Form    | Simple to complex, with more overlap and positional variation while preserving spatial identity. Kept provisionally.         |

All families use the same OKLCH palette response, with chroma fitted to each hue's RGB gamut while preserving hue and lightness. Heat rises quickly from a subtle tint to rich color in its lower half, then smoothly introduces supporting hues and contrasting accents. Heat does not change geometry or couple to Form. The CSS tone chip approximates the dominant color using the same saturation curve; browser gamut mapping differs from the renderer. It does not preview the full composition or hardware brightness. Neither Heat nor Form normalizes total emitted light, so changes in color and occupied space can still affect perceived brightness.

### Content families

| Family  | Spatial character                                    | Form                                                         |
| ------- | ---------------------------------------------------- | ------------------------------------------------------------ |
| Field   | Broad, coherent color washes with soft edges.        | Sparse lit regions to a fully revealed gradient.             |
| Layers  | Horizontal light slices fading at different heights. | Sparse soft handoffs to denser overlap and varied positions. |
| Threads | Full-height vertical columns, fading in and out.     | Sparse soft handoffs to denser overlap and varied positions. |
| Cloud   | A compact moving pool with dark surrounding space.   | Blends toward broad, evolving coherent noise.                |

Features are sampled in voxel units rather than stretching a fixed number of details to each module layout. Gradients span several voxels; lines and slices have soft, fractionally sampled edges. Palette cycling, fades, and breathing are behaviors within families rather than additional modules. At zero Heat and full Form, Field is uniform and static even when Flow is raised; below full Form its coverage still moves.

Field's Form reveals a fixed-scale gradient through broad, soft coverage masks. At zero, a sparse lit region has a full-intensity core with darkness around it; intermediate values expand neighboring washes and close the dark gaps; at one the full gradient is visible. Increasing Form never reduces coverage or moves the underlying colors, avoiding frequency-driven color flicker while adjusting the slider. The mask drifts back and forth within the display bounds, keeping a full-strength core visible on small layouts. Drift speed is independent of Form. Heat changes palette colors while coverage remains visible even at zero Heat. Larger layouts repeat these broad washes in voxel units.

At Form zero, Layers and Threads use voxel-thick structures with soft overlapping handoffs. Each structure reaches full palette intensity and holds until its replacement starts before fading out. Increasing Form adds hold time in event-cycle units after both conditions are met, retaining more events even at maximum Flow, and varies positions. Normalized soft edges prevent fractional sampling from attenuating a structure's peak. Faster cycling can overlap several fading tails even at Form zero; existing tails are allowed to finish rather than being abruptly removed. Overlaps blend colors by their intensity weights and cap total intensity at one instead of clipping RGB channels. Brightness remains the final output multiplier.

Content selection is manual. Effect changes reset the selected animation and currently switch directly without a crossfade.

Layers and Threads use two clocks: Flow drives event cycling at `FADE_EVENTS_PER_PHASE_UNIT = 0.18` through the shared phase, while fade envelopes advance in active seconds. With the current global multiplier, full Flow starts about 1.8 events per second. Fade-in and fade-out each take `FADE_SECONDS = 1.875`, independent of nonzero Flow. Lower Flow spaces out births and lengthens holds rather than slowing the fades. Flow zero freezes births, holds, and fades together; resuming or changing Flow does not jump their phase or intensity. These timing constants live in `server/src/content.rs`.

Cloud's Form-zero pool uses a 1.3-voxel Gaussian width and retains its full-strength core; Form one preserves the broad organic noise envelope. Its 2x intensity gain is capped before multiplying palette RGB. Heat 0 through 0.5 samples the selected Tone, with increasing saturation. Above 0.5 the sampled palette range opens smoothly, bringing contrasting accents into brighter regions without altering coverage, motion, or intensity shaping. These tuning constants live in `server/src/content/cloud.rs`.

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

Content writes unscaled RGB to `next`; the renderer keeps `prev` unscaled and applies Brightness only to a separate output image. Use the shared palette and accumulated Flow-driven phase. Reset animation state when `should_reset()` is true, and do not use wall-clock elapsed time for motion that must freeze at zero Flow.

Run `cargo test` in `server` for palette, geometry, freeze/reset, brightness, and settings-validation checks. Run `npm run build` in `webapp` before building the server so its embedded UI matches the API.

For a diagnostic contact sheet and local render timings, run `cargo test content_preview_and_timings -- --ignored --nocapture` in `server`. It writes `nova-content-preview.ppm` to the OS temporary directory. Columns are Field, Layers, Threads, and Cloud. The first five rows use Heat 0, 0.25, 0.5, 0.75, and 1 at Form zero; the next two use Heat 1 at Form 0.5 and 1; the final two use Heat 0 at Form 0.5 and 1. Sparse fades are captured at their peak. These synthetic previews and local timings do not replace physical-display evaluation or Raspberry Pi profiling.

---

## Troubleshooting

- If modules do not respond, verify jumper settings and network IP.
- Use `tcpdump` or `wireshark` on the interface for raw packet inspection.
- Check logs for warnings about missed sync or interface errors.
