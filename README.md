# Nova voxel display control software

Rust-based procedural content generation and sound for the Nova voxel display, with a desktop simulator and a React control interface.

Six content modules explore the low-resolution volume: **Orion's Belt** with a single noise primitive, and **Andromeda**, **Betelgeuse**, **Oort Cloud**, **Pleiades** and **Tannhäuser Gate**, which reinterpret the content of the former Java version. Four instruments, **Strata**, **Fathom**, **Shoal** and **Bayou**, play with the same controls, loosely following the light.

This README is the canonical project guidance for both human contributors and coding agents. Follow the control semantics and design constraints below when changing the project.

## Getting started

Run `cargo run --release` in the repository root. The first run creates `nova_settings.json` there; set `"simulator": true` (and `"audio": true` for sound), run again, and open `http://localhost:8080`.

- [Development](doc/development.md): building, settings, adding content, audio sketches, diagnostics
- [Raspberry Pi setup](doc/raspberry_pi_setup.md): installing and running on the display
- [Hardware protocol](doc/nova_protocol.md): packets, timing, and module addressing
- [Audio concept](doc/audio_concept.md): where the sound is heading and what is open

## The display

Nova is a modular RGB voxel LED display driven at 25 Hz (40 ms per displayed frame), which leaves approximately 20 ms per frame for rendering.

- Each module has a 50 x 50 cm base, is 100 cm high and contains 5 x 5 x 10 voxels: 250 LEDs.
- Most installations use a single module, configured in the web app. A grid of several modules is configured in the [settings file](doc/development.md#settings-file) only.
- Voxels are matte, white, ping-pong-like plastic spheres that diffuse the light. Output can be very bright in dark environments.

![Single Nova module displaying random colors](doc/nova_5x5x10.jpg)

## Artistic controls

One universal control set serves every content module and instrument. Keep this order consistent in structs, API payloads, and UI state. Every value ranges from 0 to 1. New installations and Restore defaults start every control at 0.5, except Volume at 0.

| Control                       | Meaning and implementation contract                                                                                                                                                                                                                |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Brightness** (`brightness`) | Global visual output level. Apply once in the renderer, after content generation; never feed the scaled output back into content.                                                                                                                  |
| **Volume** (`volume`)         | Independent audio output level, after the complete instrument mix. Zero mutes, with a short click-preventing ramp, then stops rendering: the tides continue, the music pauses and fades back in from there.                                        |
| **Palette** (`palette`)       | One of the curated palettes, by name. In sound, the tonal world.                                                                                                                                                                                   |
| **Instrument** (`instrument`) | One of the instruments, by name, independent of the content.                                                                                                                                                                                       |
| **Heat** (`heat`)             | Gray at 0, the palette as published at 0.5, up to twice its saturation at 1, preserving hue. Does not change occupancy, motion speed, or Form. Single-color modules use it to pick their color instead (see Content). In sound, from cold to warm. |
| **Flow** (`flow`)             | Evolution rate. Zero freezes the visual composition but retains slow audio evolution; changing the value must not jump either timeline.                                                                                                            |
| **Form** (`form`)             | Structure, from horizontal layers through columns, blobs and patches to per-voxel grain. Also sets how many palette colors show at once. In sound, texture, from still layers to grain.                                                            |
| **Void** (`void`)             | Fraction of the volume left dark. Lit voxels keep the same brightness at any value. In sound, gaps: the rhythm (none at 0, full from 0.1 to 0.5, dissolving towards 1), a thinner spectrum and distance, never silence.                            |

### Design principles

- Use the same controls everywhere, with creative interpretations that preserve their shared meaning. Keep Brightness out of synthesis and Volume out of visual rendering.
- `RenderState` only carries values; it must not remap their semantics. The exceptions are tides (below) and fades: a content or palette change dims the old one through Void, swaps at the darkest point and lets the new one come back, with short glints sparkling as the fade lifts Void (constants in `voxel/src/renderer.rs`, glints in `voxel/src/sparkle.rs`). Sliders always show the user's setting.
- Keep the control surface simple. Add presets later if needed.
- The web app shows icons without text labels, so users discover the controls by exploring (`SHOW_LABELS` in `webapp/src/ControlRow.tsx`; screen readers always get them). The icons are Material Symbols, inlined in `webapp/src/icons.tsx`.
- Content, Instrument and Palette are chosen the same way: a strip shows the current choice, and a tap selects the next one, wrapping around. The choice is sent after a second without taps, or at once when leaving the page. The content strip shows a spectral signature (`webapp/src/ContentSignature.tsx`), the instrument strip, shown only with Volume, an oscillogram (`webapp/src/InstrumentSignature.tsx`), both derived from the name, so new ones get their own. The content strip is hidden when fewer than two modules are enabled; a sole enabled module is selected.
- Keep dependencies minimal in both the web app and the Rust crates; prefer existing capabilities for small changes.

## Tides

Tides are slow, subtle variation brought in by the system rather than the user, defined in `core/src/tides.rs`: smooth, bounded swells of two sines with golden-ratio periods, so they never loop exactly and need no state.

- Global tides move the effective Heat (37 s, ±0.08), Flow (23 s, ±0.10), Form (53 s, ±0.06) and Void (29 s, ±0.08) around the sliders, for the display and the sound alike. The swing tapers to nothing at 0 and 1, so both extremes stay exact.
- Content modules and the conductor can run their own tides on the same clock (`RenderState::tide_seconds`).
- `FREEZE_WITH_FLOW` (on): the tide clock only advances while Flow is above 0, so Flow 0 keeps the display still. `TIDE_DEPTH` scales every tide; 0 switches them off.
- Tides must never change the coefficient of a time term, or patterns jump; they only move bounded values.

## Content

Modules are registered in `all_content()` in alphabetical order of their names, which is also the order in the web app. Every module's complete specification is the pseudocode at the top of its file in `voxel/src/content/`; keep it in sync with the code. Shared helpers (palette mixing, Heat, Void by rank or as a cap, a deterministic random generator) live in `voxel/src/content/common.rs`.

Preserve readable structures at 5 x 5 x 10, and sample spatial patterns in voxel units so additional modules show more of the same field rather than a stretched one; particle counts scale with the number of modules.

| Module (file)                              | Form, 0 → 1                                                                           | Void                       | Heat                  |
| ------------------------------------------ | ------------------------------------------------------------------------------------- | -------------------------- | --------------------- |
| **Andromeda** (`andromeda.rs`)             | few large, merging metaballs → many small Pong balls with paddle flashes              | by rank (exact)            | saturation            |
| **Betelgeuse** (`betelgeuse.rs`)           | bobbing level → jumps → rolling → standing → choppy waves; rain and bubbles above 0.5 | water level (1 − Void)     | picks the water color |
| **Oort Cloud** (`oort_cloud.rs`)           | helix orbit → flock → lattice snakes that burst when trapped                          | shorter trails, then a cap | saturation            |
| **Orion's Belt** (`orions_belt.rs`)        | one 4D simplex noise: layers → columns → blobs → patches → grain                      | by rank (exact)            | saturation            |
| **Pleiades** (`pleiades.rs`)               | reaction-diffusion: layers → columns → blobs → patches → grain                        | by rank (exact)            | saturation            |
| **Tannhäuser Gate** (`tannhauser_gate.rs`) | flame bed → flame tongues → rising embers → star flares → sparks                      | lower flames, then a cap   | picks the flame color |

- Drive motion with the Flow-integrated phase. Nothing may change the coefficient of that phase, or the pattern jumps; variation over time goes through bounded terms. Simulated modules advance by `advance_seconds()`: real time at Flow 0.5, stopped at Flow 0. Randomness is seeded, so a reset replays the same animation.
- Void by rank keeps exactly the Void fraction dark; Void as a cap (`void_as_cap`) keeps at most the brightest 1 − Void fraction lit, for sparse modules whose dark voxels are already part of the picture.
- Heat as a picker, for modules whose Java ancestors had a single color, blends through the palette from its first color at 0 to its last at 1.
- The Java version (branch `nova_final_java_version`) had 27 content classes, merged here into five modules that each span the Form range; each file names its origins. Test patterns, Colorcube, CA_Alexander and SSP were not ported.

## Color and performance

- Palettes are curated sets in `core/src/palettes.rs`, compiled into the binary and listed alphabetically, with 1 to 6 colors and unique names. Colors are specified for light, in OKLCh, as a color temperature, or as Pantone sRGB hex; the file documents each source and what has held up on the display. `doc/palettes.svg` and `doc/palettes.png` show all palettes; see [Development](doc/development.md#diagnostics) to regenerate them.
- Neighbouring palette colors are mixed in Oklab over only the middle part of the way between them (`BLEND_WIDTH` in `content/common.rs`), so most voxels show palette colors as published. About two colors show at once for smooth structures and all for grain.
- Heat preserves hue: it stops increasing saturation before any channel would clip.
- Dark colors drive the LEDs at low power: warm ones vanish, bluish ones survive as dim color, and greens look lighter than on a monitor. Evaluate on the physical display.
- Calibration (`voxel/src/calibration.rs`) maps output values to LED duty cycles per channel, `gain · value^gamma`, after Brightness, on the hardware only; the simulator previews it only while the calibration page shows a pattern. The defaults, measured on Nova, are gamma 2.2 and gain 0.6 for red, 1 for green and blue. Restore defaults keeps the calibration.
- Aim for no more than 20 ms render time per frame on a Raspberry Pi 4. Keep turbulence and noise octaves in check.

## Audio

Audio is experimental. The instruments are independent of the content modules; the web app selects one (`DEFAULT_INSTRUMENT` in `audio/src/instruments.rs` for new installations), and another mixes in over 5 s on the shared musical time. Each instrument's description is the comment at the top of its file in `audio/src/instruments/`; keep it in sync with the code.

| Instrument | Character                                                                                                                   |
| ---------- | --------------------------------------------------------------------------------------------------------------------------- |
| **Strata** | the drone alone: an additive bed in just intonation over a synth bass, air, a pulse of sub thumps, ticks and stabs, a space |
| **Fathom** | the drone's chord as dub stabs through a resonant low-pass, a sub bass line, fills, breaks and dub throws                   |
| **Shoal**  | the chord on beating bells that interlock like a gamelan's, closed by a gong                                                |
| **Bayou**  | a pulsing guitar and a combo organ through a rotary speaker, between ambient electronica and sixties ballads                |

- Every instrument plays the drone (`instruments/drone.rs`). Palette sets the tonal world: the mean hue picks the root, each color adds a tone, and the harmony moves in sections that end on a phrase start. Flow sets the rate of evolution and the tempo (40 to 120 BPM, from the slider without its tide); at Flow 0 the rhythm rests and the drone keeps breathing. Heat moves from cold (glassy, straight) to warm (round, swung), keeping the pitches and the loudness.
- Rhythm is data, not chance: each instrument's score (`score.rs`) of weighted steps, which the conductor (`engine/conductor.rs`) plays; Void, through `params::RHYTHM`, decides how much of it plays. Instruments give the hits their sound and never decide where steps fall.
- The light shapes the sound loosely: the renderer measures features of every frame before Brightness (level, balance, width, height, density, activity, hues) and puts them on the bus. Instruments answer them smoothed over seconds, rarely one to one, never voxel by voxel; `LIGHT_WEIGHT` in `engine.rs` scales the coupling. Sound and light should feel related, not the image being played.
- The crate `audio` (`nova-audio`) knows neither the host nor the display: it plays from the bus and applies the global tides itself. Its layers are listed in `audio/src/lib.rs`. Instruments are an enum with presets as plain data; their parts talk through `signals.rs`, a plain struct; add a signal as a field rather than routing modulation generically.
- The audio callback must not lock, allocate, log or access files; the engine builds every instrument up front. The DSP is plain code without audio dependencies and without heavily generic abstractions, which run far slower in debug builds (the crate builds with `opt-level` 2 there).
- Output goes through CPAL to the OS default device of the machine running Nova, at its sample rate, in periods of 2048 frames (shorter ones underrun on the Pi); browsers only control it. The master is mono below 120 Hz, with an infrasonic high-pass and a look-ahead limiter at about −1 dBFS before Volume, which uses a squared curve. Audio is off unless `audio` is set in the settings file. Device errors are reported apart from display status and retried; dropouts are counted and logged.
- Tests do not establish sound quality. The sound is designed for full-range speakers with a subwoofer: tune at installation listening levels, starting low, and profile with visuals active on the Raspberry Pi. Offline sketches are in [Development](doc/development.md#audio-output).

## Control API

- `GET /api/get-state` returns `brightness`, `volume`, `palette`, `instrument`, `heat`, `flow`, `form`, `void` and other settings. Content and palettes are referenced by name: `available-content`, `enabled-content`, `selected-content`, `palettes` (each with `name`, CSS `hex`, `spec` and `source`, in display order) and `instruments`. `volume` is -1 while audio is disabled or failed, and the web app then hides Volume.
- `POST /api/{brightness|volume|heat|flow|form|void}?value=<0..1>` updates a control. Select by name with `POST /api/{palette|instrument|selected-content}?value=<name>` and `POST /api/enabled-content?value=<name>,<name>`. `POST /api/flip-vertical?value=<true|false>` flips the content upside down. Invalid values are rejected with 400 and change nothing; the web app then reloads its state. Settings are saved after 500 ms without changes; pending changes are not flushed on shutdown.
- `POST /api/reset` queues a one-shot module reset (no effect in simulator mode). Modules are also reset whenever the set of modules answering status polls changes, for example after `POST /api/module0-address`.
- `POST /api/reload` saves the settings and exits Nova, for a process manager to restart it. Before exiting, as on Ctrl+C, SIGTERM and SIGHUP, the modules are reset so the display stays dark; a second signal exits right away.
- Calibration: `POST /api/{gamma|gain}-{red|green|blue}?value=<v>` (gamma 1 to 3, gain 0.2 to 1), `POST /api/reset-calibration`, and `POST /api/calibration-pattern?value=<off|gray|red|green|blue|palette>`, which shows a test pattern for at most 10 minutes after the last selection.
- `POST /api/restore` restores default settings, except those only configured in the settings file and the calibration.
- `GET /api/get-status` reports the display status (`status-ok`, `status-message`) and the audio status (`audio-ok`, `audio-message`).
- Only `POST` changes anything, and only from Nova's own pages: cross-site browser requests (`Sec-Fetch-Site`, or `Origin`) are rejected with 403, and the host name must be local (an IP address, a name without dots, or under `.local`, `.lan`, `.home.arpa`, `.internal` or `.localhost`) against DNS rebinding. There is no authentication: never expose the port to the internet.

## Development checks

From the repository root:

```sh
cargo test
npm --prefix webapp run build
cargo build
```

Build the web app before Nova so the embedded UI matches the API, and run `npx --no-install eslint src` from `webapp` for lint. TypeScript stays on 6.0.x until `typescript-eslint` supports 6.1 or newer. Automated tests and the simulator do not replace evaluation on the physical display and profiling on the Raspberry Pi.

## License

MIT, see [LICENSE](LICENSE).
