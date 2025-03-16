use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    pub port: u16,

    pub status_ok: bool,
    pub status_message: String,

    pub available_content: Vec<String>,
    pub enabled_content_indices: Vec<u32>,
    pub selected_content_index: u32,

    pub hue: f32,
    pub saturation: f32,
    pub brightness: f32,
    pub speed: f32,
    pub flip_vertical: bool,
    pub cycle_duration: f32,
    pub ethernet_interface: String,
    pub module0_address: String,
}

impl AppState {
    const SETTINGS_FILE: &str = "nova_settings.json";

    pub fn load() -> Self {
        if let Ok(json_string) = fs::read_to_string(Self::SETTINGS_FILE) {
            if let Ok(parsed_settings) = serde_json::from_str::<AppState>(&json_string) {
                return parsed_settings;
            }
        }
        eprintln!("Failed to load settings, using defaults");
        Self::default()
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            port: 8080,

            status_ok: false,
            status_message: "Nova is starting up.".to_string(),

            available_content: vec!["Fill".to_string()],
            enabled_content_indices: vec![0],
            selected_content_index: 0,

            hue: 0.0,
            saturation: 0.0,
            brightness: 0.0,
            speed: 0.0,
            flip_vertical: false,
            cycle_duration: 0.0,
            ethernet_interface: "eth0".to_string(),
            module0_address: "1".to_string(),
        }
    }
}
