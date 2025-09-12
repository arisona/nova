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

const USE_NOVA_HARDWARE: bool = false;
fn main() {
    let env = env_logger::Env::default().default_filter_or("debug,actix_server=warn");
    env_logger::Builder::from_env(env).init();

    ctrlc::set_handler(|| {
        log::info!("Interrupt received, exiting.");
        std::process::exit(0);
    })
    .expect("Error setting signal handler");

    log::info!(
        "Starting Nova server in {} mode",
        if USE_NOVA_HARDWARE {
            "hardware"
        } else {
            "simulator"
        }
    );

    let state = Arc::new(Mutex::new(app_state::AppState::load()));
    log::debug!("Using settings:\n{:#?}", *state.lock().unwrap());

    web_server::run_server(Arc::clone(&state));

    let renderer = renderer::Renderer::new(state.lock().unwrap().dim());
    if USE_NOVA_HARDWARE {
        nova::run_nova_hardware(Arc::clone(&state), renderer);
    } else {
        simulator::run_simulator(Arc::clone(&state), renderer);
    }
}
