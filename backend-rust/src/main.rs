use std::sync::{Arc, Mutex};

mod app_state;
mod content;
mod hardware;
mod renderer;
mod simulator;
mod web_server;

#[cfg(feature = "use_simulator")]
use simulator::simulator::window_conf;

#[cfg(feature = "use_simulator")]
#[macroquad::main(window_conf)]
async fn main() {
    let state = Arc::new(Mutex::new(app_state::AppState::load()));
    //println!("Using settings:\n{:#?}", *state.lock().unwrap());

    web_server::run_server(state.clone());

    let renderer = Arc::new(renderer::renderer::Renderer::new());
    simulator::simulator::run_simulator(state.clone(), renderer.clone()).await;
}

#[cfg(not(feature = "use_simulator"))]
fn main() {
    let state = Arc::new(Mutex::new(app_state::AppState::load()));
    //println!("Using settings:\n{:#?}", *state.lock().unwrap());

    web_server::run_server(state.clone());

    let renderer = Arc::new(renderer::renderer::Renderer::new());
    hardware::nova::run_nova_hardware(state.clone(), renderer.clone());
}
