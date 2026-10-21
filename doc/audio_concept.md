# Audio concept

Where the sound is heading and what is open. What it does now is in the [README](../README.md#audio) and the code; the sketches are in [Development](development.md#audio-output).

## Direction

- **One instrument, played with the sliders**, not a sound per module: drone and ambient after Biosphere, from _Substrata_ to _Microgravity_. The instruments share the harmony, the conductor and the controls.
- **One meaning per control**, shared with the light: Heat is color, Flow is time, Form is texture, Void is gaps.
- **Structure, not chance**: scores of weighted steps after dub techno, ambient music and gamelan, varied by sections, fills, breaks and tides.
- **A subtle coupling to the light**: slow, smoothed features, never voxel by voxel, and no per-module hints.
- **Not a DAW**: add voices and parameters only where they widen the sonic space.
- **The target system**: full range with a subwoofer (for example two Genelec 8030 with a 7050), mono below 120 Hz, clean gain staging, no aliasing, metallic ringing or piercing highs. On the Raspberry Pi, a USB DAC or audio HAT.
- **Listening decides**: the sketches catch technical problems only.

## Next

- **Profile on the Raspberry Pi 4** with visuals active, every instrument and a mix. The last profile, about 17% of one core, predates Fathom, Shoal, Bayou and the mix.
- **The mix between instruments**: patterns and bass switch at a phrase start rather than blending; two beds on the same chord may not sum well; a sketch of a mix is missing; `MIX_SECONDS` (5 s) is a first guess.

## To listen to on the target system

- **Overall**: Does the drone have its own character? Is the bass full but steady? Are the palettes distinct tonal worlds?
- **Controls**: Is the Void build-up (0 to 0.1) too touchy, and does the dissolve sound like drifting away? Does Form read as one axis of texture across instruments? Should the tempo curve keep about 95 BPM in the middle instead of 80?
- **Fathom**: Does one stab a bar carry the groove? How often should it throw (`Throws::SHORT`, `Throws::LONG`)?
- **Shoal**: Do the bells shimmer without sounding out of tune? Is the gong a frame or a disturbance?
- **Bayou**: Does the pulse sound electronic rather than like a guitarist practising? Is the rotary speaker's chorale calm enough for long listening, and its extreme at the grain new territory or just rough?
- **Ticks**: Cold ticks ring at 7 kHz with a Q of 9: too sharp?
- **The light**: Related without being heard as the image played? Is `LIGHT_WEIGHT` 1 right?
- **Levels**: Warm rhythmic scenes measure 3 to 4 dB louder than cold ones. At every control 0.5, each instrument measures about −18.5 LUFS. Check fatigue over long listening, and different rooms.

## Later

- **Pulsar**, a gentler Fathom preset: stabs heard only in their echoes, a send alternating sides.
- **A grain layer** of buffers the instrument records from itself, later samples.
- **Tuning ideas**: a pattern that shifts by a step every few phrases, as in Reich's _Clapping Music_; slower audio tides that shape sections over minutes.
- **The light**: local flashes as onsets; the sound describing itself back to the light.
- **A performer mode** on a Mac laptop, with more expressive input (a native interface or MIDI).
- **Output**: the highest-resolution sample format a device offers, dither for 16-bit output.
