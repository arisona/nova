pub struct VoxelImage {
    data: Vec<f32>, // Stores RGB values in a flat layout
    dx: usize,
    dy: usize,
    dz: usize,
}

impl VoxelImage {
    pub fn new(dx: usize, dy: usize, dz: usize) -> Self {
        Self {
            data: vec![0.0; dx * dy * dz * 3], // Initialize with zeros
            dx,
            dy,
            dz,
        }
    }

    fn get(&self, x: usize, y: usize, z: usize) -> (f32, f32, f32) {
        let index = (z * self.dx * self.dy + y * self.dx + x) * 3;
        (
            self.data[index],     // R
            self.data[index + 1], // G
            self.data[index + 2], // B
        )
    }

    fn set(&mut self, x: usize, y: usize, z: usize, color: (f32, f32, f32)) {
        let index = (z * self.dx * self.dy + y * self.dx + x) * 3;
        self.data[index] = color.0; // R
        self.data[index + 1] = color.1; // G
        self.data[index + 2] = color.2; // B
    }

    fn clear(&mut self) {
        self.data.fill(0.0);
    }

    fn fill(&mut self, color: (f32, f32, f32)) {
        self.data.chunks_exact_mut(3).for_each(|chunk| {
            chunk[0] = color.0; // R
            chunk[1] = color.1; // G
            chunk[2] = color.2; // B
        });
    }
}
