//! The host, as the voxel display sees it: the settings of every frame, from the state, and
//! the requests to reset the modules or to shut down. Controls and statuses go over the bus.

use std::sync::{Arc, Mutex};

use crate::state::State;

/// What the display shows next, from the state.
pub fn settings(state: &State) -> nova_voxel::Settings {
    nova_voxel::Settings {
        content: state.selected_content_index(),
        pattern: state.calibration_pattern(),
        flip_vertical: state.flip_vertical(),
        calibration: state.calibration(),
        interface: state.ethernet_interface().to_string(),
        modules: state.modules().to_vec(),
    }
}

pub struct Voxel {
    state: Arc<Mutex<State>>,
}

impl Voxel {
    pub fn new(state: Arc<Mutex<State>>) -> Self {
        Self { state }
    }
}

impl nova_voxel::Host for Voxel {
    fn settings(&mut self) -> nova_voxel::Settings {
        // Locked only briefly, so the web server never starves.
        settings(&self.state.lock().unwrap())
    }

    fn take_reset(&mut self) -> bool {
        self.state.lock().unwrap().take_hardware_reset_request()
    }

    fn shutdown_requested(&mut self) -> bool {
        self.state.lock().unwrap().shutdown_requested()
    }
}
