# Audio concept

Where the sound is heading and what is still open. What it does now is in the [README](../README.md#audio) and the code; the commands and the offline sketches are in [Development](development.md#audio-output).

## Direction

- **One instrument, played with the sliders**, not a sound per module: drone and ambient after Biosphere, from the cold spaces of _Substrata_ to the pulses of _Microgravity_. Strata, Fathom and Shoal share one world (the harmony, the conductor, the controls); each content module plays one of them.
- **One meaning per control**, shared with the light: Heat is color (glassy to round, straight to syncopated), Flow is time (tempo, evolution, sections), Form is texture (continuous to granular), Void is gaps (in time the rhythm, in space distance and a thinner spectrum). Brightness stays out of the sound.
- **Structure, not chance**: the rhythm comes from scores of weighted steps after dub techno, ambient music and gamelan, and varies with the harmony's sections, fills, breaks and tides. Instruments give the hits their sound and never decide where steps fall.
- **A subtle coupling to the light**: the sound answers slow, smoothed features that describe the light, each answer a setting with 0 to ignore it, rarely one to one. Nothing is triggered voxel by voxel, and modules have no hints of their own. A listener should sense that sound and light belong together, not hear the image being played.
- **Not a DAW**: voices and parameters are added where they widen the sonic space, not for flexibility's sake, and the web app keeps its simple controls.
- **The target system** is full range with a subwoofer, for example two Genelec 8030 with a 7050: real sub content at 30 to 60 Hz, mono below 120 Hz, several slowly beating partials rather than one static low sine (room modes), fixed gain staging with a clean limiter, and neither artefacts (aliasing, metallic ringing) nor piercing resonant highs, which such monitors and sensitive ears reveal. On the Raspberry Pi, a USB DAC or audio HAT.
- **Listening decides**: the sketches' measurements catch technical problems only. Levels stay roughed in until one listening session on the target system.

## Open

### Next

- **Profile on the Raspberry Pi 4** with visuals active, every instrument and a mix between two. The last profile, about 17% of one core, came before Fathom, Shoal, the mix, the grains and the longer delay lines.
- **The mix between instruments**: patterns switch to the new score at a phrase start rather than blending into it, and the bass swaps there too; two beds playing the same chord may not sum well at equal power; an instrument that mixes in again starts where it stopped, its echoes and hall still decaying. A sketch of a mix is missing; how long a mix takes (`MIX_SECONDS`, 5 s) and which module plays which instrument (`CONTENT_INSTRUMENTS` in `host/src/state.rs`) are still first guesses.

### To listen to

Questions for the target system:

- **Overall**: Does the drone have its own character rather than a generic pad? Is the bass full but steady, without pumping or booming notes? Do the palettes give distinct tonal worlds, and do their crossfades work? Are the modules distinguishable with every control at 0.5?
- **Void**: Is the build-up, all within Void 0 to 0.1, too touchy? Is the full rhythm right (`params::FULL`, what Form 0.8 played before), and does the dissolve sound like the rhythm drifting away rather than only thinning? Is Void 0, dense and close, calm enough (`REACH` in the space, `NARROW` in the air)? Strata's score or its polymeter variant (`strata::Preset::POLYMETER`)?
- **Form**: Do Strata's grains and dust, Fathom's legato and clipped stabs and Shoal's damped bells read as one axis of texture?
- **Flow**: The tempo runs linearly from 40 to 120 BPM, so the middle is 80 BPM: right, or should a curve keep about 95 there?
- **Fathom**: Does the space carry the groove with one stab a bar, and do the second head's irregular echoes feel intended? Do the cold pings ring without getting sharp? How often should it throw, for short and for long listening (`Throws::SHORT`, `Throws::LONG`)? Do the three bass lines vary enough? Is its dark drone a good counterpart to Strata's?
- **Shoal**: Do the bells shimmer without sounding out of tune, and is their ceiling of 2.5 kHz right? Should the highest degrees, up to about 2.4 kHz, fold down an octave? Do the interlocking parts read as one stream, and the arpeggio against the bar as intended? Is the gong a frame or a disturbance?
- **Ticks**: Cold ticks ring at 7 kHz with a Q of 9: too sharp?
- **The light**: Do height, density, balance, glow, width and Shoal's strike and voicing feel related without being heard as the image played? Is `LIGHT_WEIGHT` 1 right?

### The listening session

- Warm rhythmic scenes measure 3 to 4 dB louder than cold ones in every instrument.
- Fatigue over long listening (`fathom_10_harmonic_motion`, `fathom_11_long_listening`).
- The instruments' `level`, the reverb, the dub delay and the stereo image. The baseline: with every control at 0.5, each instrument measures about −18.5 LUFS, −16.4 LUFS in the 95th percentile of its momentary loudness, and peaks of −27 to −28 dB between 1 and 4 kHz, where Fathom's gaps between stabs stay darker, as dub stabs are.
- Rooms: listening in different rooms, perhaps a low-pass per room in the settings file.

### Later

- **Pulsar**, a gentler Fathom: a 12 dB filter without the ping, stabs heard only in their echoes (ghost stabs on the Looseness tide), a send that alternates sides bar by bar, Void raising the feedback. A preset of Fathom once ghost stabs and the alternating send exist.
- **A grain layer**: granular playback of buffers the instrument records from itself, later samples. Samples and spoken fragments are left out for now.
- **Tuning ideas**: dub throws of the chord; the pulse into the fog (less transient thumps and ticks, deeper with Void); a bass accent that stays out of the chord's range; looseness for the drone's pumping; `coupling` as a parameter; a pattern that shifts by a step every few phrases and comes back around, as in Reich's _Clapping Music_; slower audio tides that shape sections over minutes.
- **The light**: local flashes as onsets; features that follow Flip Vertical; the sound describing itself back to the light.
- **A performer mode** on a Mac laptop, with an instrument selector and more expressive input (a native interface or MIDI), after Scriabin's light organ. It can afford more than the Pi's budget.
- **Output**: the highest-resolution sample format a device offers (F32, I32 or I24 before I16), dither for 16-bit output; quality scaled down (reverb channels, partials) only if a target needs it.
