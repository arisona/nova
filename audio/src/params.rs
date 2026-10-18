//! The smoothed controls and features of the light the conductor and the voices read once
//! per block.

use nova_core::Features;

use crate::dsp::Curve;

/// The full rhythm, on the plateau of `RHYTHM` (see `Params::rhythm`): what Form 0.8 played
/// before Void took over the rhythm; the densest steps are left out.
pub const FULL: f32 = 0.8;

/// How much rhythm plays over Void: none at 0, full from 0.1 to 0.5, dissolving towards 1.
/// Rhythm is void in time: without gaps the sound is one mass, with some it falls into
/// pulses, with mostly gaps into isolated, distant events.
pub const RHYTHM: Curve = Curve(&[[0.0, 0.0], [0.1, FULL], [0.5, FULL], [1.0, 0.0]]);

/// How far the rhythm has dissolved over Void, from 0 to 1.
pub const DISSOLVE: Curve = Curve(&[[0.5, 0.0], [1.0, 1.0]]);

/// Smoothed controls and features, handed to the conductor and the voices once per block.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub heat: f32,
    pub flow: f32,
    pub form: f32,
    pub void: f32,
    pub flow_setting: f32,
    /// How much of the score plays, from 0 to 1, from Void (`RHYTHM`): a part comes in at
    /// the rhythm its score gives, and the amounts that come with the pulse follow it.
    pub rhythm: f32,
    /// How far the rhythm has dissolved, from Void (`DISSOLVE`): it loosens, and stabs move
    /// into their echoes.
    pub dissolve: f32,
    /// Visual activity, mapped to [0, 1].
    pub activity: f32,
    /// The features, smoothed over seconds; voices take the activity from `activity`.
    pub image: Features,
}

impl Params {
    /// Sets the rhythm and how far it has dissolved from Void.
    pub fn follow_void(&mut self) {
        self.rhythm = RHYTHM.at(self.void);
        self.dissolve = DISSOLVE.at(self.void);
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            heat: 0.5,
            flow: 0.5,
            form: 0.5,
            void: 0.5,
            flow_setting: 0.5,
            rhythm: FULL,
            dissolve: 0.0,
            activity: 0.0,
            image: Features::default(),
        }
    }
}
