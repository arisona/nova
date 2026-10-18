//! The features that describe the light, for the sound (see Audio in the README).

/// Hues in `Features::hues`, 30° apart.
pub const HUES: usize = 12;

/// Features of the light, measured by the display from every rendered frame. They describe
/// the light, not the display, so any renderer could measure them; how the sound answers
/// them is its own choice, rarely one to one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Features {
    /// Mean luminance of the voxels, from 0 (dark) to 1.
    pub level: f32,
    /// Where the light sits from left to right (along x), from −1 to 1.
    pub balance: f32,
    /// How far the light spreads from left to right: 0 in a single column, 1 when spread
    /// evenly (or more to the sides).
    pub width: f32,
    /// Where the light sits from bottom to top, from 0 to 1.
    pub height: f32,
    /// Share of lit voxels, from 0 to 1.
    pub density: f32,
    /// Relative change of the light per display frame (40 ms): 0 when still, around 0.01
    /// to 0.1 for slow motion, more for fast or flickering content.
    pub activity: f32,
    /// Shares of the light at 12 OKLCh hues 30° apart, from 0°, weighted by chroma and
    /// luminance, each color split between the two nearest: they sum to 1 for colored
    /// light, less as it turns gray, and 0 for gray light. Neighbouring hues lie a fifth
    /// apart, as they do in the harmony.
    pub hues: [f32; HUES],
}

impl Default for Features {
    /// A dark image: neutral, with the light (if any) spread evenly.
    fn default() -> Self {
        Self {
            level: 0.0,
            balance: 0.0,
            width: 1.0,
            height: 0.5,
            density: 0.0,
            activity: 0.0,
            hues: [0.0; HUES],
        }
    }
}

impl Features {
    /// Neutral light, which the sound leaves as it is: gray, half the voxels lit, centred
    /// and spread evenly, with activity in the middle of the range the sound follows.
    pub const NEUTRAL: Self = Self {
        level: 0.25,
        balance: 0.0,
        width: 1.0,
        height: 0.5,
        density: 0.5,
        activity: 0.03,
        hues: [0.0; HUES],
    };

    /// How bright the lit voxels are, from 0 to 1: the level over the share of lit voxels,
    /// which Void leaves alone (lit voxels keep their brightness); 0 when nothing is lit.
    pub fn glow(&self) -> f32 {
        if self.density > 0.01 {
            (self.level / self.density).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// How sparse the light is, from 0 with half the voxels lit or more to 1 when all are
    /// dark: what instruments answer as a complement, filling the space.
    pub fn sparseness(&self) -> f32 {
        ((0.5 - self.density) * 2.0).clamp(0.0, 1.0)
    }

    /// `self` moved towards `other` by `amount`, from 0 (itself) to 1 (`other`).
    pub fn towards(&self, other: &Self, amount: f32) -> Self {
        let lerp = |a: f32, b: f32| a + (b - a) * amount;
        Self {
            level: lerp(self.level, other.level),
            balance: lerp(self.balance, other.balance),
            width: lerp(self.width, other.width),
            height: lerp(self.height, other.height),
            density: lerp(self.density, other.density),
            activity: lerp(self.activity, other.activity),
            hues: std::array::from_fn(|hue| lerp(self.hues[hue], other.hues[hue])),
        }
    }

    /// Whether every feature is a number.
    pub fn is_finite(&self) -> bool {
        [
            self.level,
            self.balance,
            self.width,
            self.height,
            self.density,
            self.activity,
        ]
        .iter()
        .chain(&self.hues)
        .all(|value| value.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glow_is_the_brightness_of_what_is_lit() {
        let light = |level, density| Features {
            level,
            density,
            ..Features::NEUTRAL
        };
        assert_eq!(Features::NEUTRAL.glow(), 0.5);
        // Void by rank darkens voxels, not what stays lit.
        assert_eq!(light(0.1, 0.2).glow(), 0.5);
        assert_eq!(light(0.0, 0.0).glow(), 0.0);
        assert_eq!(Features::NEUTRAL.sparseness(), 0.0);
        assert_eq!(
            (light(0.0, 0.25).sparseness(), light(0.0, 0.0).sparseness()),
            (0.5, 1.0)
        );
        let towards = Features::default().towards(&Features::NEUTRAL, 0.5);
        assert_eq!((towards.density, towards.height), (0.25, 0.5));
    }
}
