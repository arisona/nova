//! What the voxel display asks of the host that runs it: the hardware driver (`hardware.rs`)
//! and the simulator (`simulator.rs`) ask it for the settings of every frame and for its
//! requests. Controls, the tide clock, the light and the display's status go over the bus.

use crate::calibration::{Calibration, Pattern};

/// Voxels of a module along x, y and z (up).
pub const MODULE_X_RES: usize = 5;
pub const MODULE_Y_RES: usize = 5;
pub const MODULE_Z_RES: usize = 10;

/// Modules of a grid along x and along y, at most.
pub const MODULE_GRID_MAX: usize = 10;

/// What the display shows next and how, from the host; the controls come from the bus.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Position of the selected content module in `all_content()`.
    pub content: usize,
    /// A test pattern to show instead of the content, or `Pattern::Off`.
    pub pattern: Pattern,
    pub flip_vertical: bool,
    pub calibration: Calibration,
    /// The hardware's Ethernet interface, and its modules as grid position and address.
    pub interface: String,
    pub modules: Vec<(usize, usize, u8)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            content: 0,
            pattern: Pattern::Off,
            flip_vertical: false,
            calibration: Calibration::default(),
            interface: String::new(),
            modules: vec![(0, 0, 1)],
        }
    }
}

/// The host, as the display sees it; called on the display's thread.
pub trait Host {
    /// The settings for the next frame; asked once per frame.
    fn settings(&mut self) -> Settings;

    /// Whether the hardware should reset its modules; asking takes the request.
    fn take_reset(&mut self) -> bool {
        false
    }

    /// Whether the hardware should reset its modules, so the display stays dark, and stop.
    fn shutdown_requested(&mut self) -> bool {
        false
    }
}
