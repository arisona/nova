//! Sparkle: while a fade lifts Void, short glints scatter over the dimming display, in the
//! colors and where the light was, so the dark between two modules or palettes glitters
//! rather than goes flat.
//!
//! It follows the features of the light (see `Features`), smoothed over a second and held
//! while the display is dark: glints take a color by the shares of its hues, sit around its
//! balance and height and spread with its width, shine a little brighter than its lit voxels
//! (its glow) and twinkle faster for busier light. Gray light glints white.

use glam::Vec3;
use nova_core::{Features, HUES};
use palette::convert::FromColorUnclamped;
use palette::{Oklch, Srgb};

use crate::content::common::{Rng, footprint, smoothstep};
use crate::voxel_image::VoxelImage;

/// Glints per second and 5 × 5 module footprint while a fade is at its darkest.
const RATE: f32 = 14.0;
/// Seconds a glint takes to light up, and its decay time constant for still and for busy
/// light (see `Features::activity`).
const RISE: f32 = 0.06;
const DECAY: [f32; 2] = [0.45, 0.2];
/// Seconds over which the glints follow the light.
const FOLLOW: f32 = 1.0;
/// How much lighter glints are than the lit voxels, in Oklab lightness.
const LIFT: f32 = 0.15;
/// Chroma of glints in colored light, and how far their hue strays from the light's.
const CHROMA: f32 = 0.09;
const HUE_SPREAD: f32 = 15.0;

struct Glint {
    voxel: (usize, usize, usize),
    color: Vec3,
    age: f32,
    decay: f32,
}

impl Glint {
    fn intensity(&self) -> f32 {
        (self.age / RISE).min(1.0) * (-(self.age - RISE).max(0.0) / self.decay).exp()
    }
}

pub struct Sparkle {
    glints: Vec<Glint>,
    /// The light the glints follow, and whether any has been seen yet.
    light: Features,
    seen: bool,
    /// Glints owed from previous frames, so low rates still spawn evenly.
    due: f32,
    rng: Rng,
}

impl Default for Sparkle {
    fn default() -> Self {
        Self {
            glints: Vec::new(),
            light: Features::default(),
            seen: false,
            due: 0.0,
            rng: Rng::new(0x5BA2_C1E5),
        }
    }
}

impl Sparkle {
    /// Advances by `delta` seconds: follows `light`, the features of the frame, while
    /// something is lit, and spawns glints at `amount`, from 0 (none) to 1 (a fade at its
    /// darkest).
    pub fn update(
        &mut self,
        light: &Features,
        amount: f32,
        delta: f32,
        dim: (usize, usize, usize),
    ) {
        if light.density > 0.0 {
            let follow = if self.seen {
                1.0 - (-delta / FOLLOW).exp()
            } else {
                1.0
            };
            self.light = self.light.towards(light, follow);
            self.seen = true;
        }
        for glint in &mut self.glints {
            glint.age += delta;
        }
        self.glints
            .retain(|glint| glint.age < RISE + 5.0 * glint.decay);

        self.due += RATE * footprint(dim) * amount.clamp(0.0, 1.0) * delta;
        while self.due >= 1.0 {
            self.due -= 1.0;
            let glint = self.spawn(dim);
            self.glints.push(glint);
        }
        if amount <= 0.0 {
            self.due = 0.0;
        }
    }

    /// Lights the glints in `image`, scaled by `brightness`, over what is there.
    pub fn draw(&self, image: &mut VoxelImage, brightness: f32) {
        for glint in &self.glints {
            let (x, y, z) = glint.voxel;
            let color = glint.color * glint.intensity() * brightness;
            image.set(x, y, z, image.get(x, y, z).max(color));
        }
    }

    fn spawn(&mut self, (dx, dy, dz): (usize, usize, usize)) -> Glint {
        let light = self.light;
        // Around the light's balance, spread like it: light spread evenly from left to
        // right has a deviation of 1/√12 of the width.
        let x = (light.balance + 1.0) * 0.5 * dx as f32
            + self.normal() * (light.width * dx as f32 / 12f32.sqrt()).max(0.5);
        let y = self.rng.range(0.0, dy as f32);
        let z = light.height * dz as f32 + self.normal() * 0.2 * dz as f32;
        let voxel = |position: f32, count: usize| (position.max(0.0) as usize).min(count - 1);

        let busy = smoothstep(0.01, 0.1, light.activity);
        Glint {
            voxel: (voxel(x, dx), voxel(y, dy), voxel(z, dz)),
            color: self.color(),
            age: 0.0,
            decay: DECAY[0] + (DECAY[1] - DECAY[0]) * busy,
        }
    }

    /// A color of the light: a hue picked by its share, a little lighter than the lit voxels.
    fn color(&mut self) -> Vec3 {
        let light = &self.light;
        let colored = light.hues.iter().sum::<f32>().min(1.0);
        let mut pick = self.rng.unit() * light.hues.iter().sum::<f32>();
        let mut hue = 0;
        while hue < HUES - 1 && pick >= light.hues[hue] {
            pick -= light.hues[hue];
            hue += 1;
        }
        let hue = hue as f32 * 30.0 + self.rng.range(-HUE_SPREAD, HUE_SPREAD);
        // A little lighter than the lit voxels: glow is luminance, and Oklab lightness
        // about its cube root.
        let lightness = (light.glow().cbrt() + LIFT).clamp(0.7, 0.97);
        // Less chroma until the color fits into sRGB, so the hue stays.
        let mut chroma = CHROMA * colored;
        loop {
            let rgb = Srgb::from_color_unclamped(Oklch::new(lightness, chroma, hue));
            let rgb = Vec3::new(rgb.red, rgb.green, rgb.blue);
            let fits = rgb.min_element() >= -1e-3 && rgb.max_element() <= 1.0 + 1e-3;
            if fits || chroma < 1e-3 {
                return rgb.clamp(Vec3::ZERO, Vec3::ONE);
            }
            chroma *= 0.8;
        }
    }

    /// Roughly normal, with a deviation of 1.
    fn normal(&mut self) -> f32 {
        (self.rng.unit() + self.rng.unit() + self.rng.unit() - 1.5) * 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIM: (usize, usize, usize) = (5, 5, 10);

    fn lit(image: &VoxelImage) -> Vec<Vec3> {
        let (dx, dy, dz) = image.dim();
        (0..dx)
            .flat_map(|x| (0..dy).flat_map(move |y| (0..dz).map(move |z| (x, y, z))))
            .map(|(x, y, z)| image.get(x, y, z))
            .filter(|color| color.max_element() > 0.0)
            .collect()
    }

    /// Glints appear only while there is sparkle, in the colors of the light, and fade out.
    #[test]
    fn glints_in_the_colors_of_the_light() {
        let mut hues = [0.0; HUES];
        // Red light, at the top.
        hues[1] = 1.0;
        let light = Features {
            height: 0.9,
            hues,
            ..Features::NEUTRAL
        };
        let mut sparkle = Sparkle::default();
        let mut image = VoxelImage::new(DIM);
        for _ in 0..50 {
            sparkle.update(&light, 0.0, 0.04, DIM);
        }
        sparkle.draw(&mut image, 1.0);
        assert!(lit(&image).is_empty());

        for _ in 0..50 {
            sparkle.update(&Features::default(), 1.0, 0.04, DIM);
        }
        sparkle.draw(&mut image, 1.0);
        let glints = lit(&image);
        assert!(glints.len() >= 5, "{}", glints.len());
        for color in glints {
            assert!(color.x > color.y && color.x > color.z, "{color}");
        }
        let top = sparkle
            .glints
            .iter()
            .filter(|glint| glint.voxel.2 >= 5)
            .count();
        assert!(top * 4 > sparkle.glints.len() * 3);

        for _ in 0..100 {
            sparkle.update(&light, 0.0, 0.04, DIM);
        }
        assert!(sparkle.glints.is_empty());
    }
}
