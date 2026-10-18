//! What the parts of Nova share: the bus they meet on, the universal controls, the palettes,
//! the tides, the features that describe the light, and the status a part reports. The
//! voxel display (`nova-voxel`), the sound (`nova-audio`) and the host build on it; none of
//! them owns it.

mod bus;
mod controls;
mod features;
pub mod palettes;
pub mod tides;

pub use bus::{Bus, Part, Status};
pub use controls::Controls;
pub use features::{Features, HUES};
pub use palettes::{PALETTES, Palette};
