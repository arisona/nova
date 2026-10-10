//! Tannhäuser Gate: flames, rising embers and sparks.
//!
//! Ported from the Java modules Fire, Fire (Old), Stars and Random. Fire was a single
//! color, so Heat picks the flame color from the palette, and cooler parts drift
//! towards the color before it.
//!
//! ```text
//! inputs:  p = (x, y, z), f = form, T = simulated seconds (Flow-integrated), Z = height
//! anchors (weights blend neighbours, as Form does in Orion's Belt):
//!          bed (0)      one flame sheet rising and falling together: layers
//!          tongues (¼)  every column flickers on its own (Fire (Old)): columns
//!          embers (½)   glowing spheres rising from the floor, shrinking (Fire): blobs
//!          flares (¾)   short-lived sparks with star-shaped glints (Stars): patches
//!          sparks (1)   single voxels flashing and drifting up (Random): grain
//! flames:  ψ = column phase · coherence (0 for the bed, its own for tongues)
//!          h = Z·(1 − void)·1.1 · (0.35 + 0.65·sin²(0.6·T + ψ)) · (1 + 0.12·sin(5.3·T + 3ψ))
//!          flame(p) = S(h − z) · (1 − 0.55·z/Z)
//! pool:    the floor glows at 0.6·(1 − void), fading out as sparks take over
//! embers:  10 per module footprint, √(1 − d²/r²) with radius r 1.3–2.4 shrinking towards
//!          the top, rising 1.5–3 voxels/s; respawn below the floor
//! flares:  a spark whose glint reaches out along the three axes, Gaussian in time
//! sparks:  single voxels with a Gaussian flash in time, drifting up 0.6 voxels/s
//! temperature t = Σ anchor weight · element, capped at 1
//! void:    flames burn lower with Void, and at most the 1 − void hottest voxels stay lit
//! color:   q = heat·(len − 1) − 0.5·(1 − t)   clamped to the palette, so Heat slides the
//!          flame color through the palette and cool edges blend into the previous color
//! voxel  = C(q) · t^0.75                            a mild lift for embers, glints and sparks
//! ```

use glam::Vec3;

use crate::content::common::{
    PaletteMix, Rng, anchor_weight, ease, footprint, void_as_cap, voxel_index,
};
use crate::content::{Content, advance_seconds};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const ANCHORS: usize = 5; // bed, tongues, embers, flares, sparks
const FLAME_HEIGHT: f32 = 1.1; // share of the lit height a flame reaches at most
const FLAME_RATE: f32 = 0.6; // radians per simulated second
const FLICKER: f32 = 0.12;
const FLICKER_RATE: f32 = 5.3;
const FLAME_FADE: f32 = 0.55; // flames dim towards the top
const POOL: f32 = 0.6; // brightness of the glowing pool on the floor
const EMBERS: f32 = 10.0; // per module footprint
const EMBER_RADIUS: [f32; 2] = [1.3, 2.4];
const EMBER_RISE: [f32; 2] = [1.5, 3.0]; // voxels per simulated second
const FLARES: f32 = 8.0; // per module footprint
const SPARKS: f32 = 24.0; // per module footprint
const SPARK_LIFE: [f32; 2] = [0.5, 1.6]; // seconds, flash and pause
const SPARK_RISE: f32 = 0.6;
const FLARE_REACH: f32 = 1.3; // σ of a glint along each axis, voxels
const COOLING: f32 = 0.5; // palette colors cool edges drift towards the previous color
const SUBSTEP: f32 = 0.05;
const SEED: u64 = 0xF1A3;

struct Glow {
    position: Vec3,
    radius: f32,
    rise: f32,
}

struct Spark {
    position: Vec3,
    age: f32, // negative while waiting to appear
    life: f32,
}

pub struct TannhauserGate {
    phase: f64,
    time: f64,
    rng: Rng,
    dim: (usize, usize, usize),
    columns: Vec<f32>, // flame phase per column
    embers: Vec<Glow>,
    flares: Vec<Spark>,
    sparks: Vec<Spark>,
    palette: PaletteMix,
    temperature: Vec<f32>,
    scratch: Vec<f32>,
}

/// Gaussian flash over a spark's life, peaking at 1 halfway.
fn flash(spark: &Spark) -> f32 {
    if spark.age <= 0.0 {
        return 0.0;
    }
    let t = spark.age / spark.life - 0.5;
    (-t * t / (2.0 * 0.22 * 0.22)).exp()
}

impl TannhauserGate {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            time: 0.0,
            rng: Rng::new(SEED),
            dim: (0, 0, 0),
            columns: Vec::new(),
            embers: Vec::new(),
            flares: Vec::new(),
            sparks: Vec::new(),
            palette: PaletteMix::default(),
            temperature: Vec::new(),
            scratch: Vec::new(),
        }
    }

    fn new_glow(&mut self, below_floor: bool) -> Glow {
        let radius = self.rng.range(EMBER_RADIUS[0], EMBER_RADIUS[1]);
        let mut position = self.rng.position(self.dim);
        if below_floor {
            position.z = -radius - self.rng.range(0.0, 3.0);
        }
        Glow {
            position,
            radius,
            rise: self.rng.range(EMBER_RISE[0], EMBER_RISE[1]),
        }
    }

    fn new_spark(&mut self, first: bool) -> Spark {
        let life = self.rng.range(SPARK_LIFE[0], SPARK_LIFE[1]);
        Spark {
            position: self.rng.position(self.dim).round(),
            age: -self.rng.range(0.0, if first { 2.0 * life } else { life }),
            life,
        }
    }

    fn reset(&mut self, dim: (usize, usize, usize)) {
        self.dim = dim;
        self.time = 0.0;
        self.rng = Rng::new(SEED);
        self.columns = (0..dim.0 * dim.1)
            .map(|_| self.rng.range(0.0, std::f32::consts::TAU))
            .collect();
        let scale = footprint(dim);
        self.embers = (0..(EMBERS * scale).round() as usize)
            .map(|_| self.new_glow(false))
            .collect();
        self.flares = (0..(FLARES * scale).round() as usize)
            .map(|_| self.new_spark(true))
            .collect();
        self.sparks = (0..(SPARKS * scale).round() as usize)
            .map(|_| self.new_spark(true))
            .collect();
        let voxels = dim.0 * dim.1 * dim.2;
        self.temperature = vec![0.0; voxels];
    }

    fn step(&mut self, dt: f32) {
        let steps = (dt / SUBSTEP).ceil() as usize;
        let top = self.dim.2 as f32;
        for _ in 0..steps {
            let h = dt / steps as f32;
            for i in 0..self.embers.len() {
                let glow = &mut self.embers[i];
                glow.position.z += glow.rise * h;
                if glow.position.z - glow.radius > top {
                    self.embers[i] = self.new_glow(true);
                }
            }
            for i in 0..self.flares.len() {
                self.flares[i].age += h;
                if self.flares[i].age > self.flares[i].life {
                    self.flares[i] = self.new_spark(false);
                }
            }
            for i in 0..self.sparks.len() {
                let spark = &mut self.sparks[i];
                spark.age += h;
                if spark.age > 0.0 {
                    spark.position.z += SPARK_RISE * h;
                }
                if spark.age > spark.life || spark.position.z > top {
                    self.sparks[i] = self.new_spark(false);
                }
            }
        }
        self.time += dt as f64;
    }
}

impl Content for TannhauserGate {
    fn name(&self) -> &str {
        "Tannhäuser Gate"
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
        self.palette.select(state.palette(), "Tannhäuser Gate");
        self.step(dt);

        let form = state.form();
        let void = state.void();
        let weights: Vec<f32> = (0..ANCHORS)
            .map(|i| anchor_weight(form, ANCHORS, i))
            .collect();
        let (bed, tongues, embers, flares, sparks) =
            (weights[0], weights[1], weights[2], weights[3], weights[4]);
        let flame_weight = bed + tongues;
        let coherence = tongues / flame_weight.max(1e-6); // 0 for the bed, 1 for tongues
        let top = dim.2 as f32;
        let reach = top * (1.0 - void) * FLAME_HEIGHT;
        let time = self.time as f32;
        self.temperature.fill(0.0);

        if flame_weight > 0.0 {
            for x in 0..dim.0 {
                for y in 0..dim.1 {
                    let psi = self.columns[x * dim.1 + y] * coherence;
                    let wave = (FLAME_RATE * time + psi).sin();
                    let flicker = 1.0 + FLICKER * (FLICKER_RATE * time + 3.0 * psi).sin();
                    let height = reach * (0.35 + 0.65 * wave * wave) * flicker;
                    for z in 0..dim.2 {
                        let zf = z as f32;
                        let flame = ease(height - zf) * (1.0 - FLAME_FADE * zf / top);
                        self.temperature[voxel_index(dim, x, y, z)] += flame_weight * flame;
                    }
                }
            }
        }

        // The glowing pool on the floor stays until sparks take over.
        let pool = POOL * (1.0 - void) * (1.0 - sparks);
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                let index = voxel_index(dim, x, y, 0);
                self.temperature[index] = self.temperature[index].max(pool);
            }
        }

        if embers > 0.0 {
            for glow in &self.embers {
                let height = (glow.position.z / top).clamp(0.0, 1.0);
                let r = glow.radius * (1.0 - 0.5 * height);
                let r2 = r * r;
                splat(
                    dim,
                    glow.position,
                    r,
                    |_, _, _, d2| {
                        let b = (r2 - d2) / r2;
                        (b > 0.0).then(|| embers * b.sqrt())
                    },
                    &mut self.temperature,
                );
            }
        }

        if flares > 0.0 {
            let s2 = 2.0 * FLARE_REACH * FLARE_REACH;
            for flare in &self.flares {
                let peak = flares * flash(flare);
                if peak <= 0.0 {
                    continue;
                }
                let c = flare.position;
                splat(
                    dim,
                    c,
                    3.0 * FLARE_REACH,
                    |x, y, z, _| {
                        let d = Vec3::new(x as f32, y as f32, z as f32) - c;
                        // A glint lives on the three axes through its center.
                        let on_axes = [
                            d.y == 0.0 && d.z == 0.0,
                            d.x == 0.0 && d.z == 0.0,
                            d.x == 0.0 && d.y == 0.0,
                        ];
                        let along = [d.x, d.y, d.z];
                        (0..3)
                            .filter(|&axis| on_axes[axis])
                            .map(|axis| (-along[axis] * along[axis] / s2).exp())
                            .reduce(f32::max)
                            .map(|glint| peak * glint)
                    },
                    &mut self.temperature,
                );
            }
        }

        if sparks > 0.0 {
            for spark in &self.sparks {
                let b = sparks * flash(spark);
                let p = spark.position.round();
                if b > 0.0 && p.z >= 0.0 && p.z < top {
                    let index = voxel_index(dim, p.x as usize, p.y as usize, p.z as usize);
                    self.temperature[index] += b;
                }
            }
        }

        self.temperature.iter_mut().for_each(|t| *t = t.min(1.0));
        void_as_cap(&mut self.temperature, &mut self.scratch, void);

        let hot = self.palette.heat_position(state.heat());
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    let t = self.temperature[voxel_index(dim, x, y, z)];
                    if t <= 0.0 {
                        next.set(x, y, z, Vec3::ZERO);
                        continue;
                    }
                    let color = self.palette.clamped(hot - COOLING * (1.0 - t));
                    next.set(x, y, z, color * t.powf(0.75));
                }
            }
        }
    }
}

/// Adds `value(x, y, z, distance²)` to every voxel within `radius` of `center`.
fn splat(
    dim: (usize, usize, usize),
    center: Vec3,
    radius: f32,
    value: impl Fn(usize, usize, usize, f32) -> Option<f32>,
    target: &mut [f32],
) {
    let low = (center - Vec3::splat(radius)).floor().max(Vec3::ZERO);
    let high = (center + Vec3::splat(radius)).ceil().min(Vec3::new(
        (dim.0 - 1) as f32,
        (dim.1 - 1) as f32,
        (dim.2 - 1) as f32,
    ));
    if low.cmpgt(high).any() {
        return;
    }
    for x in low.x as usize..=high.x as usize {
        for y in low.y as usize..=high.y as usize {
            for z in low.z as usize..=high.z as usize {
                let d2 = Vec3::new(x as f32, y as f32, z as f32).distance_squared(center);
                if let Some(v) = value(x, y, z, d2) {
                    target[voxel_index(dim, x, y, z)] += v;
                }
            }
        }
    }
}
