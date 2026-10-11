# Audio concept: Nova as an instrument

Status: work in progress. The instrument is live (it replaced the three-voice synth) and ready for tuning; the [README](../README.md#audio) summarizes what it does. This document keeps the reasoning, the open questions and the offline [sketches](#sketches).

## Aim

Nova produces drone and ambient sound with its own character: one instrument that can be played with the sliders, not a set of per-module sound tracks. The reference is Biosphere, from the sustained, cold spaces of _Substrata_ to the pulsing patterns of _Microgravity_. Sliders and content together move the sound between cold and warm, cosmic and sci-fi, ambient and rhythmic.

Sound and light reference each other without being locked together: they share the controls and the tide clock, and the rendered image shapes the sound through slow, smoothed features, never by triggering notes voxel by voxel.

## Target system

Sound is designed for a high-end system with full bass extension, for example two Genelec 8030 with a 7050 subwoofer (down to about 25 Hz, bass-managed below about 85 Hz). Installations vary from living rooms to offices and exhibitions; Volume adapts the level, the design does not depend on the room.

What this implies:

- **Real sub content.** Drone roots and pulses may sit at 30–60 Hz.
- **Mono below about 120 Hz.** The subwoofer sums its channels; stereo decorrelation applies only above, otherwise the bass partly cancels.
- **Headroom.** Sub content dominates the peak budget: an infrasonic high-pass at 20 Hz, fixed gain staging, loudness-normalized layers and a clean look-ahead limiter instead of saturation of the whole mix.
- **Room modes.** Avoid a single static low sine; several close partials with slow beating and drift spread the bass across the room.
- **Transparency.** Monitors like these reveal artefacts: band-limited or additive oscillators, per-sample parameter smoothing, no aliasing from nonlinearities, a dense modulated reverb without metallic ringing.
- **Output hardware.** On the Raspberry Pi, a USB DAC or audio HAT with balanced outputs; the analog jack is not good enough.

## Layers

| Layer     | Method                                                                                                                                                    | Role                                    |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- |
| **Bed**   | Additive partial bank in just intonation: each tone a set of partials with slowly beating pairs; the root is a synth bass with its own spectrum and level | The continuous drone                    |
| **Air**   | Pink noise through narrow resonators tuned to the bed's tones                                                                                             | Wind and breath, strongest when cold    |
| **Pulse** | Tuned sub thumps, noise ticks and filtered chord stabs on a step grid                                                                                     | Rhythm, from heartbeat to pattern       |
| **Space** | Long modulated FDN reverb plus a tempo-synced dub delay with a filter and saturation in the loop                                                          | Depth, distance and the signature sound |

Planned later: a **grain** layer (granular playback of buffers the instrument records from itself, later also samples), and possibly samples and spoken fragments. Samples are deliberately left out for now.

## Mapping

| Source             | Sound                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Palette**        | The tonal world. The chroma-weighted mean hue picks the root on the circle of fifths; every color adds a tone whose interval follows its hue distance to the mean (close hues give fifths and fourths, contrasting hues thirds and sixths), its lightness the octave, and its chroma the level. A palette change crossfades to the new tones.                                                                                                                                                                                                       |
| **Heat**           | Cold to warm, preserving the pitch set as visual Heat preserves hue. Cold is thin and glassy: pure fundamentals, weak low harmonics, slightly stretched upper partials, the upper tones and the air in front, straight rhythm, short metallic ticks, short thumps. Warm is round: strong low harmonics with rolled-off highs, the lower tones in front, wider beating and a gentle wow, soft breathy air, swing, longer and slightly saturated thumps, darker stabs and more dub feedback. A K-weighted level estimate keeps the loudness constant. |
| **Flow**           | Time. The rate at which the bed evolves (with a nonzero minimum, so Flow 0 still breathes) and the tempo (about 64–124 BPM, from the slider setting without its tide, so the tempo does not wobble). Musical time is integrated and never jumps. At Flow 0 the pulse rests: a frozen image gets a timeless drone.                                                                                                                                                                                                                                   |
| **Form**           | Ambient to rhythmic, as the visuals go from layers to grain: pure drone below about 0.15, thumps from 0.2 (one per bar, up to four), ticks from 0.35, chord stabs into the dub delay from 0.45, all growing denser towards 1. Patterns change only at bar boundaries.                                                                                                                                                                                                                                                                               |
| **Void**           | Space instead of silence. Like Void by rank in the visuals, only the strongest share of partials keeps sounding, at constant loudness; at the same time the sound moves away (more reverb, softer highs). Void 0 is dense and close, 0.5 the neutral middle, 1 a few partials far away; it never goes silent.                                                                                                                                                                                                                                       |
| **Volume**         | Output level after the complete mix, as today.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **Brightness**     | Not used, as today.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **Tides**          | The instrument receives the effective controls after the global tides, so sound and light breathe together. Slower audio-only tides can later shape sections lasting minutes.                                                                                                                                                                                                                                                                                                                                                                       |
| **Image features** | Taken from the content image before Brightness, at 25 Hz, smoothed over seconds. The energy of the ten voxel layers weights ten frequency bands, low at the bottom and high at the top (a Nova module is a vertical spectrum); visual activity animates the bed and the dynamics of the pulse; a sudden rise in activity may add one dub stab on the next free off-beat.                                                                                                                                                                            |
| **Module hints**   | Planned: a few numbers per module with a neutral default in the `Content` trait (for example cosmic to sci-fi, register, rhythm bias), so modules color the instrument without audio code of their own.                                                                                                                                                                                                                                                                                                                                             |

## Rhythm

Rhythm must not feel random. The backbone is deterministic, and variation is slow and controlled:

1. **Euclidean patterns**: Form sets how many pulses are spread evenly over 16 steps.
2. **Polymeter**: loops of different lengths (16, 12 and 7 steps) phase against each other, for long but predictable cycles.
3. Later, if needed: **repetition with slow mutation** (a seeded pattern repeats for many bars and changes one step at a time).

Image features change dynamics and accents, not where the steps fall. Variants 1 and 2 are rendered side by side for comparison.

## Constraints

The audio callback does not lock, allocate, log or touch files; controls are smoothed inside the instrument; Volume is applied last with a click-free ramp; everything must run comfortably on a Raspberry Pi 4 next to the renderer. Void is shared with audio, and sound is loosely coupled to the image through features. The live instrument plays the Euclidean rhythm (`RHYTHM` in `audio/output.rs`); the sketches compare both.

## Sketches

`cargo test --release audio_sketches -- --ignored --nocapture` in `server` renders the instrument offline, driven by the real content modules, tides and image features, to `nova-audio-sketches` in the OS temporary directory, or to the directory in `NOVA_AUDIO_DIR`. `NOVA_SKETCH` limits the run to scenarios whose name contains its value.

Scenarios: Heat, Form (in both rhythm variants), Void and Flow sweeps, every palette in turn, four corners (cold and warm, ambient and rhythmic), and every content module with all controls at 0.5. For each it writes a 32-bit float stereo WAV at 48 kHz and a spectrogram (PPM, 20 Hz to 20 kHz on a logarithmic axis), and `report.md` lists the integrated loudness (ITU-R BS.1770) and loudness range, sample peak, crest factor, the share of energy per frequency band and the energy per octave band (the spectral balance), the side-to-mid ratio below 80 Hz (the mono check for the subwoofer), how much faster than real time the instrument rendered, the visual activity, and how many steps each pulse voice played.

The sketches measure about −21 LUFS for the drone and −19.5 LUFS with the pulse, peaks below −3 dBFS, and side signal more than 40 dB below the mid signal under 80 Hz. On an Apple M2 they render about 50 times faster than real time in release builds and 3 times in debug builds. The reverb and the limiter are our own: an earlier FunDSP reverb, built from generic combinators, ran at 0.3 times real time in debug builds and made the stream drop out.

The measurements catch technical problems; they say nothing about whether it sounds good. Listening on the target system decides. Questions for listening:

- Does the drone have its own character, between cold and warm, without becoming a generic synth pad?
- Is the bass full but steady (no pumping from beating, no booming notes in the room)?
- Euclidean or polymeter: which rhythm feels intended rather than random?
- Do thumps, ticks and stabs sit in the drone, or on top of it? Is the dub delay too much or too little?
- Does Void at 0.5 sound like a neutral middle, and Void 1 far away but still present?
- Do the palettes give distinct tonal worlds, and do the crossfades (including the air resonators gliding) work?
- Are the modules distinguishable with all controls at 0.5, and is that coupling strong enough?

## Plan

1. Offline harness and first sketches: bed, air and space; pulse in Euclidean and polymeter variants. _(done)_
2. Go live: the renderer publishes features and effective controls to the audio thread, and the instrument replaces the three-voice synth. _(done)_
3. Listen and tune, step by step _(current)_:
   1. Tonal balance: fewer highs at their sources, the bed further back, a synth bass on the root. _(done)_
   2. A warm bed that sounds less like an organ: noise, movement, slight inharmonicity.
   3. Slow harmonic motion: root and voicing evolve over minutes.
   4. A narrower tempo range; tides vary the regularity of the rhythm rather than its speed.
   5. Later: reverb, delay and stereo image; how the pulse blends with the drone; coupling to the image; a per-room low-pass setting in the settings file; the grain layer and module hints.
4. Profile on the Raspberry Pi with visuals active, and listen on the target system in different rooms.
