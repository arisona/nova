//! Nova, the host: runs the light and the sound with the settings and the web app. The
//! parts meet on the bus in the state; the display asks for its settings through `voxel.rs`.

use std::sync::{Arc, Mutex};

mod state;
mod voxel;
mod web;

use nova_audio::Output;
use nova_core::{Part, Status};
use nova_voxel::{Hardware, Renderer, Simulator};
use state::State;
use voxel::Voxel;
use web::WebServer;

fn main() {
    let env = env_logger::Env::default().default_filter_or("debug,actix_server=warn");
    env_logger::Builder::from_env(env).init();

    let state = Arc::new(Mutex::new(State::load()));

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

    let (bus, simulator, audio, dim) = {
        let state = state.lock().unwrap();
        log::debug!("Using settings:\n{:#?}", *state);
        (state.bus(), state.simulator(), state.audio(), state.dim())
    };
    log::info!(
        "Starting Nova in {} mode",
        if simulator { "simulator" } else { "hardware" }
    );

    // The sound plays while this lives.
    let _sound = if audio {
        Output::start(Arc::clone(&bus))
            .inspect_err(|error| {
                log::error!("Cannot start the sound: {error}");
                bus.report(
                    Part::Sound,
                    Status::Err(format!("Audio service unavailable: {error}")),
                );
            })
            .ok()
    } else {
        None
    };

    let _web_server = WebServer::start(Arc::clone(&state)).expect("Cannot start the web server");

    let renderer = Renderer::new(dim, bus);
    let host = Box::new(Voxel::new(state));
    if simulator {
        Simulator::run(renderer, host);
    } else {
        Hardware::run(renderer, host);
    }
}
