//! The universal controls, as the user sets them.

use crate::PALETTES;

/// The universal controls, in their order, as the user set them (see the README). Every
/// value is from 0 to 1, except the palette, a position in `PALETTES`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub brightness: f32,
    pub volume: f32,
    pub palette: usize,
    pub heat: f32,
    pub flow: f32,
    pub form: f32,
    pub void: f32,
}

impl Controls {
    /// Within range and finite: values clamped to [0, 1], anything not finite 0, and the
    /// palette one of `PALETTES`.
    pub fn sanitized(self) -> Self {
        fn level(value: f32) -> f32 {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        Self {
            brightness: level(self.brightness),
            volume: level(self.volume),
            palette: self.palette.min(PALETTES.len() - 1),
            heat: level(self.heat),
            flow: level(self.flow),
            form: level(self.form),
            void: level(self.void),
        }
    }
}

impl Default for Controls {
    /// Every control in the middle, Volume muted, the first palette.
    fn default() -> Self {
        Self {
            brightness: 0.5,
            volume: 0.0,
            palette: 0,
            heat: 0.5,
            flow: 0.5,
            form: 0.5,
            void: 0.5,
        }
    }
}
