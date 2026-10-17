//! Voices: the sound sources and effects the instruments are built from. Each plays from the
//! smoothed parameters and, where it needs to, the signals; none knows the others.

pub mod air;
pub mod bed;
pub mod dub_delay;
pub mod reverb;
pub mod stab;
pub mod sub;
pub mod thump;
pub mod tick;

pub use air::Air;
pub use bed::Bed;
pub use dub_delay::DubDelay;
pub use reverb::Reverb;
pub use stab::Stab;
pub use sub::Sub;
pub use thump::Thump;
pub use tick::Tick;
