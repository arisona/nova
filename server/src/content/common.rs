//! Shared building blocks for content modules: easing, palette mixing, Heat, Void and a
//! small deterministic random generator.

use glam::Vec3;
use palette::convert::IntoColorUnclamped;
use palette::{IntoColor, Mix, Oklab, Srgb};

use crate::palettes::PALETTES;

// Share of the way between two palette colors spent blending them; the rest shows the
// colors as published. 1 blends all the way, smaller values keep blends short, so
// neighbours that mix through gray or pastel show less of it.
const BLEND_WIDTH: f32 = 0.5;
const HEAT_NATIVE: f32 = 0.5; // heat at which the palette shows as published
const HEAT_OVERDRIVE: f32 = 1.0; // extra saturation at heat 1 (1.0 = 2×)
const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// S(a): smoothstep(0, 1, clamp(a, 0, 1)).
pub fn ease(a: f32) -> f32 {
    smoothstep(0.0, 1.0, a)
}

/// The two neighbouring anchors to blend at `form`, for `count` anchors evenly spaced
/// from form 0 to 1, and their weights.
pub fn form_weights(form: f32, count: usize) -> [(usize, f32); 2] {
    let segment = form.clamp(0.0, 1.0) * (count - 1) as f32;
    let index = (segment.floor() as usize).min(count - 2);
    let w = ease(segment - index as f32);
    [(index, 1.0 - w), (index + 1, w)]
}

/// Weight of the anchor `index` among `count` at `form`: 1 at its own form, fading to 0
/// at its neighbours. The weights of all anchors sum to 1.
pub fn anchor_weight(form: f32, count: usize, index: usize) -> f32 {
    form_weights(form, count)
        .iter()
        .filter(|&&(i, _)| i == index)
        .map(|&(_, w)| w)
        .sum()
}

/// Heat: gray at 0, the palette as published at 0.5, oversaturated above.
pub fn apply_heat(color: Vec3, heat: f32) -> Vec3 {
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

/// The selected palette in Oklab, converted whenever the selection changes.
#[derive(Default)]
pub struct PaletteMix {
    index: Option<usize>,
    colors: Vec<Oklab>,
}

impl PaletteMix {
    pub fn select(&mut self, index: usize, module: &str) {
        let index = index.min(PALETTES.len() - 1);
        if self.index != Some(index) {
            self.index = Some(index);
            let palette = &PALETTES[index];
            self.colors = palette
                .colors
                .iter()
                .map(|color| {
                    let rgb = color.color();
                    Srgb::new(rgb.x, rgb.y, rgb.z).into_color()
                })
                .collect();
            let names: Vec<_> = palette.colors.iter().map(|color| color.name).collect();
            log::info!("{module} palette: {} ({})", palette.name, names.join(", "));
        }
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// The Oklab mix at palette position `q`, indices wrapping. The blend takes the middle
    /// `BLEND_WIDTH` of the way between two colors.
    pub fn oklab(&self, q: f32) -> Oklab {
        self.mix(q, BLEND_WIDTH)
    }

    /// C(q): the mix at palette position `q` in sRGB, indices wrapping.
    pub fn wrapped(&self, q: f32) -> Vec3 {
        to_rgb(self.oklab(q))
    }

    /// The mix at palette position `q`, clamped to the first and last color instead of
    /// wrapping. Blends all the way, since modules use it as a gradient.
    pub fn clamped(&self, q: f32) -> Vec3 {
        to_rgb(self.mix(q.clamp(0.0, (self.colors.len() - 1) as f32), 1.0))
    }

    /// The Oklab mix at palette position `q`, blending over the middle `width` of the way
    /// between two colors, indices wrapping.
    fn mix(&self, q: f32, width: f32) -> Oklab {
        let len = self.colors.len() as i64;
        let index = q.floor();
        let a = self.colors[(index as i64).rem_euclid(len) as usize];
        let b = self.colors[(index as i64 + 1).rem_euclid(len) as usize];
        let w = smoothstep(0.5 - 0.5 * width, 0.5 + 0.5 * width, q - index);
        a.mix(b, w)
    }

    /// Heat as a picker, for modules built around a single color: the blend that slides
    /// from the palette's first color (Heat 0) to its last (Heat 1). Returns the palette
    /// position, to be passed to `clamped` with optional offsets.
    pub fn heat_position(&self, heat: f32) -> f32 {
        heat.clamp(0.0, 1.0) * (self.colors.len() - 1) as f32
    }
}

pub fn to_rgb(color: Oklab) -> Vec3 {
    let rgb: Srgb = color.into_color_unclamped();
    Vec3::new(rgb.red, rgb.green, rgb.blue).clamp(Vec3::ZERO, Vec3::ONE)
}

/// Void by rank, for modules whose every voxel carries a value: replaces each value by
/// its intensity, so exactly the `void` fraction with the lowest values goes dark and
/// the lit ones look the same at any void. `fade` is the share of lit voxels fading in.
/// Ties are broken by position, so equal values never flicker.
pub fn void_by_rank(values: &mut [f32], order: &mut Vec<u32>, void: f32, fade: f32) {
    let n = values.len();
    order.clear();
    order.extend(0..n as u32);
    order.sort_unstable_by(|&a, &b| {
        values[a as usize]
            .total_cmp(&values[b as usize])
            .then(a.cmp(&b))
    });
    let fade = (fade * (1.0 - void)).max(1e-6);
    for (position, &index) in order.iter().enumerate() {
        let rank = (position as f32 + 0.5) / n as f32;
        values[index as usize] = smoothstep(void, void + fade, rank);
    }
}

/// Void as a cap, for sparse modules whose dark voxels are already part of the picture:
/// keeps the brightest `1 − void` fraction of voxels at their own intensity and darkens
/// the rest, so at least the `void` fraction is dark.
pub fn void_as_cap(intensities: &mut [f32], scratch: &mut Vec<f32>, void: f32) {
    let n = intensities.len();
    let k = ((void.clamp(0.0, 1.0) * n as f32) as usize).min(n);
    if k == 0 {
        return;
    }
    if k == n {
        intensities.fill(0.0);
        return;
    }
    scratch.clear();
    scratch.extend_from_slice(intensities);
    let (_, &mut threshold, _) = scratch.select_nth_unstable_by(k - 1, f32::total_cmp);
    let soft = 0.15 * threshold + 1e-4;
    for value in intensities.iter_mut() {
        *value *= smoothstep(threshold, threshold + soft, *value);
    }
}

/// SplitMix64: small, fast and deterministic, so a reset replays the same animation.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// A random direction.
    pub fn direction(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            );
            let length = v.length();
            if length > 0.1 && length <= 1.0 {
                return v / length;
            }
        }
    }

    /// A random position inside a volume of `dim` voxels.
    pub fn position(&mut self, dim: (usize, usize, usize)) -> Vec3 {
        Vec3::new(
            self.range(0.0, (dim.0 - 1) as f32),
            self.range(0.0, (dim.1 - 1) as f32),
            self.range(0.0, (dim.2 - 1) as f32),
        )
    }
}

/// Flat index of a voxel, matching the order of `VoxelImage`.
pub fn voxel_index(dim: (usize, usize, usize), x: usize, y: usize, z: usize) -> usize {
    (x * dim.1 + y) * dim.2 + z
}

/// How many 5 × 5 module footprints the volume covers, for scaling particle counts so
/// more modules show more of the same, not a sparser version.
pub fn footprint(dim: (usize, usize, usize)) -> f32 {
    (dim.0 * dim.1) as f32 / 25.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn void_helpers_darken_the_requested_fraction() {
        let mut rng = Rng::new(7);
        let values: Vec<f32> = (0..250).map(|_| rng.unit()).collect();
        for void in [0.0, 0.3, 0.5, 0.9, 1.0] {
            let mut ranked = values.clone();
            void_by_rank(&mut ranked, &mut Vec::new(), void, 0.25);
            let dark = ranked.iter().filter(|&&v| v == 0.0).count() as f32 / 250.0;
            assert!((dark - void).abs() <= 0.01, "rank: {dark} at void {void}");

            let mut capped = values.clone();
            void_as_cap(&mut capped, &mut Vec::new(), void);
            let lit = capped.iter().filter(|&&v| v > 0.0).count() as f32 / 250.0;
            assert!(lit <= 1.0 - void + 0.01, "cap: {lit} lit at void {void}");
            // bright voxels keep their intensity (the cut fades in just above the threshold)
            if void <= 0.5 {
                let max = values.iter().cloned().fold(0.0, f32::max);
                assert!(capped.contains(&max));
            }
        }
    }

    #[test]
    fn anchor_weights_sum_to_one() {
        for step in 0..=20 {
            let form = step as f32 / 20.0;
            let sum: f32 = (0..5).map(|i| anchor_weight(form, 5, i)).sum();
            assert!((sum - 1.0).abs() < 1e-5);
        }
        assert_eq!(anchor_weight(0.0, 5, 0), 1.0);
        assert_eq!(anchor_weight(1.0, 5, 4), 1.0);
    }
}
