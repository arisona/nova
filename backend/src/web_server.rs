use actix_files as fs;
use actix_web::web::Data;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::app_state::AppState;

pub fn run_server(state: Arc<Mutex<super::app_state::AppState>>) {
    // we are running the web server in a separate thread, so we can still use the main thread for the simulator
    thread::spawn(move || {
        let sys = actix_web::rt::System::new();
        let port = state.lock().unwrap().webserver_port();
        let address = format!("0.0.0.0:{port}");

        println!("Starting web server at http://localhost:{port}/");

        let server = HttpServer::new(move || {
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
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

#[get("/api/get-state")]
async fn get_state(data: web::Data<Arc<Mutex<AppState>>>) -> impl Responder {
    println!("get_state");
    let state = data.lock().unwrap();

    // do not expose all fields to client
    HttpResponse::Ok().json(serde_json::json!({
        "available-content": state.available_content(),
        "enabled-content-indices": state.enabled_content_indices(),
        "selected-content-index": state.selected_content_index(),
        "hue": state.hue(),
        "saturation": state.saturation(),
        "brightness": state.brightness(),
        "speed": state.speed(),
        "flip-vertical": state.is_flip_vertical(),
        "cycle-duration": state.cycle_duration(),
        "ethernet-interface": state.ethernet_interface(),
        "module0-address": state.module0_address(),
    }))
}

#[get("/api/get-status")]
async fn get_status(data: web::Data<Arc<Mutex<AppState>>>) -> impl Responder {
    //println!("get_status");
    let state = data.lock().unwrap();
    HttpResponse::Ok().json(serde_json::json!({
        "status-ok": state.status().0,
        "status-message": state.status().1,
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
            "hue" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_hue(parsed_value);
                }
            }
            "saturation" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_saturation(parsed_value);
                }
            }
            "brightness" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_brightness(parsed_value);
                }
            }
            "speed" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_speed(parsed_value);
                }
            }
            "flip-vertical" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_flip_vertical(parsed_value);
                }
            }
            "cycle-duration" => {
                if let Ok(parsed_value) = value.parse() {
                    state.set_cycle_duration(parsed_value);
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
