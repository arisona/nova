//! Flux: the unified content module.
//!
//! One noise primitive covers the whole range. `form` sweeps through five fields that
//! differ only in their per-axis frequencies, in this order:
//!
//!   layers   varies along z only: horizontal layers           (form 0)
//!   columns  varies across x/y: full-height columns with a slow ripple
//!   blobs    broad volumes
//!   patches  smaller volumes, the step between blobs and grain
//!   grain    neighbours uncorrelated: per-voxel random        (form 1)
//!
//! The fields are evenly spaced, so blobs sit at form 0.5, and neighbouring fields
//! crossfade in between. Each field also sets how many palette colors show at once:
//! about two neighbouring colors with soft gradients for smooth structures, all colors
//! for grain.
//!
//! ```text
//! inputs:  x ∈ [0..n], y ∈ [0..m], z ∈ [0..9]   voxel coordinates, z vertical
//!          A single module is n = m = 4 (5 × 5 × 10). A grid of modules extends x/y
//!          (n = 5·modules_x − 1, m = 5·modules_y − 1). All spatial constants are in
//!          voxel units, so more modules show more of the same field, not a stretched one.
//!          t = Flow-integrated phase (Flow 0 freezes everything)
//!          f = form ∈ [0,1],  void ∈ [0,1] (the Void control)
//! helpers: S(a) = smoothstep(0, 1, clamp(a, 0, 1))
//!          noise = 4D simplex (σ ≈ 0.2); each use has its own offset, so fields are independent
//!
//! tides (see `crate::tides`): the renderer has already moved heat, flow, form and void
//! around the sliders. Inside Flux, on the same tide clock T:
//!          a     = 2^(0.485·tide(T, 43 s))        stretch along z, up to ±40%
//!          spread × (1 + 0.3·tide(T, 31 s))       colors at once, ±30%
//!          drift = 0.5·tide(T, 59 s)              dominant colors swing by ± half a color
//!
//! vertical motion:
//!          z′ = z/a − v·t − D·noise(t / P)        steady rise plus a slow sway that now
//!                                                 and then reverses the direction; the
//!                                                 stretch a applies to z only, never to t
//! fields (fixed frequencies k = (kx, ky, kz), so moving form never zooms the pattern):
//!          Fᵢ = noise(kx·x, ky·y, kz·z′, τ·t)
//!                     k                     form    colors
//!          layers   = (0,    0,    0.35)    0       0.6
//!          columns  = (0.45, 0.45, 0.06)    0.25    0.6
//!          blobs    = (0.12, 0.12, 0.12)    0.5     0.6
//!          patches  = (0.2,  0.2,  0.2)     0.75    0.6
//!          grain    = (1.3,  1.3,  1.3)     1       5
//!          Frequencies climb step by step from blobs to grain: mixing fields far apart
//!          in frequency reads as speckle on top of blobs, i.e. already random. Steps are
//!          spaced by how they look, not evenly: at 5 voxels per module side, features
//!          get close to random once they are only ~2 voxels across (k ≈ 0.4).
//! form:    s = 4f,  i = min(⌊s⌋, 3),  w = S(s − i)
//!          n =((1 − w)·Fᵢ + w·Fᵢ₊₁) / √((1 − w)² + w²) / σ     unit spread at every form
//! brightness (by rank, so void is the fraction of dark voxels and the lit ones look
//! the same at any void):
//!          b = Φ(n) ≈ 1 / (1 + e^(−1.7·n))         uniform in [0,1]
//!          intensity = smoothstep(void, void + fade·(1 − void), b)
//!                                                  fade = share of lit voxels fading in
//!
//! color:   n_c = the same blend with its own offset, frequencies × 0.6, slower evolution
//!          spread = (1 − w)·colorsᵢ + w·colorsᵢ₊₁  palette colors per unit of n_c
//!          q = ρ·t + drift + spread·n_c            ρ slowly rotates through the palette
//!          C(q) = Oklab mix of C[⌊q⌋] and C[⌊q⌋ + 1] by S(q − ⌊q⌋)   (indices wrap)
//! heat:    saturation 0 at heat 0, palette as published at 0.5, up to 2× at 1
//!
//! voxel = heat(C(q)) · intensity
//! ```
//!
//! Invariant: nothing may change the coefficient of t (v, τ, ρ are constants). Since t
//! grows without bound, changing it would make the pattern jump. Variation over time
//! goes through bounded terms instead, like the sway and the tides.

use glam::Vec3;
use noise::{NoiseFn, Simplex};
use palette::convert::IntoColorUnclamped;
use palette::{IntoColor, Mix, Oklab, Srgb};

use crate::content::palettes::PALETTES;
use crate::content::{Content, advance};
use crate::renderer::RenderState;
use crate::tides::Tide;
use crate::voxel_image::VoxelImage;

/// A noise field on the form axis.
struct Field {
    frequency: [f64; 3], // noise units per voxel along x, y, z
    colors: f32,         // palette colors per unit of the color noise
}

// Fields in form order, evenly spaced from form 0 to 1.
const FIELDS: [Field; 5] = [
    Field {
        frequency: [0.0, 0.0, 0.35], // layers
        colors: 0.6,
    },
    Field {
        frequency: [0.45, 0.45, 0.06], // columns
        colors: 0.6,
    },
    Field {
        frequency: [0.12, 0.12, 0.12], // blobs
        colors: 0.6,
    },
    Field {
        frequency: [0.2, 0.2, 0.2], // patches
        colors: 0.6,
    },
    Field {
        frequency: [1.3, 1.3, 1.3], // grain
        colors: 5.0,
    },
];
const NOISE_STD: f32 = 0.2; // σ of 4D simplex, measured
const EVOLUTION: f64 = 0.06; // τ, noise time per phase unit

// Vertical motion.
const RISE: f64 = 0.02; // v, voxels per phase unit
const SWAY: f64 = 8.0; // D, voxels
const SWAY_PERIOD: f64 = 230.0; // P, phase units (≈23 s at full Flow)

// Color.
const COLOR_SCALE: f64 = 0.6; // color regions are broader than brightness features
const COLOR_EVOLUTION: f64 = 0.03;
const COLOR_ROTATION: f64 = 1.0 / 150.0; // ρ, palette colors per phase unit (≈15 s at full Flow)
const HEAT_NATIVE: f32 = 0.5; // heat at which the palette shows as published
const HEAT_OVERDRIVE: f32 = 1.0; // extra saturation at heat 1 (1.0 = 2×)
const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);

// Void.
const VOID_FADE: f32 = 0.25; // share of lit voxels fading in from dark

// Tides inside Flux, on the renderer's tide clock (see `crate::tides`).
const STRETCH: Tide = Tide::new(43.0, 0.485, [0.6, 2.8]); // along z: 2^offset, up to ±40%
const SPREAD: Tide = Tide::new(31.0, 0.3, [3.7, 1.2]); // colors at once: ×(1 + offset)
const DRIFT: Tide = Tide::new(59.0, 0.5, [5.0, 0.4]); // dominant colors: ± half a color

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// S(a) in the pseudocode.
fn ease(a: f32) -> f32 {
    smoothstep(0.0, 1.0, a)
}

/// The two neighbouring fields to blend at `form` and their weights.
fn form_weights(form: f32) -> [(usize, f32); 2] {
    let segment = form.clamp(0.0, 1.0) * (FIELDS.len() - 1) as f32;
    let index = (segment.floor() as usize).min(FIELDS.len() - 2);
    let w = ease(segment - index as f32);
    [(index, 1.0 - w), (index + 1, w)]
}

/// Φ(n): rank of a unit-spread noise value, uniform in [0, 1] (logistic approximation).
fn rank(n: f32) -> f32 {
    1.0 / (1.0 + (-1.7 * n).exp())
}

/// Heat: gray at 0, the palette as published at 0.5, oversaturated above.
fn apply_heat(color: Vec3, heat: f32) -> Vec3 {
    let gray = Vec3::splat(color.dot(LUMA));
    let chroma = color - gray;
    let saturation = if heat <= HEAT_NATIVE {
        heat / HEAT_NATIVE
    } else {
        1.0 + HEAT_OVERDRIVE * (heat - HEAT_NATIVE) / (1.0 - HEAT_NATIVE)
    };
    // Largest factor that keeps every channel in [0, 1] along gray → color, so hue is preserved.
    let limit = (0..3).fold(f32::INFINITY, |limit, i| {
        let room = if chroma[i] > 0.0 {
            (1.0 - gray[i]) / chroma[i]
        } else if chroma[i] < 0.0 {
            gray[i] / -chroma[i]
        } else {
            f32::INFINITY
        };
        limit.min(room)
    });
    // The clamp only absorbs float rounding at the cap.
    (gray + chroma * saturation.min(limit)).clamp(Vec3::ZERO, Vec3::ONE)
}

pub struct Flux {
    phase: f64,
    noise: Simplex,
    palette_index: Option<usize>,
    palette: Vec<Oklab>,
}

impl Flux {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            noise: Simplex::new(0x1337),
            palette_index: None,
            palette: Vec::new(),
        }
    }

    /// Converts the selected palette to Oklab whenever the selection changes.
    fn select_palette(&mut self, index: usize) {
        let index = index.min(PALETTES.len() - 1);
        if self.palette_index != Some(index) {
            self.palette_index = Some(index);
            let palette = &PALETTES[index];
            self.palette = palette
                .colors
                .iter()
                .map(|color| {
                    let rgb = color.color();
                    Srgb::new(rgb.x, rgb.y, rgb.z).into_color()
                })
                .collect();
            let names: Vec<_> = palette.colors.iter().map(|color| color.name).collect();
            log::info!("Flux palette: {} ({})", palette.name, names.join(", "));
        }
    }

    /// The two fields picked by form, blended and normalized to unit spread.
    fn field(
        &self,
        position: [f64; 3],
        time: f64,
        weights: &[(usize, f32); 2],
        scale: f64,
        seed: f64,
    ) -> f32 {
        let mut sum = 0.0;
        let mut norm = 0.0;
        for &(index, weight) in weights {
            if weight <= 0.0 {
                continue;
            }
            let k = FIELDS[index].frequency;
            let offset = seed + 31.0 * index as f64;
            let n = self.noise.get([
                k[0] * scale * position[0] + offset,
                k[1] * scale * position[1] + 2.0 * offset,
                k[2] * scale * position[2] + 3.0 * offset,
                time + 5.0 * offset,
            ]) as f32;
            sum += weight * n;
            norm += weight * weight;
        }
        sum / norm.sqrt() / NOISE_STD
    }

    /// C(q): Oklab mix between neighbouring palette colors, indices wrapping.
    fn palette_color(&self, q: f32) -> Vec3 {
        let len = self.palette.len() as i64;
        let index = q.floor();
        let a = self.palette[(index as i64).rem_euclid(len) as usize];
        let b = self.palette[(index as i64 + 1).rem_euclid(len) as usize];
        let rgb: Srgb = a.mix(b, ease(q - index)).into_color_unclamped();
        Vec3::new(rgb.red, rgb.green, rgb.blue).clamp(Vec3::ZERO, Vec3::ONE)
    }
}

impl Content for Flux {
    fn name(&self) -> &str {
        "Flux"
    }

    fn render(
        &mut self,
        state: &RenderState,
        _elapsed: f32,
        delta: f32,
        _prev: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        advance(&mut self.phase, state, delta);
        // Time terms stay in f64 so long installation runs keep their precision.
        let t = self.phase;
        let form = state.form();
        let heat = state.heat();
        let void = state.void();
        self.select_palette(state.palette());

        let weights = form_weights(form);
        let spread: f32 = weights
            .iter()
            .map(|&(index, weight)| weight * FIELDS[index].colors)
            .sum();
        let fade = (VOID_FADE * (1.0 - void)).max(1e-6);

        let lift = RISE * t + SWAY * self.noise.get([t / SWAY_PERIOD, 300.0]);
        let palette_len = self.palette.len() as f64;
        let rotation = (COLOR_ROTATION * t).rem_euclid(palette_len) as f32;

        let tide_time = state.tide_seconds();
        let stretch = 2f64.powf(STRETCH.offset(tide_time) as f64);
        let spread = spread * (1.0 + SPREAD.offset(tide_time));
        let rotation = rotation + DRIFT.offset(tide_time);

        let dim = next.dim();
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    // Stretch only the voxel's own height, never the unbounded lift.
                    let position = [x as f64, y as f64, z as f64 / stretch - lift];
                    let n = self.field(position, EVOLUTION * t, &weights, 1.0, 0.0);
                    let intensity = smoothstep(void, void + fade, rank(n));
                    if intensity <= 0.0 {
                        next.set(x, y, z, Vec3::ZERO);
                        continue;
                    }
                    let n_color =
                        self.field(position, COLOR_EVOLUTION * t, &weights, COLOR_SCALE, 500.0);
                    let color = self.palette_color(rotation + spread * n_color);
                    next.set(x, y, z, apply_heat(color, heat) * intensity);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug-checking only (live evaluation is on the display). Renders a form sweep
    /// for every palette to a PPM contact sheet.
    #[test]
    #[ignore = "writes a visual contact sheet for local bug-checking"]
    fn flux_form_sweep_dump() {
        let forms: Vec<f32> = (0..=8).map(|step| step as f32 / 8.0).collect();
        let dim = (5usize, 5usize, 10usize);
        let cell = 16usize; // pixels per voxel
        let gap = 8usize;
        let width = forms.len() * (dim.0 * cell + gap) + gap;
        let height = PALETTES.len() * (dim.2 * cell + gap) + gap;
        let mut bitmap = vec![12u8; width * height * 3];
        let previous = VoxelImage::new(dim);
        for row_block in 0..PALETTES.len() {
            for (col_block, form) in forms.iter().enumerate() {
                let mut settings = crate::app_state::AppState::default();
                settings.set_palette(PALETTES[row_block].name);
                settings.set_heat(0.5);
                settings.set_form(*form);
                settings.set_flow(1.0);
                let state = RenderState::from(&settings);
                let mut effect = Flux::new();
                let mut image = previous.clone();
                // advance a few frames so time-based structure is visible
                for frame in 0..30 {
                    effect.render(&state, frame as f32 * 0.04, 0.04, &previous, &mut image);
                }
                // project the y=middle slice: x across, z up
                let y = dim.1 / 2;
                for x in 0..dim.0 {
                    for z in 0..dim.2 {
                        let rgb = image.get(x, y, z).clamp(Vec3::ZERO, Vec3::ONE) * 255.0;
                        let px0 = gap + col_block * (dim.0 * cell + gap) + x * cell;
                        let py0 = gap + row_block * (dim.2 * cell + gap) + (dim.2 - 1 - z) * cell;
                        for dy in 0..cell {
                            for dx in 0..cell {
                                let idx = ((py0 + dy) * width + (px0 + dx)) * 3;
                                bitmap[idx] = rgb.x as u8;
                                bitmap[idx + 1] = rgb.y as u8;
                                bitmap[idx + 2] = rgb.z as u8;
                            }
                        }
                    }
                }
            }
        }
        let path = std::env::temp_dir().join("nova-flux-sweep.ppm");
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        bytes.extend(bitmap);
        std::fs::write(&path, bytes).unwrap();
        println!(
            "Flux sweep: {} (columns: form 0 .. 1, rows: palettes)",
            path.display()
        );
    }
}
