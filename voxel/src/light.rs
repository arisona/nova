//! Measures the light for the sound: features of every rendered frame of content, before
//! Brightness (see `Features`). The sound smooths them over seconds, so sound and light
//! reference each other without being locked together.

use glam::Vec3;
use palette::convert::FromColorUnclamped;
use palette::{Oklch, Srgb};

use crate::voxel_image::VoxelImage;
use nova_core::{Features, HUES};

/// Activity is measured per display frame, whatever the render rate.
const FRAME_SECONDS: f32 = 0.04;

/// Voxels of at least this luminance count as lit.
const LIT: f32 = 0.02;

/// Mean chroma from which the light counts as fully colored; grayer light hands less to
/// `Features::hues`.
const COLORED: f32 = 0.03;

/// Measures the features of the light (see `Features`) from rendered frames.
#[derive(Default)]
pub struct Meter {
    previous: Vec<f32>,
}

impl Meter {
    /// `image` is the content before Brightness, `delta` the time since the previous frame,
    /// in seconds.
    pub fn measure(&mut self, image: &VoxelImage, delta: f32) -> Features {
        let (dx, dy, dz) = image.dim();
        let count = dx * dy * dz;
        let fresh = self.previous.len() != count;
        if fresh {
            self.previous = vec![0.0; count];
        }
        let (mut total, mut change, mut lit) = (0.0, 0.0, 0);
        let (mut balance, mut spread, mut height) = (0.0, 0.0, 0.0);
        let mut hues = [0.0; HUES];
        let mut index = 0;
        for x in 0..dx {
            // From −1 at the left edge to 1 at the right, at the voxels' centres.
            let position = (2 * x + 1) as f32 / dx as f32 - 1.0;
            for y in 0..dy {
                for z in 0..dz {
                    let rgb = image.get(x, y, z).clamp(Vec3::ZERO, Vec3::ONE);
                    let luminance = 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
                    total += luminance;
                    balance += luminance * position;
                    spread += luminance * position * position;
                    height += luminance * (z as f32 + 0.5) / dz as f32;
                    lit += (luminance >= LIT) as usize;
                    change += (luminance - self.previous[index]).abs();
                    self.previous[index] = luminance;
                    index += 1;
                    if luminance > 1e-4 {
                        let color = Oklch::from_color_unclamped(
                            Srgb::new(rgb.x, rgb.y, rgb.z).into_linear::<f32>(),
                        );
                        let weight = color.chroma * luminance;
                        let place = color.hue.into_positive_degrees() / 30.0;
                        let below = (place as usize).min(HUES - 1);
                        let fraction = (place - below as f32).clamp(0.0, 1.0);
                        hues[below] += weight * (1.0 - fraction);
                        hues[(below + 1) % HUES] += weight * fraction;
                    }
                }
            }
        }
        if total <= 1e-6 {
            return Features::default();
        }
        balance /= total;
        let chroma = hues.iter().sum::<f32>();
        let colored = (chroma / total / COLORED).min(1.0);
        for share in &mut hues {
            *share *= colored / chroma.max(1e-9);
        }
        // A nearly dark image must not read as a burst of activity.
        let reference = total.max(0.02 * count as f32);
        Features {
            level: total / count as f32,
            balance,
            // Light spread evenly from left to right has a deviation of 1/√3.
            width: ((spread / total - balance * balance).max(0.0) * 3.0)
                .sqrt()
                .min(1.0),
            height: height / total,
            density: lit as f32 / count as f32,
            activity: if fresh {
                0.0
            } else {
                change / reference * FRAME_SECONDS / delta.max(0.005)
            },
            hues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_follow_the_light_and_activity_its_change() {
        let mut image = VoxelImage::new((5, 5, 10));
        let mut meter = Meter::default();
        assert_eq!(meter.measure(&image, 0.04), Features::default());
        for y in 0..5 {
            for z in 0..10 {
                image.set(0, y, z, Vec3::new(1.0, 0.0, 0.0));
            }
        }
        let left = meter.measure(&image, 0.04);
        assert!((left.balance + 0.8).abs() < 1e-5, "{}", left.balance);
        assert!(left.width < 1e-3 && (left.height - 0.5).abs() < 1e-5);
        assert!((left.density - 0.2).abs() < 1e-6);
        // Red lies at about 29° in OKLCh, between the first two hues.
        assert!((left.hues.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert!(left.hues[0] + left.hues[1] > 0.999, "{:?}", left.hues);
        assert_eq!(meter.measure(&image, 0.04).activity, 0.0);

        let mut image = VoxelImage::new((5, 5, 10));
        for x in 0..5 {
            for y in 0..5 {
                image.set(x, y, 0, Vec3::ONE);
            }
        }
        let bottom = meter.measure(&image, 0.04);
        assert!(bottom.activity > 0.0, "the light moved");
        assert!((bottom.height - 0.05).abs() < 1e-5 && bottom.width > 0.95);
        assert!(bottom.balance.abs() < 1e-5);
        assert!(
            bottom.hues.iter().sum::<f32>() < 0.05,
            "white is gray light"
        );
    }
}
