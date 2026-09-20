use std::sync::{Arc, Mutex};

mod app_state;
mod audio;
mod content;
mod ethernet;
mod nova;
mod renderer;
mod simulator;
mod voxel_image;
mod web_server;

#[macro_use]
mod macros;

const ENABLE_SIMULATOR: bool = true;
const ENABLE_AUDIO: bool = false;
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
        if ENABLE_SIMULATOR {
            "simulator"
        } else {
            "hardware"
        }
    );

    let state = Arc::new(Mutex::new(app_state::AppState::load()));
    log::debug!("Using settings:\n{:#?}", *state.lock().unwrap());

    let _audio = if ENABLE_AUDIO {
        audio::output::AudioService::start(Arc::clone(&state))
            .map_err(|error| {
                log::error!("Cannot start audio service: {error}");
                state
                    .lock()
                    .unwrap()
                    .set_audio_status(app_state::Status::Err(format!(
                        "Audio service unavailable: {error}"
                    )));
            })
            .ok()
    } else {
        None
    };

    web_server::run_server(Arc::clone(&state));

    let renderer = renderer::Renderer::new(state.lock().unwrap().dim());
    if ENABLE_SIMULATOR {
        simulator::run_simulator(Arc::clone(&state), renderer);
    } else {
        nova::run_nova_hardware(Arc::clone(&state), renderer);
    }
}
