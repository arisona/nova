## NOVA Voxel Display

- Modular RGB voxel LED system driven at 25 Hz (40 ms per frame, however with the current architecture only 20ms per frame)
- Each module: 50×50 cm base, 100 cm high → 5×5×10 voxels (250 LEDs)
- Voxels: ping‑pong–like white plastic spheres; matte; diffuse light
- High output; modules can get very bright in dark environments

## Artistic Controls (Canonical)

Use a single, universal control set for all content modules. Keep the order
consistent everywhere (structs, API payloads, UI state):

1. glow — global brightness dimmer (0–1). Applied last as an overall multiplier.
2. tone — dominant color steer (0–1 → 0–360° hue). Modules decide color space; UI preview uses OKLCH for perceptual uniformity.
3. punch — visual impact (0–1). Modules map to saturation/contrast/feature size; higher values should read “bolder”, monotonically.
4. flow — motion feel over time (0–1). Modules map to animation rate and/or motion amplitude.
5. form — order ↔ chaos (0–1). 0 = regular/predictable, 1 = turbulent/organic/noisy.

Design principles
- Same controls for every module; modules interpret them creatively.
- RenderState only carries values; it must not remap semantics.
- Keep surface simple; add presets later if needed.

API shape
- GET `/api/get-state` returns: `glow`, `tone`, `punch`, `flow`, `form` (plus other settings).
- SET `/api/{glow|tone|punch|flow|form}?value=<0..1>` updates a control and persists it.

Color notes
- Prefer OKLCH for palette operations and previews to keep saturation/lightness perceptually even across hues.
- Tone preview chip uses `oklch(L C Hdeg)` with sensible defaults.

Performance note
- Aim for ≤ 20 ms render time per frame on RPi 4 (50 Hz loop). Keep turbulence/octaves in check.
