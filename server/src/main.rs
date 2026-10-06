use std::sync::{Arc, Mutex};

mod app_state;
mod audio;
mod content;
mod ethernet;
mod macros;
mod nova;
mod palettes;
mod renderer;
mod simulator;
mod tides;
mod voxel_image;
mod web_server;

fn main() {
    let env = env_logger::Env::default().default_filter_or("debug,actix_server=warn");
    env_logger::Builder::from_env(env).init();

    ctrlc::set_handler(|| {
        log::info!("Interrupt received, exiting.");
        std::process::exit(0);
    })
    .expect("Error setting signal handler");

    let state = Arc::new(Mutex::new(app_state::AppState::load()));
    log::debug!("Using settings:\n{:#?}", *state.lock().unwrap());
    let (run_simulator, start_audio) = {
        let state = state.lock().unwrap();
        (state.simulator(), state.audio())
    };

    log::info!(
        "Starting Nova server in {} mode",
        if run_simulator {
            "simulator"
        } else {
            "hardware"
        }
    );

    let _audio = if start_audio {
        audio::AudioService::start(Arc::clone(&state))
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

    web_server::start(Arc::clone(&state));

    let renderer = renderer::Renderer::new(state.lock().unwrap().dim());
    if run_simulator {
        simulator::run(Arc::clone(&state), renderer);
    } else {
        nova::run(Arc::clone(&state), renderer);
    }
}
