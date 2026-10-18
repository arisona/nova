//! Voices: the sound sources and effects the instruments are built from. Each plays from the
//! smoothed parameters and, where it needs to, the signals; none knows the others.

pub mod air;
pub mod bed;
pub mod bell;
pub mod dub_delay;
pub mod organ;
pub mod pluck;
pub mod reverb;
pub mod stab;
pub mod sub;
pub mod thump;
pub mod tick;

pub use air::Air;
pub use bed::Bed;
pub use bell::{Bell, Strike};
pub use dub_delay::DubDelay;
pub use organ::Organ;
pub use pluck::{Pick, Pluck};
pub use reverb::Reverb;
pub use stab::Stab;
pub use sub::Sub;
pub use thump::Thump;
pub use tick::Tick;
