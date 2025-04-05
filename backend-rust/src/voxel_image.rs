use glam::{Vec3, vec3};

#[derive(Clone)]
pub struct VoxelImage {
    data: Vec<f32>, // Store RGB values in a flat layout
    dx: usize,
    dy: usize,
    dz: usize,
}

impl VoxelImage {
    pub fn new(dim: (usize, usize, usize)) -> Self {
        Self {
            data: vec![0.0; dim.0 * dim.1 * dim.2 * 3],
            dx: dim.0,
            dy: dim.1,
            dz: dim.2,
        }
    }

    pub fn dx(&self) -> usize {
        self.dx
    }

    pub fn dy(&self) -> usize {
        self.dy
    }

    pub fn dz(&self) -> usize {
        self.dz
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> Vec3 {
        let index = (x * self.dy * self.dz + y * self.dz + z) * 3;
        vec3(self.data[index], self.data[index + 1], self.data[index + 2])
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, color: Vec3) {
        let index = (x * self.dy * self.dz + y * self.dz + z) * 3;
        self.data[index] = color.x;
        self.data[index + 1] = color.y;
        self.data[index + 2] = color.z;
    }

    pub fn slice(&self, x: usize, y: usize) -> &[f32] {
        let start = (x * self.dy * self.dz + y * self.dz) * 3;
        let end = start + self.dz * 3;
        &self.data[start..end]
    }

    pub fn clear(&mut self) {
        self.data.fill(0.0);
    }

    pub fn fill(&mut self, color: Vec3) {
        self.data.chunks_exact_mut(3).for_each(|chunk| {
            chunk[0] = color.x;
            chunk[1] = color.y;
            chunk[2] = color.z;
        });
    }
}

pub fn rgb_to_hsb(rgb: Vec3) -> Vec3 {
    let (r, g, b) = (rgb.x, rgb.y, rgb.z);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let b = max;

    vec3(h, s, b)
}

pub(crate) fn hsb_to_rgb(hsb: Vec3) -> Vec3 {
    let (h, s, b) = (hsb.x, hsb.y, hsb.z);
    let h = h * 360.0;
    let c = b * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = b - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    vec3(r + m, g + m, b + m)
}
