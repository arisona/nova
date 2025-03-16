use actix_files as fs;
use actix_web::web::Data;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use std::sync::{Arc, Mutex};
use std::thread;

use super::app_state::AppState;

#[get("/api/get-state")]
async fn get_state(data: web::Data<Arc<Mutex<AppState>>>) -> impl Responder {
    println!("get_state");
    let state = data.lock().unwrap();

    // do not expose all fields to client
    HttpResponse::Ok().json(serde_json::json!({
        "available-content": state.available_content,
        "enabled-content-indices": state.enabled_content_indices,
        "selected-content-index": state.selected_content_index,
        "hue": state.hue,
        "saturation": state.saturation,
        "brightness": state.brightness,
        "speed": state.speed,
        "flip-vertical": state.flip_vertical,
        "cycle-duration": state.cycle_duration,
        "ethernet-interface": state.ethernet_interface,
        "module0-address": state.module0_address,
    }))
}

#[get("/api/get-status")]
async fn get_status(data: web::Data<Arc<Mutex<AppState>>>) -> impl Responder {
    println!("get_status");
    let state = data.lock().unwrap();
    HttpResponse::Ok().json(serde_json::json!({
        "status-ok": state.status_ok,
        "status-message": state.status_message,
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
        println!("command: {command} value: {:?}", value);
        match command.as_str() {
            // TODO: missing content commands
            "available-content" => {}
            "enabled-content-indices" => {}
            "selected-content-index" => {
                state.selected_content_index = value.parse().unwrap_or(state.selected_content_index)
            }
            "hue" => state.hue = value.parse().unwrap_or(state.hue),
            "saturation" => state.saturation = value.parse().unwrap_or(state.saturation),
            "brightness" => state.brightness = value.parse().unwrap_or(state.brightness),
            "speed" => state.speed = value.parse().unwrap_or(state.speed),
            "flip-vertical" => state.flip_vertical = value.parse().unwrap_or(state.flip_vertical),
            "cycle-duration" => {
                state.cycle_duration = value.parse().unwrap_or(state.cycle_duration)
            }
            "ethernet-interface" => state.ethernet_interface = value.to_string(),
            "module0-address" => state.module0_address = value.to_string(),
            _ => {
                return HttpResponse::NotFound();
            }
        }
    } else {
        println!("command: {command}");
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

pub fn run_server(state: Arc<Mutex<super::app_state::AppState>>) {
    // we are running the web server in a separate thread, so we can still use the main thread for the simulator
    thread::spawn(|| {
        let sys = actix_web::rt::System::new();
        let port = state.lock().unwrap().port;
        let address = format!("0.0.0.0:{port}");

        println!("Starting web server at http://localhost:{port}/");

        let server = HttpServer::new(move || {
            App::new()
                .app_data(Data::new(state.clone()))
                .service(get_state)
                .service(get_status)
                .service(command)
                .service(fs::Files::new("/", "./src/web_client").index_file("index.html"))
        })
        .bind(address)
        .expect("Failed to bind address {address}")
        .run();
        sys.block_on(server).expect("Failed to run server");
    });
}
