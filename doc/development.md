# Development

How to build, run, configure, and extend Nova and its web app. Control semantics, design constraints, the API, and the development checks are in the [README](../README.md).

## Prerequisites

- Rust 1.94 or newer, with Cargo
- `libpcap` development headers
- Audio: on Linux, ALSA development headers (`libasound2-dev`) and `pkg-config`; on macOS, 14.2 or newer (CoreAudio); on Windows, WASAPI
- Node.js 20.19+, 22.13+, or 24+, only for changing the web app. The built UI is committed in `host/src/www` and embedded in the `nova` binary.

## Project layout

```text
Cargo.toml        # The workspace of the four crates
core/             # nova-core: the bus the parts meet on, controls, palettes, tides, features of the light, status
voxel/            # nova-voxel: the light: content (one file per module in src/content/), renderer,
                  # calibration, the light meter, the hardware driver and the simulator
audio/            # nova-audio: the sound: DSP, voices, instruments, engine and native output;
                  # offline sketches in sounddesign/sketches
host/             # nova-host, the binary `nova`
├── src/main.rs   # Wires the parts together
├── src/state.rs  # Settings and runtime state; owns the bus
├── src/voxel.rs  # The host, as the display sees it: the settings of every frame, reset and shutdown
└── src/web.rs    # HTTP API and the embedded web app (built into src/www)
webapp/src/       # React + Material UI control interface
```

The terms, held to in code and docs:

| Term           | Means                                                                                                                                |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **Host**       | The application that runs the parts: `nova-host`, the binary `nova`. `nova_voxel::Host` is the host as the display sees it.          |
| **Renderer**   | Turns the bus into light or sound and describes its output back onto it: `nova-voxel` (hardware or simulator) and `nova-audio`.      |
| **Controller** | Where people play the instrument: the web app and its API.                                                                           |
| **Bus**        | What the parts share (`nova_core::Bus`): the controls as set, the tide clock, the features of the light, and a status for each part. |
| **Settings**   | What is saved, in `nova_settings.json`.                                                                                              |
| **State**      | The host's settings plus what only lives while Nova runs: the bus, requests, the test pattern.                                       |

The crates depend in one direction: `nova-voxel` and `nova-audio` build on `nova-core` and never on each other (only the audio sketches use `nova-voxel`, as a dev-dependency), and the host builds on all three. The host puts the controls on the bus whenever they change; the display renders with them and adds the tide clock and the features of its light with every frame; the sound plays from all of it; both leave their status there. Only the display asks the host for more, through `nova_voxel::Host`: what to show and how, and the requests to reset or shut down. `cargo build` and `cargo test` in the repository root cover every crate; `-p nova-voxel` and the like pick one. The parts follow one convention:

- Components are structs with methods, such as `Bus`, `Renderer` and the audio `Engine`.
- Services that run in the background start with `X::start(…) -> io::Result<X>` and live as long as their handle: `WebServer` and the audio `Output`. Services that take over the calling thread run with `X::run(…)`: `Hardware` and `Simulator`.
- No generics where a plain type or `dyn` does: the display's host is a `Box<dyn Host>`.
- Free functions are for stateless helpers, such as `all_content()`, `tides::effective()` and the packet builders of the hardware protocol.

## Build and run

Run Nova from the repository root:

```sh
cargo run --release
```

It serves the web app on port 8080 and reads `nova_settings.json` from the working directory, creating it on first run. By default it drives the hardware with audio off. For desktop development, set `"simulator": true` and, for sound, `"audio": true` in the [settings file](#settings-file), then restart.

For web app development, run a hot-reloading server from `webapp`; it proxies `/api` to Nova on port 8080:

```sh
npm install
npm run dev
```

`npm run build` writes the production bundle to `host/src/www`. Rebuild Nova afterwards to embed it.

## Settings file

`nova_settings.json` holds all settings:

```json
{
  "ethernet_interface": "eth0",
  "webserver_port": 8080,
  "simulator": false,
  "audio": false,
  "instrument": "Strata",
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
  "enabled_content": ["Orion's Belt"],
  "selected_content": "Orion's Belt"
}
```

- `modules` lists `[x, y, address]` per module: its grid position and its jumper address (see [addressing](nova_protocol.md#module-addressing)). Addresses must be unique. A single module can be set up in the web app; a layout of several modules only here, and the web app then shows the address as not configurable.
- `ethernet_interface`, `webserver_port`, `simulator`, `audio` and `instrument` can only be changed here; restart Nova afterwards. `simulator` shows the desktop simulator instead of driving the hardware; both use the same renderer. `audio` starts the audio service; while it is off or failing, the web app hides Volume. `instrument` picks the sound, `Strata` or `Fathom` (see the [README](../README.md#audio)); Restore defaults keeps it.
- Missing fields fall back to defaults. A file that cannot be parsed is moved to `nova_settings.json.invalid`, and Nova starts with defaults.
- `calibration` holds the display calibration per channel (red, green, blue); set it on the web app's calibration page. Values outside gamma 1 to 3 and gain 0.2 to 1 are clamped.
- Content and palettes are stored by name. Unknown names are dropped; if no enabled content remains, all modules are enabled. An unknown selection falls back to the first enabled module, an unknown palette to the first palette.

## Audio output

Audio plays on the default output device of the machine running Nova, not in the browser, in both simulator and hardware mode. Select the device in the OS before starting Nova. For the Raspberry Pi, see [Native audio output](raspberry_pi_setup.md#native-audio-output).

Audio tests, from the repository root:

```sh
cargo test -p nova-audio
cargo test audio
cargo test native_audio_output_smoke -- --ignored --nocapture
```

The first command runs the audio crate's tests, the second every test about audio across the crates. The smoke test needs an output device and stays muted.

The sketches render the instruments offline and need no device (see [Audio concept](audio_concept.md#sketches)), from the repository root:

```sh
cargo run --release -p nova-audio --example sketches -- fathom
```

They write WAV files, spectrograms and `report.md` to `nova-audio-sketches` in the OS temporary directory, or to `NOVA_AUDIO_DIR`; the argument renders only scenarios whose name contains it, such as `void`, `fathom` or `module`, and without one, all. `… --example sketches -- ducking 0.5 0.5` prints how Strata's drone dips with the thumps at the given Heat and Flow, `… -- features` what every content module hands to the sound.

## Adding content

1. Create a struct in `voxel/src/content/` and implement the `Content` trait from `voxel/src/content.rs`.
2. Register it in `all_content()`, keeping the alphabetical order of names. The web app derives its signature from the name.

Content writes unscaled RGB to `next`; the renderer applies Brightness to a separate output image. Drive motion with the Flow-integrated phase from `advance()`, or for simulations its step in seconds from `advance_seconds()`, never wall-clock time, and reset animation state when `RenderState::should_reset()` is true; seed randomness (`common::Rng`) so a reset replays the same animation. `content/common.rs` has helpers for palettes, Heat and Void. `render()` must finish within the render budget (see the [README](../README.md#color-and-performance)).

## Diagnostics

`cargo test orions_belt_form_sweep_dump -- --ignored --nocapture` writes `nova-orions-belt-sweep.ppm` to the OS temporary directory. Its columns step Form from 0 to 1 by 0.125, and its rows are the palettes; each cell shows the middle slice of one module at Heat 0.5.

`cargo test content_form_sweep_dump -- --ignored --nocapture` writes `nova-content-sweep.ppm` with every content module: the columns step Form from 0 to 1, and each module has three rows, after 2, 6 and 15 s at Flow 0.5. Each cell shows the middle slice of two modules side by side. `NOVA_PALETTE`, `NOVA_HEAT` and `NOVA_VOID` change the palette (by name) and the controls.

`cargo test --release content_render_timing -- --ignored --nocapture` prints the render time per frame of every module, for one module and a 2 x 2 grid.

## Troubleshooting

- Modules do not respond: check jumper addresses, the Ethernet interface, and the module status in the web app. Inspect raw packets with `tcpdump` or Wireshark.
- Check Nova's log for missed sync or interface errors.
- No sound: check the OS default device, the audio permissions of the account running Nova, and whether another process holds the device exclusively. Display and audio errors are reported separately.
