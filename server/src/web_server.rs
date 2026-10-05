use actix_web::web::Data;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use include_dir::{Dir, include_dir};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::app_state::{AppState, Status};
use crate::content::palettes::PALETTES;

type PendingSave = Mutex<Option<actix_web::rt::task::JoinHandle<()>>>;

fn debounce_save(pending: &PendingSave, save: impl FnOnce() + 'static) {
    let mut pending = pending.lock().unwrap();
    if let Some(task) = pending.take() {
        task.abort();
    }
    *pending = Some(actix_web::rt::spawn(async move {
        actix_web::rt::time::sleep(Duration::from_millis(500)).await;
        save();
    }));
}

pub fn run_server(state: Arc<Mutex<super::app_state::AppState>>) {
    // we are running the web server in a separate thread, so we can still use the main thread for the simulator
    thread::spawn(move || {
        let sys = actix_web::rt::System::new();
        let port = state.lock().unwrap().webserver_port();
        let address = format!("0.0.0.0:{port}");

        log::info!("Starting web server at http://localhost:{port}/");

        let pending_save = Data::new(PendingSave::default());
        let server = HttpServer::new(move || {
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
                .app_data(pending_save.clone())
                .service(get_state)
                .service(get_status)
                .service(command)
                .service(files)
        })
        .bind(address)
        .expect("Failed to bind address {address}")
        .run();
        sys.block_on(server).expect("Failed to run server");
    });
}

#[get("/api/get-state")]
async fn get_state(data: web::Data<Arc<Mutex<AppState>>>) -> impl Responder {
    log::debug!("get_state");
    let state = data.lock().unwrap();

    // do not expose all fields to client
    HttpResponse::Ok().json(serde_json::json!({
        "available-content": state.available_content(),
        "enabled-content": state.enabled_content(),
        "selected-content": state.selected_content(),
        "audio-enabled": crate::ENABLE_AUDIO,
        "brightness": state.brightness(),
        "volume": state.volume(),
        "palettes": palettes_json(),
        "palette": state.palette(),
        "heat": state.heat(),
        "flow": state.flow(),
        "form": state.form(),
        "void": state.void(),
        "flip-vertical": state.is_flip_vertical(),
        "ethernet-interface": state.ethernet_interface(),
        "module0-address": state.module0_address(),
    }))
}

/// All palettes in display order, with CSS-ready colors.
fn palettes_json() -> serde_json::Value {
    PALETTES
        .iter()
        .map(|palette| {
            serde_json::json!({
                "name": palette.name,
                "colors": palette
                    .colors
                    .iter()
                    .map(|color| {
                        serde_json::json!({
                            "name": color.name,
                            "code": color.code,
                            "hex": color.hex(),
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect()
}

#[get("/api/get-status")]
async fn get_status(data: web::Data<Arc<Mutex<AppState>>>) -> impl Responder {
    let state = data.lock().unwrap();
    let (ok, message): (bool, &str) = match state.status() {
        Status::Ok(msg) => (true, msg.as_str()),
        Status::Err(msg) => (false, msg.as_str()),
        Status::Unknown => (false, ""),
    };
    let (audio_ok, audio_message): (bool, &str) = match state.audio_status() {
        Status::Ok(message) => (true, message),
        Status::Err(message) => (false, message),
        Status::Unknown => (false, ""),
    };
    HttpResponse::Ok().json(serde_json::json!({
        "status-ok": ok,
        "status-message": message,
        "audio-ok": audio_ok,
        "audio-message": audio_message,
    }))
}

#[get("/api/{command}")]
async fn command(
    data: web::Data<Arc<Mutex<AppState>>>,
    pending_save: web::Data<PendingSave>,
    command: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let mut state = data.lock().unwrap();
    if let Some(value) = query.get("value") {
        log::debug!("command: {command} value: {value}");
        match command.as_str() {
            // Unknown names are rejected so the web app reloads its state, e.g. after
            // content or a palette was renamed while the page was open.
            "enabled-content" => {
                let names: Vec<&str> = value.split(',').filter(|name| !name.is_empty()).collect();
                if names.is_empty() || !names.iter().all(|name| state.knows_content(name)) {
                    return HttpResponse::BadRequest();
                }
                state.set_enabled_content(&names);
            }
            "selected-content" => {
                if !state.set_selected_content(value) {
                    return HttpResponse::BadRequest();
                }
            }
            "palette" => {
                if !state.set_palette(value) {
                    return HttpResponse::BadRequest();
                }
            }
            "brightness" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_brightness(parsed_value);
                }
            }
            "volume" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_volume(parsed_value);
                }
            }
            "heat" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_heat(parsed_value);
                }
            }
            "flow" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_flow(parsed_value);
                }
            }
            "form" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_form(parsed_value);
                }
            }
            "void" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_void(parsed_value);
                }
            }
            "flip-vertical" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_flip_vertical(parsed_value);
                }
            }
            "ethernet-interface" => {
                state.set_ethernet_interface(value);
            }
            "module0-address" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_module0_address(parsed_value);
                }
            }
            _ => {
                return HttpResponse::NotFound();
            }
        }
    } else {
        log::debug!("command: {command}");
        match command.as_str() {
            "restore" => {
                // restore settings to default
                *state = AppState::default();
            }
            "reset" => {
                state.request_hardware_reset();
                return HttpResponse::Ok();
            }
            "reload" => {
                // save state and exit the application (rely on external process manager to restart)
                state.save();
                std::process::exit(0);
            }
            _ => {
                return HttpResponse::NotFound();
            }
        }
    }
    drop(state);
    debounce_save(&pending_save, move || data.lock().unwrap().save());
    HttpResponse::Ok()
}

static WWW_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/www");

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn reset_requests_hardware_reset_without_saving() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let pending_save = Data::new(PendingSave::default());
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
                .app_data(pending_save.clone())
                .service(command),
        )
        .await;
        let request = actix_web::test::TestRequest::get()
            .uri("/api/reset")
            .to_request();
        let response = actix_web::test::call_service(&app, request).await;
        assert!(response.status().is_success());
        assert!(state.lock().unwrap().take_hardware_reset_request());
        assert!(!state.lock().unwrap().take_hardware_reset_request());
        assert!(pending_save.lock().unwrap().is_none());
    }

    #[actix_web::test]
    async fn unknown_names_are_rejected_without_changes() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
                .app_data(Data::new(PendingSave::default()))
                .service(command),
        )
        .await;
        let status = |uri: String| {
            let request = actix_web::test::TestRequest::get().uri(&uri).to_request();
            actix_web::test::call_service(&app, request)
        };
        for uri in [
            "/api/palette?value=Renamed",
            "/api/selected-content?value=Renamed",
            "/api/enabled-content?value=Renamed",
            "/api/enabled-content?value=",
        ] {
            assert_eq!(status(uri.to_string()).await.status(), 400, "{uri}");
        }
        assert_eq!(state.lock().unwrap().palette(), PALETTES[0].name);
        let uri = format!(
            "/api/palette?value={}",
            PALETTES[1].name.replace(' ', "%20")
        );
        assert!(status(uri).await.status().is_success());
        assert_eq!(state.lock().unwrap().palette(), PALETTES[1].name);
    }

    #[actix_web::test]
    async fn save_is_debounced_by_500_ms() {
        let pending = PendingSave::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        let first_sender = sender.clone();
        debounce_save(&pending, move || first_sender.send(1).unwrap());
        actix_web::rt::time::sleep(Duration::from_millis(300)).await;
        assert!(receiver.try_recv().is_err());
        debounce_save(&pending, move || sender.send(2).unwrap());
        actix_web::rt::time::sleep(Duration::from_millis(300)).await;
        assert!(receiver.try_recv().is_err());
        actix_web::rt::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(receiver.try_recv().unwrap(), 2);
        assert!(receiver.try_recv().is_err());
    }

    #[actix_web::test]
    async fn audio_api_reports_volume_and_independent_status() {
        let mut state = AppState::default();
        state.set_volume(0.42);
        state.set_status(Status::Ok("Display ready".into()));
        state.set_audio_status(Status::Err("Audio unavailable".into()));
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::new(Mutex::new(state))))
                .service(get_state)
                .service(get_status),
        )
        .await;
        let request = actix_web::test::TestRequest::get()
            .uri("/api/get-state")
            .to_request();
        let response: serde_json::Value =
            actix_web::test::call_and_read_body_json(&app, request).await;
        assert_eq!(response["volume"], serde_json::json!(0.42_f32));
        assert_eq!(response["audio-enabled"], crate::ENABLE_AUDIO);
        let request = actix_web::test::TestRequest::get()
            .uri("/api/get-status")
            .to_request();
        let response: serde_json::Value =
            actix_web::test::call_and_read_body_json(&app, request).await;
        assert_eq!(response["status-ok"], true);
        assert_eq!(response["audio-ok"], false);
        assert_eq!(response["audio-message"], "Audio unavailable");
    }
}

#[get("/{path:.*}")]
async fn files(path: web::Path<String>) -> impl Responder {
    let path = path.as_str();
    let path = if path.is_empty() { "index.html" } else { path };
    log::debug!("files: {path}");

    match WWW_DIR.get_file(path) {
        Some(file) => {
            let body = file.contents();
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            HttpResponse::Ok().content_type(mime.as_ref()).body(body)
        }
        None => HttpResponse::NotFound().body("404 Not Found"),
    }
}
