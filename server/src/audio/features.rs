//! Features of the rendered image that shape the sound (see `doc/audio_concept.md`).
//!
//! Taken from the content image before Brightness, once per rendered frame. The instrument
//! smooths them over seconds, so sound and light reference each other without being locked
//! together.

use crate::voxel_image::VoxelImage;

/// Horizontal layers of a module, bottom to top.
pub const LAYERS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Features {
    /// Share of the light in each layer, bottom first; sums to 1 (even when dark).
    pub layers: [f32; LAYERS],
    /// Relative change of the light since the previous frame: 0 when still, around 0.01 to
    /// 0.1 for slow motion, more for fast or flickering content.
    pub activity: f32,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            layers: [1.0 / LAYERS as f32; LAYERS],
            activity: 0.0,
        }
    }
}

#[derive(Default)]
pub struct FeatureExtractor {
    previous: Vec<f32>,
}

impl FeatureExtractor {
    pub fn extract(&mut self, image: &VoxelImage) -> Features {
        let (dx, dy, dz) = image.dim();
        let fresh = self.previous.len() != dx * dy * dz;
        if fresh {
            self.previous = vec![0.0; dx * dy * dz];
        }
        let mut layers = [0.0; LAYERS];
        let mut total = 0.0;
        let mut change = 0.0;
        let mut index = 0;
        for x in 0..dx {
            for y in 0..dy {
                for z in 0..dz {
                    let rgb = image.get(x, y, z);
                    let luminance = 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
                    layers[(z * LAYERS / dz).min(LAYERS - 1)] += luminance;
                    total += luminance;
                    change += (luminance - self.previous[index]).abs();
                    self.previous[index] = luminance;
                    index += 1;
                }
            }
        }
        if total <= 1e-6 {
            return Features::default();
        }
        for layer in &mut layers {
            *layer /= total;
        }
        // A nearly dark image must not read as a burst of activity.
        let reference = total.max(0.02 * (dx * dy * dz) as f32);
        Features {
            layers,
            activity: if fresh { 0.0 } else { change / reference },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn layers_follow_the_light_and_activity_its_change() {
        let mut image = VoxelImage::new((5, 5, 10));
        let mut extractor = FeatureExtractor::default();
        assert_eq!(extractor.extract(&image), Features::default());
        for x in 0..5 {
            for y in 0..5 {
                image.set(x, y, 0, Vec3::ONE);
            }
        }
        let bottom = extractor.extract(&image);
        assert!((bottom.layers[0] - 1.0).abs() < 1e-6);
        assert_eq!(extractor.extract(&image).activity, 0.0);
        image.set(2, 2, 9, Vec3::ONE);
        let moved = extractor.extract(&image);
        assert!(moved.layers[9] > 0.0 && moved.activity > 0.0);
        assert!((moved.layers.iter().sum::<f32>() - 1.0).abs() < 1e-5);
    }
}
