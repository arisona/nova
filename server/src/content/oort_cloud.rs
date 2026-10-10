//! Oort Cloud: glowing agents that leave fading trails.
//!
//! Ported from the Java modules Cylinder, Boids, BoidsNr and Snake. Form moves the
//! swarm from order to independence: a spinning helix, a flock, then snakes that crawl
//! voxel by voxel and burst when they trap themselves.
//!
//! ```text
//! inputs:  f = form, T = simulated seconds (Flow-integrated), 8 agents per module footprint
//! steering (continuous, form < 0.75), per agent i of n:
//!          orbit   seek a point circling the volume's axis (Cylinder):
//!                  θ = 0.9·T + 2π·i/n,  z = mid · (1 + 0.85·sin(0.45·T + 2.4·i))
//!          flock   separation, alignment and cohesion within 3 voxels (Boids)
//!          wander  a random nudge, stronger towards form 0.75
//!          walls   a soft push back from the boundary
//!          acc = (1 − S(2f))·orbit + flock·(1 − S(2f) − S(2f − 1)) + wander + walls
//!          speed is kept within [1, 2.5] voxels/s; heads deposit trilinearly, scaled
//!          so the nearest voxel gets full intensity
//! lattice  (form ≥ 0.75, Snake): every 0.3 s an agent steps one voxel along an axis,
//!          turning at random or when the next voxel is a wall or a fresh trail; with no
//!          way out it bursts, a sphere swelling to 3.5 voxels and fading over 1.2 s,
//!          and respawns elsewhere
//! trails:  intensity decays by e^(−dt/τ), τ = lerp(1.6, 0.4, void) s, shown as intensity^¼ so
//!          trails stay visible as light; a deposit keeps the brighter value and takes the
//!          depositing agent's color
//! void:    trails shorten with Void, and at most the 1 − void brightest voxels stay lit
//!          (see `void_as_cap`)
//! color:   each agent owns a palette position, rotating slowly; heat = saturation
//! ```

use std::f32::consts::{PI, TAU};

use glam::Vec3;

use crate::content::common::{
    PaletteMix, Rng, apply_heat, ease, footprint, void_as_cap, voxel_index,
};
use crate::content::{Content, advance_seconds};
use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

const AGENTS: f32 = 8.0; // per module footprint
const MAX_AGENTS: usize = 64;
const MIN_SPEED: f32 = 1.0; // voxels per simulated second
const MAX_SPEED: f32 = 2.5;
const ORBIT_RATE: f32 = 0.9; // radians per simulated second
const ORBIT_GAIN: f32 = 3.0;
const ORBIT_HEIGHT: f32 = 0.85; // share of the half-height agents climb and dive
const NEIGHBORHOOD: f32 = 3.0; // voxels
const WANDER: [f32; 2] = [0.6, 3.0]; // random acceleration at form 0 and 0.75
const LATTICE_FORM: f32 = 0.75;
const LATTICE_STEP: f32 = 0.3; // seconds per voxel
const LATTICE_TURN: f32 = 0.2; // chance of a random turn per step
const TRAIL: [f32; 2] = [1.6, 0.4]; // trail time constant at void 0 and 1, seconds
const FRESH_TRAIL: f32 = 0.25; // snakes don't cross trails brighter than this
const POP_LIFE: f32 = 1.2;
const POP_RADIUS: f32 = 3.5;
const ROTATION: f32 = 0.04; // palette colors per simulated second
const SUBSTEP: f32 = 0.05;
const SEED: u64 = 0x5A4D;

const AXES: [Vec3; 6] = [
    Vec3::X,
    Vec3::NEG_X,
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::Z,
    Vec3::NEG_Z,
];

struct Agent {
    position: Vec3,
    velocity: Vec3,
    hue: f32, // palette position as a fraction of the palette
    axis: usize,
    timer: f32,
}

struct Pop {
    center: Vec3,
    hue: f32,
    age: f32,
}

pub struct OortCloud {
    phase: f64,
    time: f64,
    rng: Rng,
    dim: (usize, usize, usize),
    agents: Vec<Agent>,
    pops: Vec<Pop>,
    trail: Vec<f32>,
    trail_hue: Vec<f32>,
    palette: PaletteMix,
    intensity: Vec<f32>,
    hue: Vec<f32>,
    scratch: Vec<f32>,
}

impl OortCloud {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            time: 0.0,
            rng: Rng::new(SEED),
            dim: (0, 0, 0),
            agents: Vec::new(),
            pops: Vec::new(),
            trail: Vec::new(),
            trail_hue: Vec::new(),
            palette: PaletteMix::default(),
            intensity: Vec::new(),
            hue: Vec::new(),
            scratch: Vec::new(),
        }
    }

    fn reset(&mut self, dim: (usize, usize, usize)) {
        self.dim = dim;
        self.time = 0.0;
        self.rng = Rng::new(SEED);
        self.pops.clear();
        let count = ((AGENTS * footprint(dim)).round() as usize).clamp(1, MAX_AGENTS);
        self.agents = (0..count)
            .map(|i| Agent {
                position: self.rng.position(dim),
                velocity: self.rng.direction() * MIN_SPEED,
                hue: (i as f32 * 0.618_034).fract(),
                axis: self.rng.below(AXES.len()),
                timer: self.rng.range(0.0, LATTICE_STEP),
            })
            .collect();
        let voxels = dim.0 * dim.1 * dim.2;
        self.trail = vec![0.0; voxels];
        self.trail_hue = vec![0.0; voxels];
        self.intensity = vec![0.0; voxels];
        self.hue = vec![0.0; voxels];
    }

    fn max(&self) -> Vec3 {
        Vec3::new(
            (self.dim.0 - 1) as f32,
            (self.dim.1 - 1) as f32,
            (self.dim.2 - 1) as f32,
        )
    }

    fn deposit(&mut self, voxel: [usize; 3], amount: f32, hue: f32) {
        let index = voxel_index(self.dim, voxel[0], voxel[1], voxel[2]);
        if amount >= self.trail[index] {
            self.trail[index] = amount;
            self.trail_hue[index] = hue;
        }
    }

    /// Splats a head at a continuous position over its eight neighbouring voxels.
    fn deposit_trilinear(&mut self, position: Vec3, hue: f32) {
        let max = self.max();
        let p = position.clamp(Vec3::ZERO, max);
        let base = p.floor();
        let t = p - base;
        // The nearest voxel gets full intensity, the others in proportion.
        let nearest = t.max(Vec3::ONE - t);
        let peak = nearest.x * nearest.y * nearest.z;
        for corner in 0..8 {
            let offset = Vec3::new(
                (corner & 1) as f32,
                ((corner >> 1) & 1) as f32,
                ((corner >> 2) & 1) as f32,
            );
            let voxel = (base + offset).min(max);
            let w = Vec3::ONE - offset + t * (2.0 * offset - Vec3::ONE);
            let weight = w.x * w.y * w.z;
            if weight > 0.0 {
                self.deposit(
                    [voxel.x as usize, voxel.y as usize, voxel.z as usize],
                    weight / peak,
                    hue,
                );
            }
        }
    }

    fn is_free(&self, voxel: Vec3) -> bool {
        let max = self.max();
        if voxel.cmplt(Vec3::ZERO).any() || voxel.cmpgt(max).any() {
            return false;
        }
        let index = voxel_index(
            self.dim,
            voxel.x as usize,
            voxel.y as usize,
            voxel.z as usize,
        );
        self.trail[index] < FRESH_TRAIL
    }

    fn steer(&mut self, i: usize, h: f32, form: f32) {
        let n = self.agents.len();
        let max = self.max();
        let center = max * 0.5;
        let orbit_weight = 1.0 - ease(2.0 * form);
        let flock_weight = 1.0 - orbit_weight - ease(2.0 * form - 1.0);
        let wander = WANDER[0] + (WANDER[1] - WANDER[0]) * (form / LATTICE_FORM).min(1.0);
        let time = self.time as f32;

        let agent = &self.agents[i];
        let mut acceleration = Vec3::ZERO;
        if orbit_weight > 0.0 {
            let theta = ORBIT_RATE * time + TAU * i as f32 / n as f32;
            let lift = (0.45 * time + 2.4 * i as f32).sin();
            let target = Vec3::new(
                center.x + center.x.max(0.5) * theta.cos(),
                center.y + center.y.max(0.5) * theta.sin(),
                center.z * (1.0 + ORBIT_HEIGHT * lift),
            );
            let desired = ((target - agent.position) * 2.0).clamp_length_max(MAX_SPEED);
            acceleration += orbit_weight * ORBIT_GAIN * (desired - agent.velocity);
        }
        if flock_weight > 0.0 {
            let mut separation = Vec3::ZERO;
            let mut alignment = Vec3::ZERO;
            let mut cohesion = Vec3::ZERO;
            let mut neighbours = 0.0;
            for (j, other) in self.agents.iter().enumerate() {
                let offset = agent.position - other.position;
                let d2 = offset.length_squared();
                if j == i || d2 > NEIGHBORHOOD * NEIGHBORHOOD || d2 == 0.0 {
                    continue;
                }
                separation += offset / d2;
                alignment += other.velocity;
                cohesion += other.position;
                neighbours += 1.0;
            }
            if neighbours > 0.0 {
                alignment = alignment / neighbours - agent.velocity;
                cohesion = cohesion / neighbours - agent.position;
            }
            acceleration += flock_weight * (2.0 * separation + 0.8 * alignment + 0.5 * cohesion);
        }
        acceleration += self.rng.direction() * wander;
        for axis in 0..3 {
            let low = 1.0 - agent.position[axis];
            let high = agent.position[axis] - (max[axis] - 1.0);
            if low > 0.0 {
                acceleration[axis] += 4.0 * low;
            }
            if high > 0.0 {
                acceleration[axis] -= 4.0 * high;
            }
        }

        let agent = &mut self.agents[i];
        agent.velocity += acceleration * h;
        let speed = agent.velocity.length();
        if speed < MIN_SPEED {
            agent.velocity = if speed > 1e-4 {
                agent.velocity * (MIN_SPEED / speed)
            } else {
                Vec3::Z * MIN_SPEED
            };
        }
        agent.velocity = agent.velocity.clamp_length_max(MAX_SPEED);
        agent.position += agent.velocity * h;
        for axis in 0..3 {
            if agent.position[axis] < 0.0 {
                agent.position[axis] = -agent.position[axis];
                agent.velocity[axis] = agent.velocity[axis].abs();
            } else if agent.position[axis] > max[axis] {
                agent.position[axis] = 2.0 * max[axis] - agent.position[axis];
                agent.velocity[axis] = -agent.velocity[axis].abs();
            }
        }
        agent.position = agent.position.clamp(Vec3::ZERO, max);
        let (position, hue) = (agent.position, agent.hue);
        self.deposit_trilinear(position, hue);
    }

    fn crawl(&mut self, i: usize, h: f32) {
        self.agents[i].timer += h;
        while self.agents[i].timer >= LATTICE_STEP {
            self.agents[i].timer -= LATTICE_STEP;
            let head = self.agents[i].position.round();
            self.agents[i].position = head;
            if self.rng.unit() < LATTICE_TURN {
                self.agents[i].axis = self.rng.below(AXES.len());
            }
            let mut moved = false;
            for attempt in 0..AXES.len() * 2 {
                let axis = if attempt == 0 {
                    self.agents[i].axis
                } else {
                    self.rng.below(AXES.len())
                };
                if self.is_free(head + AXES[axis]) {
                    let agent = &mut self.agents[i];
                    agent.axis = axis;
                    agent.position = head + AXES[axis];
                    agent.velocity = AXES[axis] / LATTICE_STEP;
                    moved = true;
                    break;
                }
            }
            let agent = &self.agents[i];
            let (position, hue) = (agent.position, agent.hue);
            if moved {
                self.deposit(
                    [
                        position.x as usize,
                        position.y as usize,
                        position.z as usize,
                    ],
                    1.0,
                    hue,
                );
            } else {
                self.pops.push(Pop {
                    center: position,
                    hue,
                    age: 0.0,
                });
                let respawn = self.rng.position(self.dim).round();
                self.agents[i].position = respawn;
            }
        }
    }

    fn step(&mut self, dt: f32, form: f32, void: f32) {
        let steps = (dt / SUBSTEP).ceil() as usize;
        let tau = TRAIL[0] + (TRAIL[1] - TRAIL[0]) * void;
        for _ in 0..steps {
            let h = dt / steps as f32;
            let decay = (-h / tau).exp();
            self.trail.iter_mut().for_each(|value| *value *= decay);
            for pop in self.pops.iter_mut() {
                pop.age += h;
            }
            self.pops.retain(|pop| pop.age < POP_LIFE);
            for i in 0..self.agents.len() {
                if form < LATTICE_FORM {
                    self.steer(i, h, form);
                } else {
                    self.crawl(i, h);
                }
            }
            self.time += h as f64;
        }
    }
}

impl Content for OortCloud {
    fn name(&self) -> &str {
        "Oort Cloud"
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
        self.palette.select(state.palette(), "Oort Cloud");
        self.step(dt, state.form(), state.void());

        self.intensity.copy_from_slice(&self.trail);
        self.hue.copy_from_slice(&self.trail_hue);
        for pop in &self.pops {
            let r = POP_RADIUS * (PI * pop.age / POP_LIFE).sin();
            let r2 = r * r;
            if r2 <= 0.0 {
                continue;
            }
            for x in 0..dim.0 {
                for y in 0..dim.1 {
                    for z in 0..dim.2 {
                        let d2 =
                            Vec3::new(x as f32, y as f32, z as f32).distance_squared(pop.center);
                        let b = (r2 - d2) / r2;
                        let index = voxel_index(dim, x, y, z);
                        if b > self.intensity[index] {
                            self.intensity[index] = b;
                            self.hue[index] = pop.hue;
                        }
                    }
                }
            }
        }
        void_as_cap(&mut self.intensity, &mut self.scratch, state.void());

        let len = self.palette.len() as f32;
        let rotation = (ROTATION as f64 * self.time).rem_euclid(len as f64) as f32;
        let heat = state.heat();
        for x in 0..dim.0 {
            for y in 0..dim.1 {
                for z in 0..dim.2 {
                    let index = voxel_index(dim, x, y, z);
                    let intensity = self.intensity[index].min(1.0).powf(0.25);
                    if intensity <= 0.0 {
                        next.set(x, y, z, Vec3::ZERO);
                        continue;
                    }
                    let color = self.palette.wrapped(self.hue[index] * len + rotation);
                    next.set(x, y, z, apply_heat(color, heat) * intensity);
                }
            }
        }
    }
}
