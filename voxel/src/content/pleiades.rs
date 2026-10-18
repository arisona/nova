//! Pleiades: a reaction-diffusion system growing layers, columns, spots and grain.
//!
//! Ported from the Java modules ReactionDiffusion and ReactionDiffusionRandom (Turing's
//! two-morphogen model, as popularised by Greg Turk). Two chemicals A and B react in
//! every voxel and diffuse to their six neighbours; the volume wraps around on all
//! sides. The Java version cycled through named parameter sets every 20 to 35 s; here
//! Form chooses where along those sets the system lives and how the diffusion differs
//! across (x, y) and along z, and a slow swing keeps it from settling.
//!
//! ```text
//! inputs:  f = form, T = simulated seconds (Flow-integrated)
//! step:    25 steps per simulated second (at most 16 per frame)
//!          A += 0.01·(A·B − A − 12) + 0.01·Dₐ·(s_xy·∇²_xy A + s_z·∇²_z A)
//!          B += 0.01·(16 − A·B)     + 0.01·D_b·(s_xy·∇²_xy B + s_z·∇²_z B)   both clamped to [0, 100]
//! (Dₐ, D_b) interpolated along the Java presets, from broad to fine features:
//!          cheetah (3.5, 16) · fingerprint (1, 16) · colony (1.6, 6) · pocked (1, 3) · fine (0.1, 1)
//! form:    anchors (preset position p, scale across s_xy, scale along z s_z); scaling an axis
//!          stretches the pattern along it by √s, the ratio Dₐ : D_b stays
//!                     p   s_xy  s_z
//!          layers   = 0   1.3   0.3     (0)
//!          columns  = 0   0.15  2.5     (¼)
//!          blobs    = 1   1     1       (½)
//!          patches  = 3   1     1       (¾)
//!          grain    = 4   1     1       (1)
//!          neighbouring anchors blend by `form_weights`, p linearly, the scales geometrically;
//!          position = p + 0.6·sin(2π·T/40), clamped. The scales keep 0.01·D_b·(8·s_xy + 4·s_z) < 2,
//!          so the explicit step stays stable.
//! life:    every 1.5 s per module footprint, a random voxel pulls its surroundings towards
//!          random (A, B), weighted by exp(−r²) over an ellipsoid with radii 0.6·√(s·D_b)
//!          across and along: it seeds a feature of the current kind, so patterns keep
//!          reshaping instead of freezing
//! void:    by rank of A: exactly the void fraction stays dark (see `void_by_rank`)
//! color:   u = rank of A among the lit voxels, in [0, 1);  q = ρ·T + spread·u   palette colors
//!          (like the Java hue mapping of A, but through the palette);
//!          spread = 1.5 + S(2f − 1)·max(0, len − 1.5): about two colors up to all for grain;
//!          heat = saturation
//! ```

use glam::Vec3;

use crate::content::common::{
    PaletteMix, Rng, apply_heat, ease, footprint, form_weights, void_by_rank, voxel_index,
};
use crate::content::{Content, advance_seconds};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const PRESETS: [(f32, f32); 5] = [(3.5, 16.0), (1.0, 16.0), (1.6, 6.0), (1.0, 3.0), (0.1, 1.0)];
// Form anchors: preset position, diffusion scale across (x, y) and along z. Scaling an
// axis down shortens the pattern along it: broad across and short along z makes layers.
const ANCHORS: [(f32, f32, f32); 5] = [
    (0.0, 1.3, 0.3),  // layers
    (0.0, 0.15, 2.5), // columns
    (1.0, 1.0, 1.0),  // blobs
    (3.0, 1.0, 1.0),  // patches
    (4.0, 1.0, 1.0),  // grain
];
const SWING: f32 = 0.6; // presets
const SWING_PERIOD: f64 = 40.0; // simulated seconds
const STEPS_PER_SECOND: f32 = 25.0;
const MAX_STEPS: usize = 16;
const RATE: f32 = 0.01;
const LIMIT: f32 = 100.0;
const DISTURB_INTERVAL: f32 = 1.5; // simulated seconds per module footprint
const DISTURB_RADIUS: f32 = 0.6; // voxels per √D_b
const COLOR_SPREAD: f32 = 1.5; // palette colors from the lowest to the highest A, up to all for grain
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

    /// The diffusion rates (Dₐ, D_b) across (x, y) and along z.
    fn diffusion(&self, form: f32) -> [(f32, f32); 2] {
        let swing = SWING * (std::f64::consts::TAU * self.time / SWING_PERIOD).sin() as f32;
        let (mut position, mut across, mut along) = (swing, 0.0, 0.0);
        for (i, w) in form_weights(form, ANCHORS.len()) {
            let (anchor, a, z) = ANCHORS[i];
            position += w * anchor;
            across += w * a.ln();
            along += w * z.ln();
        }
        let position = position.clamp(0.0, (PRESETS.len() - 1) as f32);
        let index = (position.floor() as usize).min(PRESETS.len() - 2);
        let t = position - index as f32;
        let (a0, b0) = PRESETS[index];
        let (a1, b1) = PRESETS[index + 1];
        let (ca, cb) = (a0 + (a1 - a0) * t, b0 + (b1 - b0) * t);
        [across.exp(), along.exp()].map(|scale| (scale * ca, scale * cb))
    }

    fn react(&mut self, [(ca, cb), (ca_z, cb_z)]: [(f32, f32); 2]) {
        let (nx, ny, nz) = self.dim;
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    let i = voxel_index(self.dim, x, y, z);
                    let across = [
                        voxel_index(self.dim, (x + nx - 1) % nx, y, z),
                        voxel_index(self.dim, (x + 1) % nx, y, z),
                        voxel_index(self.dim, x, (y + ny - 1) % ny, z),
                        voxel_index(self.dim, x, (y + 1) % ny, z),
                    ];
                    let along = [
                        voxel_index(self.dim, x, y, (z + nz - 1) % nz),
                        voxel_index(self.dim, x, y, (z + 1) % nz),
                    ];
                    let (a, b) = (self.a[i], self.b[i]);
                    let lap = |c: &[f32], neighbours: &[usize]| {
                        neighbours.iter().map(|&n| c[n] - c[i]).sum::<f32>()
                    };
                    let lap_a = ca * lap(&self.a, &across) + ca_z * lap(&self.a, &along);
                    let lap_b = cb * lap(&self.b, &across) + cb_z * lap(&self.b, &along);
                    self.da[i] = RATE * (a * b - a - 12.0 + lap_a);
                    self.db[i] = RATE * (16.0 - a * b + lap_b);
                }
            }
        }
        for i in 0..self.a.len() {
            self.a[i] = (self.a[i] + self.da[i]).clamp(0.0, LIMIT);
            self.b[i] = (self.b[i] + self.db[i]).clamp(0.0, LIMIT);
        }
    }

    /// Pushes a region towards random values: an ellipsoid shaped like the features the
    /// current diffusion grows, so it seeds a new layer, column, spot or grain.
    fn disturb(&mut self, [(_, cb), (_, cb_z)]: [(f32, f32); 2]) {
        let (nx, ny, nz) = self.dim;
        let (x, y, z) = (self.rng.below(nx), self.rng.below(ny), self.rng.below(nz));
        let a = self.rng.range(0.0, 12.0);
        let b = self.rng.range(0.0, 12.0);
        let (across, along) = (DISTURB_RADIUS * cb.sqrt(), DISTURB_RADIUS * cb_z.sqrt());
        let distance = |p: usize, q: usize, n: usize| {
            let d = p.abs_diff(q);
            d.min(n - d) as f32
        };
        for (px, py, pz) in
            (0..nx).flat_map(|px| (0..ny).flat_map(move |py| (0..nz).map(move |pz| (px, py, pz))))
        {
            let r2 = (distance(px, x, nx).powi(2) + distance(py, y, ny).powi(2))
                / (across * across)
                + distance(pz, z, nz).powi(2) / (along * along);
            let w = (-r2).exp();
            let i = voxel_index(self.dim, px, py, pz);
            self.a[i] += w * (a - self.a[i]);
            self.b[i] += w * (b - self.b[i]);
        }
    }

    fn step(&mut self, dt: f32, form: f32) {
        self.steps += dt * STEPS_PER_SECOND;
        let steps = (self.steps.floor() as usize).min(MAX_STEPS);
        self.steps = (self.steps - steps as f32).min(1.0);
        let h = 1.0 / STEPS_PER_SECOND;
        for _ in 0..steps {
            let diffusion = self.diffusion(form);
            self.react(diffusion);
            self.disturb -= h;
            if self.disturb <= 0.0 {
                self.disturb += DISTURB_INTERVAL / footprint(self.dim);
                self.disturb(diffusion);
            }
            self.time += h as f64;
        }
    }
}

impl Content for Pleiades {
    fn name(&self) -> &str {
        "Pleiades"
    }

    fn render(&mut self, state: &RenderState, delta: f32, next: &mut VoxelImage) {
        let dt = advance_seconds(&mut self.phase, state, delta);
        let dim = next.dim();
        if state.should_reset() || self.dim != dim {
            self.reset(dim);
        }
        self.palette.select(state.palette(), "Pleiades");
        self.step(dt, state.form());

        self.values.copy_from_slice(&self.a);
        void_by_rank(&mut self.values, &mut self.order, state.void(), VOID_FADE);
        // Color by rank among the lit voxels, so the palette spreads evenly whatever the
        // range of A and whatever Void leaves dark.
        let n = self.order.len() as f32;
        let void = state.void();
        let lit = (1.0 - void).max(1e-3);
        for (position, &index) in self.order.iter().enumerate() {
            self.ranks[index as usize] = ((position as f32 / n - void) / lit).clamp(0.0, 1.0);
        }

        let len = self.palette.len() as f64;
        let rotation = (ROTATION * self.time).rem_euclid(len) as f32;
        let spread = COLOR_SPREAD
            + ease(2.0 * state.form() - 1.0) * (self.palette.len() as f32 - COLOR_SPREAD).max(0.0);
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
                    let color = self.palette.wrapped(rotation + spread * self.ranks[i]);
                    next.set(x, y, z, apply_heat(color, heat) * intensity);
                }
            }
        }
    }
}
