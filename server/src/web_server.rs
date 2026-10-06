use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use actix_web::web::Data;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use include_dir::{Dir, include_dir};

use crate::app_state::{AppState, Status};
use crate::palettes::PALETTES;

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

/// Starts the web server on its own thread and returns.
pub fn start(state: Arc<Mutex<AppState>>) {
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
        // Otherwise SIGTERM (e.g. systemctl stop) only stops the web server, not Nova.
        .disable_signals()
        .bind(&address)
        .unwrap_or_else(|error| panic!("Failed to bind address {address}: {error}"))
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
        "brightness": state.brightness(),
        // -1 tells the web app that audio is disabled or failing, so Volume is hidden.
        "volume": if state.audio_available() { state.volume() } else { -1.0 },
        "palettes": palettes_json(),
        "palette": state.palette(),
        "heat": state.heat(),
        "flow": state.flow(),
        "form": state.form(),
        "void": state.void(),
        "flip-vertical": state.flip_vertical(),
        // -1 tells the web app that a layout of several modules is not configurable there.
        "module0-address": state.module0_address().map_or(-1, i32::from),
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
        Status::Ok(message) => (true, message),
        Status::Err(message) => (false, message),
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

/// Whether a change may come from this request. Browsers send `Sec-Fetch-Site` (or, in
/// older versions, `Origin`), so a page on another site cannot change settings through a
/// visitor's browser (cross-site request forgery). The host must be local, so such a page
/// cannot pose as this server under its own domain either (DNS rebinding). Clients other
/// than browsers, such as curl, send none of these headers and are allowed.
fn is_trusted(request: &HttpRequest) -> bool {
    let header = |name: &str| {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    let host = header("host");
    if host.is_some_and(|host| !is_local_host(host)) {
        return false;
    }
    match (header("sec-fetch-site"), header("origin")) {
        (Some(site), _) => site == "same-origin",
        (None, Some(origin)) => origin
            .split_once("://")
            .is_some_and(|(_, origin_host)| Some(origin_host) == host),
        (None, None) => true,
    }
}

/// Whether a `Host` header, with or without port, names a machine on the local network:
/// an IP address, a name without dots such as `localhost`, or a name under a domain
/// reserved for local networks. Anyone can point a public name at a local address.
fn is_local_host(host: &str) -> bool {
    if let Some(bracketed) = host.strip_prefix('[') {
        return bracketed
            .split_once(']')
            .is_some_and(|(address, _)| address.parse::<Ipv6Addr>().is_ok());
    }
    let name = host.split_once(':').map_or(host, |(name, _)| name);
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    !name.is_empty()
        && (name.parse::<Ipv4Addr>().is_ok()
            || !name.contains('.')
            || [".local", ".lan", ".home.arpa", ".internal", ".localhost"]
                .iter()
                .any(|suffix| name.ends_with(suffix)))
}

#[post("/api/{command}")]
async fn command(
    request: HttpRequest,
    data: web::Data<Arc<Mutex<AppState>>>,
    pending_save: web::Data<PendingSave>,
    command: web::Path<String>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    if !is_trusted(&request) {
        log::warn!("Rejected untrusted request to /api/{command}");
        return HttpResponse::Forbidden();
    }
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
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                state.set_brightness(parsed_value);
            }
            "volume" => {
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                state.set_volume(parsed_value);
            }
            "heat" => {
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                state.set_heat(parsed_value);
            }
            "flow" => {
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                state.set_flow(parsed_value);
            }
            "form" => {
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                state.set_form(parsed_value);
            }
            "void" => {
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                state.set_void(parsed_value);
            }
            "flip-vertical" => {
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                state.set_flip_vertical(parsed_value);
            }
            "module0-address" => {
                let Ok(parsed_value) = value.parse() else {
                    return HttpResponse::BadRequest();
                };
                if !state.set_module0_address(parsed_value) {
                    return HttpResponse::BadRequest();
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
                state.restore_defaults();
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
        let request = actix_web::test::TestRequest::post()
            .uri("/api/reset")
            .to_request();
        let response = actix_web::test::call_service(&app, request).await;
        assert!(response.status().is_success());
        assert!(state.lock().unwrap().take_hardware_reset_request());
        assert!(!state.lock().unwrap().take_hardware_reset_request());
        assert!(pending_save.lock().unwrap().is_none());
    }

    #[actix_web::test]
    async fn unknown_names_and_invalid_values_are_rejected_without_changes() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
                .app_data(Data::new(PendingSave::default()))
                .service(command),
        )
        .await;
        let status = |uri: String| {
            let request = actix_web::test::TestRequest::post().uri(&uri).to_request();
            actix_web::test::call_service(&app, request)
        };
        for uri in [
            "/api/palette?value=Renamed",
            "/api/selected-content?value=Renamed",
            "/api/enabled-content?value=Renamed",
            "/api/enabled-content?value=",
            "/api/heat?value=warm",
            "/api/flip-vertical?value=1",
            "/api/module0-address?value=256",
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
    async fn changes_from_other_sites_and_hosts_are_rejected() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
                .app_data(Data::new(PendingSave::default()))
                .service(command),
        )
        .await;
        let status = |headers: &[(&'static str, &'static str)]| {
            let mut request = actix_web::test::TestRequest::post().uri("/api/heat?value=0.9");
            for &header in headers {
                request = request.insert_header(header);
            }
            actix_web::test::call_service(&app, request.to_request())
        };
        for headers in [
            &[
                ("host", "nova.local:8080"),
                ("sec-fetch-site", "cross-site"),
            ][..],
            &[("host", "nova.local:8080"), ("sec-fetch-site", "same-site")],
            &[
                ("host", "nova.local:8080"),
                ("origin", "http://example.com"),
            ],
            &[
                ("host", "rebind.example.com:8080"),
                ("sec-fetch-site", "same-origin"),
            ],
        ] {
            assert_eq!(status(headers).await.status(), 403, "{headers:?}");
        }
        assert_eq!(state.lock().unwrap().heat(), 0.5);
        for headers in [
            &[
                ("host", "nova.local:8080"),
                ("sec-fetch-site", "same-origin"),
            ][..],
            &[
                ("host", "192.168.1.20:8080"),
                ("origin", "http://192.168.1.20:8080"),
            ],
            &[("host", "localhost:8080")],
            &[],
        ] {
            assert!(status(headers).await.status().is_success(), "{headers:?}");
        }
        assert_eq!(state.lock().unwrap().heat(), 0.9);
    }

    #[test]
    fn local_hosts() {
        for host in [
            "localhost",
            "localhost:8080",
            "nova",
            "nova.local",
            "Nova.Local.:80",
            "nova.lan",
            "nova.home.arpa",
            "192.168.1.20:8080",
            "[::1]:8080",
            "[fe80::1]",
        ] {
            assert!(is_local_host(host), "{host}");
        }
        for host in [
            "",
            ":8080",
            "example.com",
            "nova.local.example.com:8080",
            "192.168.1.20.nip.io",
            "[nova.local]",
        ] {
            assert!(!is_local_host(host), "{host}");
        }
    }

    #[actix_web::test]
    async fn ethernet_interface_cannot_be_set() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
                .app_data(Data::new(PendingSave::default()))
                .service(command),
        )
        .await;
        let request = actix_web::test::TestRequest::post()
            .uri("/api/ethernet-interface?value=wlan0")
            .to_request();
        let response = actix_web::test::call_service(&app, request).await;
        assert_eq!(response.status(), 404);
        assert_eq!(state.lock().unwrap().ethernet_interface(), "eth0");
    }

    #[actix_web::test]
    async fn web_app_routes_serve_the_web_app() {
        let app = actix_web::test::init_service(App::new().service(files)).await;
        let index = WWW_DIR.get_file("index.html").unwrap().contents();
        for (uri, status) in [
            ("/", 200),
            ("/settings", 200),
            ("/api/heat?value=0.5", 404),
            ("/assets/missing.js", 404),
        ] {
            let request = actix_web::test::TestRequest::get().uri(uri).to_request();
            let response = actix_web::test::call_service(&app, request).await;
            assert_eq!(response.status(), status, "{uri}");
            if status == 200 {
                assert_eq!(actix_web::test::read_body(response).await, index, "{uri}");
            }
        }
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
        let mut state: AppState =
            serde_json::from_value(serde_json::json!({ "audio": true })).unwrap();
        state.set_volume(0.42);
        state.set_status(Status::Ok("Display ready".into()));
        let state = Arc::new(Mutex::new(state));
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::clone(&state)))
                .service(get_state)
                .service(get_status),
        )
        .await;
        let get = |uri: &str| {
            let request = actix_web::test::TestRequest::get().uri(uri).to_request();
            actix_web::test::call_and_read_body_json::<_, _, serde_json::Value>(&app, request)
        };
        assert_eq!(
            get("/api/get-state").await["volume"],
            serde_json::json!(0.42_f32)
        );

        // A failing output hides Volume but keeps the saved value.
        state
            .lock()
            .unwrap()
            .set_audio_status(Status::Err("Audio unavailable".into()));
        assert_eq!(get("/api/get-state").await["volume"], -1.0);
        assert_eq!(state.lock().unwrap().volume(), 0.42);
        let response = get("/api/get-status").await;
        assert_eq!(response["status-ok"], true);
        assert_eq!(response["audio-ok"], false);
        assert_eq!(response["audio-message"], "Audio unavailable");
    }

    #[actix_web::test]
    async fn volume_is_reported_as_minus_one_when_audio_is_disabled() {
        let app = actix_web::test::init_service(
            App::new()
                .app_data(Data::new(Arc::new(Mutex::new(AppState::default()))))
                .service(get_state),
        )
        .await;
        let request = actix_web::test::TestRequest::get()
            .uri("/api/get-state")
            .to_request();
        let response: serde_json::Value =
            actix_web::test::call_and_read_body_json(&app, request).await;
        assert_eq!(response["volume"], -1.0);
        assert!(response.get("audio-enabled").is_none());
    }
}

#[get("/{path:.*}")]
async fn files(path: web::Path<String>) -> impl Responder {
    let path = match path.as_str() {
        "" => "index.html",
        // Web app routes such as `settings` are pages of index.html, so reloading them works.
        route if !route.starts_with("api/") && !route.contains('.') => "index.html",
        file => file,
    };
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
