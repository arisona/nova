use std::fs;

use serde::{Deserialize, Serialize};

use crate::content::content::get_all_content_names;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    enabled_content_indices: Vec<u32>,
    selected_content_index: usize,

    hue: f32,
    saturation: f32,
    brightness: f32,
    speed: f32,
    flip_vertical: bool,
    cycle_duration: f32,

    ethernet_interface: String,
    modules: Vec<(usize, usize, u8)>,
    webserver_port: u16,

    #[serde(skip_serializing, skip_deserializing)]
    available_content: Vec<String>,

    #[serde(skip_serializing, skip_deserializing)]
    dim: (usize, usize, usize),

    #[serde(skip_serializing, skip_deserializing)]
    status: (bool, String),
}

impl AppState {
    pub const MODULE_GRID_MAX: usize = 10;

    pub const MODULE_X_RES: usize = 5;
    pub const MODULE_Y_RES: usize = 5;
    pub const MODULE_Z_RES: usize = 10;

    pub const MODULE_DEFAULT_ADDRESS: u8 = 1;

    const SETTINGS_FILE: &str = "nova_settings.json";

    pub fn load() -> Self {
        let mut settings = Self::default();
        if let Ok(json_string) = fs::read_to_string(Self::SETTINGS_FILE) {
            if let Ok(parsed_settings) = serde_json::from_str::<AppState>(&json_string) {
                settings.set_selected_content_index(parsed_settings.selected_content_index);
                settings.set_enabled_content_indices(parsed_settings.enabled_content_indices);
                settings.set_hue(parsed_settings.hue);
                settings.set_saturation(parsed_settings.saturation);
                settings.set_brightness(parsed_settings.brightness);
                settings.set_speed(parsed_settings.speed);
                settings.set_flip_vertical(parsed_settings.flip_vertical);
                settings.set_cycle_duration(parsed_settings.cycle_duration);
                settings.set_ethernet_interface(&parsed_settings.ethernet_interface);

                {
                    let mut max_x = 0;
                    let mut max_y = 0;
                    let mut modules: Vec<(usize, usize, u8)> = Vec::new();
                    for module in parsed_settings.modules.iter() {
                        let x = module.0;
                        let y = module.1;
                        if x >= AppState::MODULE_GRID_MAX || y >= AppState::MODULE_GRID_MAX {
                            eprintln!(
                                "Module location out of bounds (max is {}), skipping module",
                                AppState::MODULE_GRID_MAX - 1
                            );
                            continue;
                        }
                        modules.push(*module);
                        max_x = max_x.max(module.0);
                        max_y = max_y.max(module.1);
                    }
                    if !modules.is_empty() {
                        settings.modules = modules;
                        settings.dim = (
                            (max_x + 1) * AppState::MODULE_X_RES,
                            (max_y + 1) * AppState::MODULE_Y_RES,
                            AppState::MODULE_Z_RES,
                        );
                    } else {
                        eprintln!("No valid modules found, using defaults");
                    }
                }

                settings.set_webserver_port(parsed_settings.webserver_port);
            } else {
                eprintln!("Failed to parse settings, using defaults");
            }
        } else {
            eprintln!("Failed to load settings, using defaults");
        }
        settings
    }

    pub fn save(&self) {
        if let Ok(json_string) = serde_json::to_string_pretty(self) {
            if let Err(e) = fs::write(Self::SETTINGS_FILE, json_string) {
                eprintln!("Failed to save settings: {}", e);
            }
        } else {
            eprintln!("Failed to serialize settings");
        }
    }

    pub fn enabled_content_indices(&self) -> &Vec<u32> {
        &self.enabled_content_indices
    }

    pub fn set_enabled_content_indices(&mut self, indices: Vec<u32>) {
        self.enabled_content_indices = indices;
    }

    pub fn selected_content_index(&self) -> usize {
        self.selected_content_index
    }

    pub fn set_selected_content_index(&mut self, index: usize) {
        self.selected_content_index = index;
    }

    pub fn hue(&self) -> f32 {
        self.hue
    }

    pub fn set_hue(&mut self, hue: f32) {
        self.hue = hue.clamp(0.0, 360.0);
    }

    pub fn saturation(&self) -> f32 {
        self.saturation
    }

    pub fn set_saturation(&mut self, saturation: f32) {
        self.saturation = saturation.clamp(0.0, 1.0);
    }

    pub fn brightness(&self) -> f32 {
        self.brightness
    }

    pub fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness.clamp(0.0, 1.0);
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.0, 1.0);
    }

    pub fn flip_vertical(&self) -> bool {
        self.flip_vertical
    }

    pub fn set_flip_vertical(&mut self, flip_vertical: bool) {
        self.flip_vertical = flip_vertical;
    }

    pub fn cycle_duration(&self) -> f32 {
        self.cycle_duration
    }

    pub fn set_cycle_duration(&mut self, cycle_duration: f32) {
        self.cycle_duration = cycle_duration.clamp(0.0, 3600.0);
    }

    pub fn ethernet_interface(&self) -> &str {
        &self.ethernet_interface
    }

    pub fn set_ethernet_interface(&mut self, ethernet_interface: &str) {
        self.ethernet_interface = if ethernet_interface.len() > 20 {
            ethernet_interface[..20].to_string()
        } else {
            ethernet_interface.to_string()
        };
    }

    pub fn module0_address(&self) -> u8 {
        self.modules[0].2
    }

    pub fn set_module0_address(&mut self, module0_address: u8) {
        self.modules[0].2 = module0_address;
    }

    pub fn webserver_port(&self) -> u16 {
        self.webserver_port
    }

    pub fn set_webserver_port(&mut self, webserver_port: u16) {
        self.webserver_port = webserver_port;
    }

    pub fn available_content(&self) -> &Vec<String> {
        &self.available_content
    }

    pub fn dim(&self) -> (usize, usize, usize) {
        self.dim
    }

    pub fn status(&self) -> (bool, &str) {
        (self.status.0, self.status.1.as_str())
    }

    pub fn set_status(&mut self, status: (bool, &str)) {
        self.status = (status.0, status.1.to_string());
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            enabled_content_indices: vec![0, 1],
            selected_content_index: 0,

            hue: 0.0,
            saturation: 1.0,
            brightness: 0.5,
            speed: 0.1,
            flip_vertical: false,
            cycle_duration: 0.0,
            ethernet_interface: "eth0".to_string(),
            modules: vec![(0, 0, AppState::MODULE_DEFAULT_ADDRESS)],

            webserver_port: 8080,

            available_content: get_all_content_names(),

            dim: (
                AppState::MODULE_X_RES,
                AppState::MODULE_Y_RES,
                AppState::MODULE_Z_RES,
            ),

            status: (false, "Nova is starting up.".to_string()),
        }
    }
}
