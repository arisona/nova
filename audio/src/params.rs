//! The smoothed controls and features of the light the conductor and the voices read once
//! per block.

use nova_core::Features;

/// Smoothed controls and features, handed to the conductor and the voices once per block.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub heat: f32,
    pub flow: f32,
    pub form: f32,
    pub void: f32,
    pub flow_setting: f32,
    /// Visual activity, mapped to [0, 1].
    pub activity: f32,
    /// The features, smoothed over seconds; voices take the activity from `activity`.
    pub image: Features,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            heat: 0.5,
            flow: 0.5,
            form: 0.5,
            void: 0.5,
            flow_setting: 0.5,
            activity: 0.0,
            image: Features::default(),
        }
    }
}
