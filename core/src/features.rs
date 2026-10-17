//! The features that describe the light (see `doc/audio_concept.md`).

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
