//! Spice Melange: soft spheres drifting and bouncing through the volume.
//!
//! Ported from the Java modules BouncingMetaBalls, BouncingMetaBallsT, ColorSplash, Pong
//! and Pong2. Their fields add up like metaballs, so nearby orbs melt into each other;
//! each breathes like a ColorSplash sphere, and a bounce off the floor or the ceiling
//! lights a short Pong paddle there.
//!
//! ```text
//! inputs:  p = voxel position, f = form, T = simulated seconds (Flow-integrated)
//! orbs:    count  = lerp(3, 12, f) per module footprint; extra orbs fade in and out
//!                   one by one (presence wᵢ ∈ [0, 1]), so moving Form never pops
//!          radius rᵢ = lerp(2.4, 0.9, f) · (1 + 0.35·sin(0.6·T + φᵢ))   breathing
//!          speed  = lerp(0.8, 3, f) voxels/s along a fixed direction, ×2 vertically
//!          walls reflect; floor and ceiling also nudge the horizontal direction (Pong)
//! field:   F(p)   = Σ wᵢ · (rᵢ² / (|p − cᵢ|² + 0.3))^(k/2),  k = lerp(1.5, 4, f)
//!          low Form: few large orbs with soft, merging fields (lava lamp)
//!          high Form: many small, distinct balls (Pong)
//! color:   each orb owns a palette position hᵢ·len + ρ·T (slowly rotating); a voxel
//!          mixes the orbs' colors in Oklab, weighted by the square of their share
//! void:    by rank of F: exactly the void fraction stays dark (see `void_by_rank`)
//! paddles: a 3 × 3 flash on the floor or ceiling where an orb bounced, fading over
//!          0.6 s, weight smoothstep(0.4, 0.8, f)
//! heat:    saturation, as in Flux Capacitor
//! ```

use glam::Vec3;
use palette::Oklab;

use crate::content::common::{
    PaletteMix, Rng, apply_heat, footprint, smoothstep, to_rgb, void_by_rank, voxel_index,
};
use crate::content::{Content, advance_seconds};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const COUNT: [f32; 2] = [3.0, 12.0]; // orbs per module footprint at form 0 and 1
const RADIUS: [f32; 2] = [2.4, 0.9]; // voxels
const SHARPNESS: [f32; 2] = [1.5, 4.0]; // field exponent k
const SPEED: [f32; 2] = [0.8, 3.0]; // voxels per simulated second
const VERTICAL: f32 = 2.0; // the volume is tall: orbs move faster up and down
const BREATH: f32 = 0.35; // radius swell
const BREATH_RATE: f64 = 0.6; // radians per simulated second
const ROTATION: f64 = 0.05; // palette colors per simulated second
const BOUNCE_NUDGE: f32 = 0.15; // horizontal direction change on a floor/ceiling bounce
const PADDLE_LIFE: f32 = 0.6; // seconds
const PADDLE_FORM: [f32; 2] = [0.4, 0.8];
const MAX_PADDLES: usize = 32;
const VOID_FADE: f32 = 0.25;
const SUBSTEP: f32 = 0.05;
const SEED: u64 = 0x0B5;

fn lerp(range: [f32; 2], t: f32) -> f32 {
    range[0] + (range[1] - range[0]) * t
}

struct Orb {
    position: Vec3,
    direction: Vec3,
    hue: f32,    // palette position as a fraction of the palette
    breath: f64, // phase offset
}

struct Paddle {
    x: f32,
    y: f32,
    z: usize,
    hue: f32,
    age: f32,
}

pub struct SpiceMelange {
    phase: f64,
    time: f64,
    rng: Rng,
    dim: (usize, usize, usize),
    orbs: Vec<Orb>,
    paddles: Vec<Paddle>,
    palette: PaletteMix,
    field: Vec<f32>,
    colors: Vec<Vec3>,
    order: Vec<u32>,
}

impl SpiceMelange {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            time: 0.0,
            rng: Rng::new(SEED),
            dim: (0, 0, 0),
            orbs: Vec::new(),
            paddles: Vec::new(),
            palette: PaletteMix::default(),
            field: Vec::new(),
            colors: Vec::new(),
            order: Vec::new(),
        }
    }

    fn reset(&mut self, dim: (usize, usize, usize)) {
        self.dim = dim;
        self.time = 0.0;
        self.rng = Rng::new(SEED);
        self.paddles.clear();
        let pool = (COUNT[1] * footprint(dim)).ceil() as usize;
        self.orbs = (0..pool)
            .map(|i| {
                let mut direction = self.rng.direction();
                direction.z *= VERTICAL;
                Orb {
                    position: self.rng.position(dim),
                    direction,
                    // spread evenly around the palette, in a shuffled order
                    hue: (i as f32 * 0.618_034).fract(),
                    breath: self.rng.range(0.0, std::f32::consts::TAU) as f64,
                }
            })
            .collect();
        let voxels = dim.0 * dim.1 * dim.2;
        self.field = vec![0.0; voxels];
        self.colors = vec![Vec3::ZERO; voxels];
    }

    fn step(&mut self, dt: f32, form: f32) {
        let steps = (dt / SUBSTEP).ceil() as usize;
        let max = Vec3::new(
            (self.dim.0 - 1) as f32,
            (self.dim.1 - 1) as f32,
            (self.dim.2 - 1) as f32,
        );
        let speed = lerp(SPEED, form);
        for _ in 0..steps {
            let h = dt / steps as f32;
            for paddle in self.paddles.iter_mut() {
                paddle.age += h;
            }
            self.paddles.retain(|paddle| paddle.age < PADDLE_LIFE);
            for orb in self.orbs.iter_mut() {
                orb.position += orb.direction * speed * h;
                for axis in 0..3 {
                    let bounced = if orb.position[axis] < 0.0 {
                        orb.position[axis] = -orb.position[axis];
                        orb.direction[axis] = orb.direction[axis].abs();
                        Some(0)
                    } else if orb.position[axis] > max[axis] {
                        orb.position[axis] = 2.0 * max[axis] - orb.position[axis];
                        orb.direction[axis] = -orb.direction[axis].abs();
                        Some(self.dim.2 - 1)
                    } else {
                        None
                    };
                    if let (2, Some(layer)) = (axis, bounced) {
                        orb.direction.x += self.rng.range(-BOUNCE_NUDGE, BOUNCE_NUDGE);
                        orb.direction.y += self.rng.range(-BOUNCE_NUDGE, BOUNCE_NUDGE);
                        if self.paddles.len() < MAX_PADDLES {
                            self.paddles.push(Paddle {
                                x: orb.position.x,
                                y: orb.position.y,
                                z: layer,
                                hue: orb.hue,
                                age: 0.0,
                            });
                        }
                    }
                }
                // Keep the horizontal pace from drifting after many nudges.
                let horizontal = orb.direction.truncate().length();
                if horizontal > 1.0 {
                    orb.direction.x /= horizontal;
                    orb.direction.y /= horizontal;
                }
            }
        }
        self.time += dt as f64;
    }

    fn hue_position(&self, hue: f32) -> f32 {
        let len = self.palette.len() as f64;
        (hue as f64 * len + ROTATION * self.time).rem_euclid(len) as f32
    }
}

impl Content for SpiceMelange {
    fn name(&self) -> &str {
        "Spice Melange"
    }

    fn render(
        &mut self,
        state: &RenderState,
        _elapsed: f32,
        delta: f32,
        _prev: &VoxelImage,
        next: &mut VoxelImage,
    ) {
        let dt = advance_seconds(&mut self.phase, state, delta);
        let dim = next.dim();
        if state.should_reset() || self.dim != dim {
            self.reset(dim);
        }
        self.palette.select(state.palette(), "Spice Melange");
        let form = state.form();
        let heat = state.heat();
        self.step(dt, form);

        let count = lerp(COUNT, form) * footprint(dim);
        let radius = lerp(RADIUS, form);
        let sharpness = lerp(SHARPNESS, form) * 0.5;
        // Per orb: center, presence, radius², Oklab color.
        let orbs: Vec<(Vec3, f32, f32, Vec3)> = self
            .orbs
            .iter()
            .enumerate()
            .filter_map(|(i, orb)| {
                let presence = (count - i as f32).clamp(0.0, 1.0);
                (presence > 0.0).then(|| {
                    let swell = 1.0 + BREATH * (BREATH_RATE * self.time + orb.breath).sin() as f32;
                    let r = radius * swell;
                    let lab = self.palette.oklab(self.hue_position(orb.hue));
                    (
                        orb.position,
                        presence,
                        r * r,
                        Vec3::new(lab.l, lab.a, lab.b),
                    )
                })
            })
            .collect();

        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    let p = Vec3::new(x as f32, y as f32, z as f32);
                    let mut sum = 0.0;
                    let mut lab = Vec3::ZERO;
                    let mut weights = 0.0;
                    for &(center, presence, r2, color) in &orbs {
                        let share =
                            presence * (r2 / (p.distance_squared(center) + 0.3)).powf(sharpness);
                        sum += share;
                        lab += color * share * share;
                        weights += share * share;
                    }
                    let index = voxel_index(dim, x, y, z);
                    self.field[index] = sum;
                    self.colors[index] = lab / weights.max(1e-12);
                }
            }
        }
        void_by_rank(&mut self.field, &mut self.order, state.void(), VOID_FADE);

        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    let index = voxel_index(dim, x, y, z);
                    let intensity = self.field[index];
                    let lab = self.colors[index];
                    let color = apply_heat(to_rgb(Oklab::new(lab.x, lab.y, lab.z)), heat);
                    next.set(x, y, z, color * intensity);
                }
            }
        }

        let paddle_weight = smoothstep(PADDLE_FORM[0], PADDLE_FORM[1], form);
        if paddle_weight > 0.0 {
            for paddle in &self.paddles {
                let b = paddle_weight * (1.0 - paddle.age / PADDLE_LIFE);
                let color = apply_heat(self.palette.wrapped(self.hue_position(paddle.hue)), heat);
                let (px, py) = (paddle.x.round() as i64, paddle.y.round() as i64);
                for x in (px - 1).max(0)..=(px + 1).min(dim.0 as i64 - 1) {
                    for y in (py - 1).max(0)..=(py + 1).min(dim.1 as i64 - 1) {
                        let (x, y) = (x as usize, y as usize);
                        let mixed = next.get(x, y, paddle.z).lerp(color, b);
                        next.set(x, y, paddle.z, mixed);
                    }
                }
            }
        }
    }
}
