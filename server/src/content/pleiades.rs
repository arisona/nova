//! Pleiades: a reaction-diffusion system growing spots, stripes and mazes.
//!
//! Ported from the Java modules ReactionDiffusion and ReactionDiffusionRandom (Turing's
//! two-morphogen model, as popularised by Greg Turk). Two chemicals A and B react in
//! every voxel and diffuse to their six neighbours; the volume wraps around on all
//! sides. The Java version cycled through named parameter sets every 20 to 35 s; here
//! Form chooses where along those sets the system lives, and a slow swing keeps it from
//! settling.
//!
//! ```text
//! inputs:  f = form, T = simulated seconds (Flow-integrated)
//! step:    25 steps per simulated second (at most 16 per frame)
//!          A += 0.01·(A·B − A − 12) + 0.01·Dₐ·∇²A
//!          B += 0.01·(16 − A·B)     + 0.01·D_b·∇²B         both clamped to [0, 100]
//! (Dₐ, D_b) interpolated along the Java presets, from broad to fine features:
//!          cheetah (3.5, 16) · fingerprint (1, 16) · colony (1.6, 6) · pocked (1, 3) · fine (0.1, 1)
//!          position = f·4 + 0.6·sin(2π·T/40), clamped
//! life:    every 2.5 s a random voxel and its neighbours are disturbed, so patterns
//!          keep reshaping instead of freezing
//! void:    by rank of A: exactly the void fraction stays dark (see `void_by_rank`)
//! color:   u = rank of A in [0, 1);  q = ρ·T + 1.5·u   palette colors (like the
//!          Java hue mapping of A, but through the palette); heat = saturation
//! ```

use glam::Vec3;

use crate::content::common::{PaletteMix, Rng, apply_heat, void_by_rank, voxel_index};
use crate::content::{Content, advance_seconds};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const PRESETS: [(f32, f32); 5] = [(3.5, 16.0), (1.0, 16.0), (1.6, 6.0), (1.0, 3.0), (0.1, 1.0)];
const SWING: f32 = 0.6; // presets
const SWING_PERIOD: f64 = 40.0; // simulated seconds
const STEPS_PER_SECOND: f32 = 25.0;
const MAX_STEPS: usize = 16;
const RATE: f32 = 0.01;
const LIMIT: f32 = 100.0;
const DISTURB_INTERVAL: f32 = 2.5; // simulated seconds
const COLOR_SPREAD: f32 = 1.5; // palette colors from the lowest to the highest A
const ROTATION: f64 = 0.03; // palette colors per simulated second
const VOID_FADE: f32 = 0.25;
const SEED: u64 = 0x7E7;

pub struct Pleiades {
    phase: f64,
    time: f64,
    rng: Rng,
    dim: (usize, usize, usize),
    a: Vec<f32>,
    b: Vec<f32>,
    da: Vec<f32>,
    db: Vec<f32>,
    steps: f32,   // steps owed
    disturb: f32, // seconds until the next disturbance
    palette: PaletteMix,
    values: Vec<f32>,
    ranks: Vec<f32>,
    order: Vec<u32>,
}

impl Pleiades {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            time: 0.0,
            rng: Rng::new(SEED),
            dim: (0, 0, 0),
            a: Vec::new(),
            b: Vec::new(),
            da: Vec::new(),
            db: Vec::new(),
            steps: 0.0,
            disturb: DISTURB_INTERVAL,
            palette: PaletteMix::default(),
            values: Vec::new(),
            ranks: Vec::new(),
            order: Vec::new(),
        }
    }

    fn reset(&mut self, dim: (usize, usize, usize)) {
        self.dim = dim;
        self.time = 0.0;
        self.rng = Rng::new(SEED);
        self.steps = 0.0;
        self.disturb = DISTURB_INTERVAL;
        let voxels = dim.0 * dim.1 * dim.2;
        // Around the steady state A = B = 4, with noise to break the symmetry.
        self.a = (0..voxels)
            .map(|_| 4.0 + self.rng.range(-2.0, 2.0))
            .collect();
        self.b = (0..voxels)
            .map(|_| 4.0 + self.rng.range(-2.0, 2.0))
            .collect();
        self.da = vec![0.0; voxels];
        self.db = vec![0.0; voxels];
        self.values = vec![0.0; voxels];
        self.ranks = vec![0.0; voxels];
    }

    fn diffusion(&self, form: f32) -> (f32, f32) {
        let swing = SWING * (std::f64::consts::TAU * self.time / SWING_PERIOD).sin() as f32;
        let position =
            (form * (PRESETS.len() - 1) as f32 + swing).clamp(0.0, (PRESETS.len() - 1) as f32);
        let index = (position.floor() as usize).min(PRESETS.len() - 2);
        let t = position - index as f32;
        let (a0, b0) = PRESETS[index];
        let (a1, b1) = PRESETS[index + 1];
        (a0 + (a1 - a0) * t, b0 + (b1 - b0) * t)
    }

    fn react(&mut self, ca: f32, cb: f32) {
        let (nx, ny, nz) = self.dim;
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    let i = voxel_index(self.dim, x, y, z);
                    let neighbours = [
                        voxel_index(self.dim, (x + nx - 1) % nx, y, z),
                        voxel_index(self.dim, (x + 1) % nx, y, z),
                        voxel_index(self.dim, x, (y + ny - 1) % ny, z),
                        voxel_index(self.dim, x, (y + 1) % ny, z),
                        voxel_index(self.dim, x, y, (z + nz - 1) % nz),
                        voxel_index(self.dim, x, y, (z + 1) % nz),
                    ];
                    let (a, b) = (self.a[i], self.b[i]);
                    let lap_a: f32 = neighbours.iter().map(|&n| self.a[n]).sum::<f32>() - 6.0 * a;
                    let lap_b: f32 = neighbours.iter().map(|&n| self.b[n]).sum::<f32>() - 6.0 * b;
                    self.da[i] = RATE * (a * b - a - 12.0 + ca * lap_a);
                    self.db[i] = RATE * (16.0 - a * b + cb * lap_b);
                }
            }
        }
        for i in 0..self.a.len() {
            self.a[i] = (self.a[i] + self.da[i]).clamp(0.0, LIMIT);
            self.b[i] = (self.b[i] + self.db[i]).clamp(0.0, LIMIT);
        }
    }

    fn disturb(&mut self) {
        let (nx, ny, nz) = self.dim;
        let (x, y, z) = (self.rng.below(nx), self.rng.below(ny), self.rng.below(nz));
        let a = self.rng.range(0.0, 12.0);
        let b = self.rng.range(0.0, 12.0);
        for (dx, dy, dz) in [
            (0, 0, 0),
            (1, 0, 0),
            (nx - 1, 0, 0),
            (0, 1, 0),
            (0, ny - 1, 0),
            (0, 0, 1),
            (0, 0, nz - 1),
        ] {
            let i = voxel_index(self.dim, (x + dx) % nx, (y + dy) % ny, (z + dz) % nz);
            self.a[i] = a;
            self.b[i] = b;
        }
    }

    fn step(&mut self, dt: f32, form: f32) {
        self.steps += dt * STEPS_PER_SECOND;
        let steps = (self.steps.floor() as usize).min(MAX_STEPS);
        self.steps = (self.steps - steps as f32).min(1.0);
        let h = 1.0 / STEPS_PER_SECOND;
        for _ in 0..steps {
            let (ca, cb) = self.diffusion(form);
            self.react(ca, cb);
            self.disturb -= h;
            if self.disturb <= 0.0 {
                self.disturb += DISTURB_INTERVAL;
                self.disturb();
            }
            self.time += h as f64;
        }
    }
}

impl Content for Pleiades {
    fn name(&self) -> &str {
        "Pleiades"
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
        self.palette.select(state.palette(), "Pleiades");
        self.step(dt, state.form());

        self.values.copy_from_slice(&self.a);
        void_by_rank(&mut self.values, &mut self.order, state.void(), VOID_FADE);
        // Color by rank too, so the palette spreads evenly whatever the range of A.
        let n = self.order.len() as f32;
        for (position, &index) in self.order.iter().enumerate() {
            self.ranks[index as usize] = position as f32 / n;
        }

        let len = self.palette.len() as f64;
        let rotation = (ROTATION * self.time).rem_euclid(len) as f32;
        let heat = state.heat();
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    let i = voxel_index(dim, x, y, z);
                    let intensity = self.values[i];
                    if intensity <= 0.0 {
                        next.set(x, y, z, Vec3::ZERO);
                        continue;
                    }
                    let color = self
                        .palette
                        .wrapped(rotation + COLOR_SPREAD * self.ranks[i]);
                    next.set(x, y, z, apply_heat(color, heat) * intensity);
                }
            }
        }
    }
}
