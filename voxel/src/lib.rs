//! The light of the Nova voxel display: content modules, the renderer that applies the
//! controls and the tides, display calibration and test patterns, the features of the light
//! for the sound, and the two outputs, the hardware driver and the desktop simulator.
//!
//! It knows nothing of the host that runs it, nor of the sound: the renderer takes the
//! controls from the bus (`nova_core::Bus`) and puts the tide clock and the features of its
//! light back on it, and the hardware and the simulator ask their `Host` for the settings
//! of every frame and leave their status on the bus.

mod macros;

mod calibration;
mod content;
mod ethernet;
mod hardware;
mod host;
mod light;
#[cfg(test)]
mod palettes_overview;
mod renderer;
mod simulator;
mod sparkle;
mod voxel_image;

pub use calibration::{Calibration, Pattern, PatternSelection};
pub use content::all_content_names;
pub use hardware::Hardware;
pub use host::{Host, MODULE_GRID_MAX, MODULE_X_RES, MODULE_Y_RES, MODULE_Z_RES, Settings};
pub use renderer::Renderer;
pub use simulator::Simulator;
pub use voxel_image::VoxelImage;
