//! Display calibration: per-channel gamma and gain (white balance) for the hardware output,
//! and the test patterns shown while the calibration page is open.
//!
//! Calibration applies to the hardware, after Brightness. The simulator draws on a monitor,
//! which applies its own gamma, so it stays uncorrected except while a test pattern is
//! shown: then it previews the calibration.

use std::time::{Duration, Instant};

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::palettes::PALETTES;
use crate::voxel_image::VoxelImage;

pub const GAMMA_RANGE: (f32, f32) = (1.0, 3.0);
pub const GAIN_RANGE: (f32, f32) = (0.2, 1.0);

/// A test pattern switches itself off this long after it was last selected, so the wall
/// cannot stay stuck on it if the calibration page is closed without saying so.
const PATTERN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Per channel: red, green, blue. Gamma 1 and gain 1 send values to the LEDs unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Calibration {
    gamma: [f32; 3],
    gain: [f32; 3],
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            gamma: [1.0; 3],
            gain: [1.0; 3],
        }
    }
}

impl Calibration {
    pub fn gamma(&self) -> [f32; 3] {
        self.gamma
    }

    pub fn gain(&self) -> [f32; 3] {
        self.gain
    }

    /// Sets a channel's gamma, clamped to `GAMMA_RANGE`. Returns false, changing nothing,
    /// for a non-finite value or an unknown channel.
    pub fn set_gamma(&mut self, channel: usize, gamma: f32) -> bool {
        let valid = channel < 3 && gamma.is_finite();
        if valid {
            self.gamma[channel] = gamma.clamp(GAMMA_RANGE.0, GAMMA_RANGE.1);
        }
        valid
    }

    /// Sets a channel's gain, clamped to `GAIN_RANGE`. Returns false, changing nothing,
    /// for a non-finite value or an unknown channel.
    pub fn set_gain(&mut self, channel: usize, gain: f32) -> bool {
        let valid = channel < 3 && gain.is_finite();
        if valid {
            self.gain[channel] = gain.clamp(GAIN_RANGE.0, GAIN_RANGE.1);
        }
        valid
    }

    /// Settings restored from a file, validated through the setters.
    pub fn validated(self) -> Self {
        let mut calibration = Self::default();
        for channel in 0..3 {
            calibration.set_gamma(channel, self.gamma[channel]);
            calibration.set_gain(channel, self.gain[channel]);
        }
        calibration
    }

    /// The LED duty cycle in [0, 1] for an output value of one channel.
    pub fn apply(&self, channel: usize, value: f32) -> f32 {
        self.gain[channel] * value.clamp(0.0, 1.0).powf(self.gamma[channel])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pattern {
    #[default]
    Off,
    /// Ten layers from 10 % to 100 % white, bottom to top: for gamma and white balance.
    Gray,
    Red,
    Green,
    Blue,
    /// The selected palette's colors as layers, to compare with the web app.
    Palette,
}

impl Pattern {
    const ALL: [Pattern; 6] = [
        Pattern::Off,
        Pattern::Gray,
        Pattern::Red,
        Pattern::Green,
        Pattern::Blue,
        Pattern::Palette,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Pattern::Off => "off",
            Pattern::Gray => "gray",
            Pattern::Red => "red",
            Pattern::Green => "green",
            Pattern::Blue => "blue",
            Pattern::Palette => "palette",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|pattern| pattern.name() == name)
    }

    /// Fills `image` with the pattern, in layers from bottom to top.
    pub fn render(self, palette: usize, image: &mut VoxelImage) {
        let (dx, dy, dz) = image.dim();
        let colors = PALETTES[palette.min(PALETTES.len() - 1)].colors;
        for z in 0..dz {
            let level = (z + 1) as f32 / dz as f32;
            let color = match self {
                Pattern::Off => Vec3::ZERO,
                Pattern::Gray => Vec3::splat(level),
                Pattern::Red => Vec3::new(level, 0.0, 0.0),
                Pattern::Green => Vec3::new(0.0, level, 0.0),
                Pattern::Blue => Vec3::new(0.0, 0.0, level),
                Pattern::Palette => colors[z * colors.len() / dz].color(),
            };
            for x in 0..dx {
                for y in 0..dy {
                    image.set(x, y, z, color);
                }
            }
        }
    }
}

/// The selected test pattern and when it was selected.
#[derive(Debug, Default)]
pub struct PatternSelection {
    pattern: Pattern,
    selected: Option<Instant>,
}

impl PatternSelection {
    /// The pattern to show; `Off` once it has timed out.
    pub fn pattern(&self) -> Pattern {
        match self.selected {
            Some(selected) if selected.elapsed() < PATTERN_TIMEOUT => self.pattern,
            _ => Pattern::Off,
        }
    }

    pub fn select(&mut self, pattern: Pattern) {
        self.pattern = pattern;
        self.selected = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_maps_values_and_rejects_invalid_settings() {
        let mut calibration = Calibration::default();
        assert_eq!(calibration.apply(0, 0.5), 0.5);
        assert_eq!(calibration.apply(2, 1.7), 1.0);
        assert!(calibration.set_gamma(1, 2.0));
        assert!(calibration.set_gain(1, 0.8));
        assert!((calibration.apply(1, 0.5) - 0.8 * 0.25).abs() < 1e-6);
        assert!(!calibration.set_gamma(0, f32::NAN));
        assert!(!calibration.set_gain(3, 0.5));
        assert!(calibration.set_gamma(0, 9.0));
        assert_eq!(calibration.gamma()[0], GAMMA_RANGE.1);
        assert_eq!(calibration.gamma()[2], 1.0);
        for pattern in Pattern::ALL {
            assert_eq!(Pattern::from_name(pattern.name()), Some(pattern));
        }
    }
}
