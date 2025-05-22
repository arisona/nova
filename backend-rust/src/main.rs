use std::sync::{Arc, Mutex};

mod app_state;
mod content;
mod ethernet;
mod nova;
mod renderer;
mod simulator;
mod voxel_image;
mod web_server;

#[macro_use]
mod macros;

const USE_NOVA_HARDWARE: bool = true;
fn main() {
    let state = Arc::new(Mutex::new(app_state::AppState::load()));
    println!("Using settings:\n{:#?}", *state.lock().unwrap());

    web_server::run_server(Arc::clone(&state));

    let renderer = renderer::Renderer::new(state.lock().unwrap().dim());
    if USE_NOVA_HARDWARE {
        nova::run_nova_hardware(Arc::clone(&state), renderer);
    } else {
        simulator::run_simulator(Arc::clone(&state), renderer);
    }
}
