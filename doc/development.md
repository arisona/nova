# Development

How to build, run, configure, and extend the Nova server and web app. Control semantics, design constraints, the API, and the development checks are in the [README](../README.md).

## Prerequisites

- Rust 1.89 or newer, with Cargo
- `libpcap` development headers
- Audio: on Linux, ALSA development headers (`libasound2-dev`) and `pkg-config`; on macOS, 14.2 or newer (CoreAudio); on Windows, WASAPI
- Node.js 20.19+, 22.13+, or 24+, only for changing the web app. The built UI is committed in `server/src/www` and embedded in the server binary.

## Project layout

```text
server/src/
├── main.rs          # Entry point and build-time switches
├── app_state.rs     # Settings and shared state
├── web_server.rs    # HTTP API and embedded web app
├── renderer.rs      # Applies controls and tides, calls content
├── content.rs       # Content trait and registry
├── content/         # One file per module, each with its specification; common.rs holds shared helpers
├── tides.rs         # Slow system-driven variation
├── palettes.rs      # Curated palettes
├── audio.rs         # Audio service; synth and output in audio/
├── nova.rs          # Hardware driver loop and protocol
├── ethernet.rs      # pcap packet I/O
└── simulator.rs     # Desktop simulator window
webapp/src/          # React + Material UI control interface
```

## Build and run

Run the server from `server`:

```sh
cargo run --release
```

It serves the web app on port 8080 and reads `nova_settings.json` from the working directory, creating it on first run. By default it drives the hardware with audio off. For desktop development, set `"simulator": true` and, for sound, `"audio": true` in the [settings file](#settings-file), then restart.

For web app development, run a hot-reloading server from `webapp`; it proxies `/api` to the server on port 8080:

```sh
npm install
npm run dev
```

`npm run build` writes the production bundle to `server/src/www`. Rebuild the server afterwards to embed it.

## Settings file

`nova_settings.json` holds all settings:

```json
{
  "ethernet_interface": "eth0",
  "webserver_port": 8080,
  "simulator": false,
  "audio": false,
  "modules": [
    [0, 0, 1],
    [0, 1, 2],
    [1, 0, 4]
  ],
  "brightness": 0.5,
  "volume": 0.0,
  "palette": "Belize Ripple",
  "heat": 0.5,
  "flow": 0.5,
  "form": 0.5,
  "void": 0.5,
  "flip_vertical": false,
  "calibration": { "gamma": [2.2, 2.2, 2.2], "gain": [0.6, 1.0, 1.0] },
  "enabled_content": ["Flux Capacitor"],
  "selected_content": "Flux Capacitor"
}
```

- `modules` lists `[x, y, address]` per module: its grid position and its jumper address (see [addressing](nova_protocol.md#module-addressing)). Addresses must be unique. A single module can be set up in the web app; a layout of several modules only here, and the web app then shows the address as not configurable.
- `ethernet_interface`, `webserver_port`, `simulator`, and `audio` can only be changed here; restart the server afterwards. `simulator` shows the desktop simulator instead of driving the hardware; both use the same renderer. `audio` starts the audio service; while it is off or failing, the web app hides Volume.
- Missing fields fall back to defaults. A file that cannot be parsed is moved to `nova_settings.json.invalid`, and the server starts with defaults.
- `calibration` holds the display calibration per channel (red, green, blue); set it on the web app's calibration page. Values outside gamma 1 to 3 and gain 0.2 to 1 are clamped.
- Content and palettes are stored by name. Unknown names fall back: content to all modules enabled and the first selected, the palette to the first palette.

## Audio output

Audio plays on the server machine's default output device, not in the browser, in both simulator and hardware mode. Select the device in the OS before starting Nova. For the Raspberry Pi, see [Native audio output](raspberry_pi_setup.md#native-audio-output).

Audio tests, from the repository root:

```sh
cargo test --manifest-path server/Cargo.toml audio
cargo test --manifest-path server/Cargo.toml audio_listening_previews -- --ignored --nocapture
cargo test --manifest-path server/Cargo.toml native_audio_output_smoke -- --ignored --nocapture
```

The preview test writes three unnormalized 30-second WAV files (`slow`, `bleeps`, `arpeggios`) to `nova-audio` in the OS temporary directory and needs no device. The smoke test needs an output device and stays muted.

## Adding content

1. Create a struct in `server/src/content/` and implement the `Content` trait from `server/src/content.rs`.
2. Register it in `all_content()`, keeping the alphabetical order of names.
3. Add a pictogram for its name in `webapp/src/ContentPictogram.tsx`, and rebuild the web app.

Content writes unscaled RGB to `next`; the renderer applies Brightness to a separate output image. Drive motion with the Flow-integrated phase from `advance()`, or for simulations its step in seconds from `advance_seconds()`, never wall-clock time, and reset animation state when `RenderState::should_reset()` is true; seed randomness (`common::Rng`) so a reset replays the same animation. `content/common.rs` has helpers for palettes, Heat and Void. `render()` must finish within the render budget (see the [README](../README.md#color-and-performance)).

## Diagnostics

`cargo test flux_form_sweep_dump -- --ignored --nocapture` in `server` writes `nova-flux-sweep.ppm` to the OS temporary directory. Its columns step Form from 0 to 1 by 0.125, and its rows are the palettes; each cell shows the middle slice of one module at Heat 0.5.

`cargo test content_form_sweep_dump -- --ignored --nocapture` writes `nova-content-sweep.ppm` with every content module: the columns step Form from 0 to 1, and each module has three rows, after 2, 6 and 15 s at Flow 0.5. Each cell shows the middle slice of two modules side by side. `NOVA_PALETTE`, `NOVA_HEAT` and `NOVA_VOID` change the palette (by name) and the controls.

`cargo test --release content_render_timing -- --ignored --nocapture` prints the render time per frame of every module, for one module and a 2 x 2 grid.

## Troubleshooting

- Modules do not respond: check jumper addresses, the Ethernet interface, and the module status in the web app. Inspect raw packets with `tcpdump` or Wireshark.
- Check the server log for missed sync or interface errors.
- No sound: check the OS default device, the audio permissions of the account running Nova, and whether another process holds the device exclusively. Display and audio errors are reported separately.
