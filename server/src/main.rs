use std::sync::{Arc, Mutex};

mod app_state;
mod audio;
mod calibration;
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

    let state = Arc::new(Mutex::new(app_state::AppState::load()));

    // Ctrl+C, SIGTERM (e.g. systemctl stop) and SIGHUP let the hardware loop reset the
    // modules before exiting, so the display stays dark. A second signal exits right away.
    let signal_state = Arc::clone(&state);
    ctrlc::set_handler(move || {
        if signal_state
            .lock()
            .is_ok_and(|mut state| state.request_shutdown())
        {
            log::info!("Signal received, resetting modules before exiting.");
        } else {
            log::info!("Signal received, exiting.");
            std::process::exit(0);
        }
    })
    .expect("Error setting signal handler");
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
