use actix_web::web::Data;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use include_dir::{Dir, include_dir};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::app_state::{AppState, Status};

pub fn run_server(state: Arc<Mutex<super::app_state::AppState>>) {
    // we are running the web server in a separate thread, so we can still use the main thread for the simulator
    thread::spawn(move || {
        let sys = actix_web::rt::System::new();
        let port = state.lock().unwrap().webserver_port();
        let address = format!("0.0.0.0:{port}");

        log::info!("Starting web server at http://localhost:{port}/");

        let server = HttpServer::new(move || {
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
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
        "enabled-content-indices": state.enabled_content_indices(),
        "selected-content-index": state.selected_content_index(),
        "audio-enabled": crate::ENABLE_AUDIO,
        "brightness": state.brightness(),
        "volume": state.volume(),
        "tone": state.tone(),
        "heat": state.heat(),
        "flow": state.flow(),
        "form": state.form(),
        "flip-vertical": state.is_flip_vertical(),
        "ethernet-interface": state.ethernet_interface(),
        "module0-address": state.module0_address(),
    }))
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
    command: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let mut state = data.lock().unwrap();
    if let Some(value) = query.get("value") {
        log::debug!("command: {command} value: {value}");
        match command.as_str() {
            "enabled-content-indices" => {
                let enabled_indices: Vec<u32> =
                    value.split(',').filter_map(|s| s.parse().ok()).collect();
                state.set_enabled_content_indices(enabled_indices);
            }
            "selected-content-index" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_selected_content_index(parsed_value);
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
            "tone" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_tone(parsed_value);
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
                // TODO: request hw reset
            }
            "reload" => {
                // TODO: server exit and relaunch
            }
            _ => {
                return HttpResponse::NotFound();
            }
        }
    }
    state.save();
    HttpResponse::Ok()
}

static WWW_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/www");

#[cfg(test)]
mod tests {
    use super::*;

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
