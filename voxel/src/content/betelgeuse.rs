//! Betelgeuse: a body of water filling the volume from the floor, with rain and bubbles.
//!
//! Ported from the Java modules Waves, Waves3D, Sweep, Pulse, Jump and Snow. The water
//! was a single blue, so Heat picks the water color from the palette; the deep water below
//! a moving thermocline blends towards the color before it, raindrops towards the color
//! after it. The thermocline carries the same waves as the surface, so the water moves
//! and shows its Form even when it fills the whole volume.
//!
//! ```text
//! inputs:  (x, y, z) voxel, f = form, T = simulated seconds (Flow-integrated), Z = height
//! level:   L = (1 − void)·Z, so the void fraction above the water stays dark on average
//! swell:   h(x, y, t) = Σ anchor weight · componentₖ(x, y, t), components of similar strength
//!          bob   (0)  the whole surface rises and falls (Sweep, Pulse): layers
//!          jump  (¼)  each module's rim against its core, in counter-phase (Jump): columns
//!          roll  (½)  four travelling sines along a fixed diagonal (Waves): blobs
//!          cross (¾)  standing waves across x and y (Waves3D): patches
//!          chop  (1)  short, fast interference, 2 to 3 voxels long: grain
//! surface: s(x, y) = L + A·(h(x, y, T) + rain·ripples)
//!          A = min(1.6, L/2, (Z − L)/2) tapers the swell, so Void 0 and 1 stay exact
//! thermocline: c(x, y) = L/2 − B·h(x, y, 0.8·T),  B = min(1.5, 0.3·L)
//!          internal waves, slower and in counter-phase to the surface
//! rain:    weight r = S(2f − 1)·min(1, 4·(1 − void)); drops (Snow) fall from the top at 5 voxels/s,
//!          each with a short tail; where one meets the surface, a ripple ring spreads at
//!          2.5 voxels/s and fades over 0.9 s
//! bubbles: weight S(2f − 1), 3 per second and module footprint rise from the floor at 2.5
//!          voxels/s and vanish a voxel below the surface; the drop color halfway to white
//! water:   w = S(s − z)    voxels below the surface are lit, the surface voxel partly
//! color:   d = S((c − z)/1.5 + ½)   deep water below the thermocline
//!          q = heat·(len − 1) − d,  brightness 1 − 0.35·d
//!          drops: q = heat·(len − 1) + 0.6     (both clamped to the palette)
//! ```

use std::f32::consts::FRAC_1_SQRT_2;

use glam::Vec3;

use crate::content::common::{PaletteMix, Rng, anchor_weight, ease, footprint};
use crate::content::{Content, advance_seconds};
use crate::host::{MODULE_X_RES, MODULE_Y_RES};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const ANCHORS: usize = 5; // bob, jump, roll, cross, chop
const BOB: (f32, f32) = (1.5, 0.9); // amplitude (voxels), radians per second
const JUMP: (f32, f32) = (1.8, 1.7);
const ROLL: [(f32, f32, f32); 4] = [
    // amplitude, wavenumber (radians per voxel), radians per second
    (0.9, 0.35, 0.8),
    (0.6, 0.6, -1.1),
    (0.45, 0.9, 1.5),
    (0.3, 1.25, -0.7),
];
const CROSS: [(f32, f32, f32); 3] = [(1.2, 0.5, 0.9), (0.8, 0.9, -1.3), (0.55, 1.4, 1.7)];
const CHOP: [(f32, f32, f32, f32); 2] = [
    // amplitude, wavenumber x, wavenumber y, radians per second
    (1.4, 2.4, 2.0, 2.6),
    (0.9, 1.7, -2.3, 3.4),
];
const SWELL: f32 = 1.6; // largest surface amplitude A (voxels)
const THERMOCLINE: f32 = 0.5; // height of the thermocline, as a share of the water level
const INTERNAL: (f32, f32, f32) = (1.5, 0.3, 0.8); // amplitude (voxels), cap as a share of the level, pace
const THERMOCLINE_WIDTH: f32 = 1.5; // voxels
const DEEP_DIM: f32 = 0.35; // deep water is this much darker
const RAIN_FORM: f32 = 0.5; // rain starts above this form
const RAIN_RATE: f32 = 2.5; // drops per second and module footprint at full rain
const MAX_DROPS: usize = 64;
const DROP_SPEED: f32 = 5.0; // voxels per simulated second
const DROP_TAIL: f32 = 0.4; // brightness of the voxel above a drop
const BUBBLE_RATE: f32 = 3.0; // bubbles per second and module footprint at full rain
const BUBBLE_SPEED: f32 = 2.5; // voxels per simulated second
const BUBBLE_WHITE: f32 = 0.5; // bubbles mix the drop color this much towards white
const RIPPLE: (f32, f32, f32) = (0.9, 2.5, 0.9); // amplitude, speed (voxels/s), life (s)
const RIPPLE_WAVENUMBER: f32 = 2.2;
const DEPTH: f32 = 1.0; // palette colors the deep water drifts towards the previous color
const DROP_SHIFT: f32 = 0.6; // palette colors drops drift towards the next color
const SUBSTEP: f32 = 0.05;
const SEED: u64 = 0x5EA;

struct Drop {
    x: usize,
    y: usize,
    z: f32,
}

struct Ripple {
    x: f32,
    y: f32,
    age: f32,
}

pub struct Betelgeuse {
    phase: f64,
    time: f64,
    rng: Rng,
    dim: (usize, usize, usize),
    drops: Vec<Drop>,
    bubbles: Vec<Drop>,
    ripples: Vec<Ripple>,
    spawn: f32,        // drops owed
    bubble_spawn: f32, // bubbles owed
    surface: Vec<f32>,
    thermocline: Vec<f32>,
    palette: PaletteMix,
}

impl Betelgeuse {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            time: 0.0,
            rng: Rng::new(SEED),
            dim: (0, 0, 0),
            drops: Vec::new(),
            bubbles: Vec::new(),
            ripples: Vec::new(),
            spawn: 0.0,
            bubble_spawn: 0.0,
            surface: Vec::new(),
            thermocline: Vec::new(),
            palette: PaletteMix::default(),
        }
    }

    fn reset(&mut self, dim: (usize, usize, usize)) {
        self.dim = dim;
        self.time = 0.0;
        self.rng = Rng::new(SEED);
        self.drops.clear();
        self.bubbles.clear();
        self.ripples.clear();
        self.spawn = 0.0;
        self.bubble_spawn = 0.0;
        self.surface = vec![0.0; dim.0 * dim.1];
        self.thermocline = vec![0.0; dim.0 * dim.1];
    }

    /// The swell at a column and time `t`, before scaling by its amplitude.
    fn swell(&self, x: usize, y: usize, t: f32, weights: &[f32; ANCHORS]) -> f32 {
        let (xf, yf) = (x as f32, y as f32);
        let mut height = 0.0;
        if weights[0] > 0.0 {
            height += weights[0] * BOB.0 * (BOB.1 * t).sin();
        }
        if weights[1] > 0.0 {
            let (mx, my) = (x % MODULE_X_RES, y % MODULE_Y_RES);
            let rim = mx == 0 || my == 0 || mx == MODULE_X_RES - 1 || my == MODULE_Y_RES - 1;
            let sign = if rim { 1.0 } else { -1.0 };
            height += weights[1] * sign * JUMP.0 * (JUMP.1 * t).sin();
        }
        if weights[2] > 0.0 {
            let u = (xf + yf) * FRAC_1_SQRT_2;
            let roll: f32 = ROLL
                .iter()
                .map(|&(a, k, w)| a * (k * u + w * t).sin())
                .sum();
            height += weights[2] * roll;
        }
        if weights[3] > 0.0 {
            let (cx, cy) = (0.5 * (self.dim.0 - 1) as f32, 0.5 * (self.dim.1 - 1) as f32);
            let cross: f32 = CROSS
                .iter()
                .map(|&(a, k, w)| {
                    a * ((k * (xf - cx)).cos() * (w * t).sin()
                        + (k * (yf - cy)).cos() * (w * t + 1.3).cos())
                })
                .sum();
            height += weights[3] * cross;
        }
        if weights[4] > 0.0 {
            let chop: f32 = CHOP
                .iter()
                .map(|&(a, kx, ky, w)| a * (kx * xf + w * t).sin() * (ky * yf - 0.8 * w * t).sin())
                .sum();
            height += weights[4] * chop;
        }
        height
    }

    fn ripples_at(&self, x: usize, y: usize) -> f32 {
        let (amplitude, speed, life) = RIPPLE;
        self.ripples
            .iter()
            .map(|ripple| {
                let r = ((x as f32 - ripple.x).powi(2) + (y as f32 - ripple.y).powi(2)).sqrt();
                let front = r - speed * ripple.age;
                amplitude
                    * (-ripple.age / life).exp()
                    * (RIPPLE_WAVENUMBER * front).cos()
                    * (-front * front / 1.5).exp()
            })
            .sum()
    }

    fn step(&mut self, dt: f32, rain: f32, bubbles: f32) {
        let steps = (dt / SUBSTEP).ceil() as usize;
        let top = self.dim.2 as f32;
        for _ in 0..steps {
            let h = dt / steps as f32;
            for ripple in self.ripples.iter_mut() {
                ripple.age += h;
            }
            self.ripples.retain(|ripple| ripple.age < 4.0 * RIPPLE.2);
            self.spawn += rain * RAIN_RATE * footprint(self.dim) * h;
            while self.spawn >= 1.0 {
                self.spawn -= 1.0;
                if self.drops.len() < MAX_DROPS {
                    let x = self.rng.below(self.dim.0);
                    let y = self.rng.below(self.dim.1);
                    self.drops.push(Drop { x, y, z: top });
                }
            }
            for drop in self.drops.iter_mut() {
                drop.z -= DROP_SPEED * h;
            }
            let surface = &self.surface;
            let dy = self.dim.1;
            let mut landed = Vec::new();
            self.drops.retain(|drop| {
                let hit = drop.z <= surface[drop.x * dy + drop.y].max(0.0);
                if hit {
                    landed.push((drop.x as f32, drop.y as f32));
                }
                !hit
            });
            for (x, y) in landed {
                self.ripples.push(Ripple { x, y, age: 0.0 });
            }

            self.bubble_spawn += bubbles * BUBBLE_RATE * footprint(self.dim) * h;
            while self.bubble_spawn >= 1.0 {
                self.bubble_spawn -= 1.0;
                if self.bubbles.len() < MAX_DROPS {
                    let x = self.rng.below(self.dim.0);
                    let y = self.rng.below(self.dim.1);
                    self.bubbles.push(Drop { x, y, z: 0.0 });
                }
            }
            for bubble in self.bubbles.iter_mut() {
                bubble.z += BUBBLE_SPEED * h;
            }
            self.bubbles
                .retain(|bubble| bubble.z < surface[bubble.x * dy + bubble.y] - 1.0);
        }
        self.time += dt as f64;
    }
}

impl Content for Betelgeuse {
    fn name(&self) -> &str {
        "Betelgeuse"
    }

    fn render(&mut self, state: &RenderState, delta: f32, next: &mut VoxelImage) {
        let dt = advance_seconds(&mut self.phase, state, delta);
        let dim = next.dim();
        if state.should_reset() || self.dim != dim {
            self.reset(dim);
        }
        self.palette.select(state.palette(), "Betelgeuse");
        let form = state.form();
        let void = state.void();
        let bubbles = ease((form - RAIN_FORM) / (1.0 - RAIN_FORM));
        let rain = bubbles * (4.0 * (1.0 - void)).min(1.0);
        self.step(dt, rain, bubbles);

        let top = dim.2 as f32;
        let level = (1.0 - void) * top;
        let amplitude = SWELL.min(0.5 * level).min(0.5 * (top - level));
        let (internal, cap, pace) = INTERNAL;
        let internal = internal.min(cap * level);
        let t = self.time as f32;
        let weights: [f32; ANCHORS] = std::array::from_fn(|i| anchor_weight(form, ANCHORS, i));
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                let swell = self.swell(x, y, t, &weights) + rain * self.ripples_at(x, y);
                self.surface[x * dim.1 + y] = level + amplitude * swell;
                self.thermocline[x * dim.1 + y] =
                    THERMOCLINE * level - internal * self.swell(x, y, pace * t, &weights);
            }
        }

        let hot = self.palette.heat_position(state.heat());
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                let surface = self.surface[x * dim.1 + y];
                let thermocline = self.thermocline[x * dim.1 + y];
                for z in 0..dim.2 {
                    let water = ease(surface - z as f32);
                    if water <= 0.0 {
                        next.set(x, y, z, Vec3::ZERO);
                        continue;
                    }
                    let deep = ease((thermocline - z as f32) / THERMOCLINE_WIDTH + 0.5);
                    let color = self.palette.clamped(hot - DEPTH * deep) * (1.0 - DEEP_DIM * deep);
                    next.set(x, y, z, color * water);
                }
            }
        }

        let drop_color = self.palette.clamped(hot + DROP_SHIFT);
        let bubble_color = drop_color.lerp(Vec3::ONE, BUBBLE_WHITE);
        for bubble in &self.bubbles {
            let z = bubble.z.round() as usize;
            if z < dim.2 {
                let water = next.get(bubble.x, bubble.y, z);
                next.set(bubble.x, bubble.y, z, water.lerp(bubble_color, bubbles));
            }
        }
        for drop in &self.drops {
            let z = drop.z.round();
            for (offset, brightness) in [(0.0, 1.0), (1.0, DROP_TAIL)] {
                let zz = z + offset;
                if zz >= 0.0 && zz < top {
                    let zz = zz as usize;
                    let b = brightness * rain;
                    let color = next.get(drop.x, drop.y, zz).max(drop_color * b);
                    next.set(drop.x, drop.y, zz, color);
                }
            }
        }
    }
}
