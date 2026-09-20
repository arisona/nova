use std::fs;

use serde::{Deserialize, Serialize};

use crate::content::get_all_content_names;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Status {
    #[default]
    Unknown,
    Ok(String),
    Err(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    content_version: u32,
    enabled_content_indices: Vec<u32>,
    selected_content_index: usize,

    #[serde(alias = "glow")]
    brightness: f32,
    #[serde(default)]
    volume: f32,
    tone: f32,
    heat: f32,
    flow: f32,
    form: f32,
    flip_vertical: bool,
    cycle_duration: f32, // TODO: currently ignored, need to rework

    ethernet_interface: String,
    modules: Vec<(usize, usize, u8)>,
    webserver_port: u16,

    #[serde(skip_serializing, skip_deserializing)]
    available_content: Vec<String>,

    #[serde(skip_serializing, skip_deserializing)]
    dim: (usize, usize, usize),

    #[serde(skip_serializing, skip_deserializing)]
    status: Status,
    #[serde(skip)]
    audio_status: Status,
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
            if let Ok(mut parsed_settings) = serde_json::from_str::<AppState>(&json_string) {
                let migrated = parsed_settings.migrate_content();
                settings.set_selected_content_index(parsed_settings.selected_content_index);
                settings.set_enabled_content_indices(parsed_settings.enabled_content_indices);
                settings.set_brightness(parsed_settings.brightness);
                settings.set_volume(parsed_settings.volume);
                settings.set_tone(parsed_settings.tone);
                settings.set_heat(parsed_settings.heat);
                settings.set_flow(parsed_settings.flow);
                settings.set_form(parsed_settings.form);
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
                            log::warn!(
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
                        log::warn!("No valid modules found, using defaults");
                    }
                }

                settings.set_webserver_port(parsed_settings.webserver_port);
                if migrated {
                    settings.save();
                }
            } else {
                log::error!("Failed to parse settings, using defaults");
                settings.save();
            }
        } else {
            log::error!("Failed to load settings, using defaults");
            settings.save();
        }
        settings
    }

    pub fn save(&self) {
        if let Ok(json_string) = serde_json::to_string_pretty(self) {
            if let Err(e) = fs::write(Self::SETTINGS_FILE, json_string) {
                log::error!("Failed to save settings: {e}");
            }
        } else {
            log::error!("Failed to serialize settings");
        }
    }

    fn migrate_content(&mut self) -> bool {
        if self.content_version >= 1 {
            return false;
        }
        self.selected_content_index = match self.selected_content_index {
            2 | 5 => 3,
            3 => 1,
            4 => 2,
            _ => 0,
        };
        self.enabled_content_indices = (0..get_all_content_names().len() as u32).collect();
        self.content_version = 1;
        true
    }

    pub fn enabled_content_indices(&self) -> &Vec<u32> {
        &self.enabled_content_indices
    }

    pub fn set_enabled_content_indices(&mut self, indices: Vec<u32>) {
        self.enabled_content_indices.clear();
        for index in indices {
            if (index as usize) < self.available_content.len()
                && !self.enabled_content_indices.contains(&index)
            {
                self.enabled_content_indices.push(index);
            }
        }
        if let [index] = self.enabled_content_indices.as_slice() {
            self.selected_content_index = *index as usize;
        }
    }

    pub fn selected_content_index(&self) -> usize {
        self.selected_content_index
    }

    pub fn set_selected_content_index(&mut self, index: usize) {
        if index < self.available_content.len() {
            self.selected_content_index = index;
        }
    }

    pub fn brightness(&self) -> f32 {
        self.brightness
    }
    pub fn set_brightness(&mut self, brightness: f32) {
        if brightness.is_finite() {
            self.brightness = brightness.clamp(0.0, 1.0);
        }
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        if volume.is_finite() {
            self.volume = volume.clamp(0.0, 1.0);
        }
    }

    pub fn tone(&self) -> f32 {
        self.tone
    }
    pub fn set_tone(&mut self, tone: f32) {
        self.tone = tone.clamp(0.0, 1.0);
    }

    pub fn heat(&self) -> f32 {
        self.heat
    }
    pub fn set_heat(&mut self, heat: f32) {
        self.heat = heat.clamp(0.0, 1.0);
    }

    pub fn flow(&self) -> f32 {
        self.flow
    }
    pub fn set_flow(&mut self, flow: f32) {
        self.flow = flow.clamp(0.0, 1.0);
    }

    pub fn form(&self) -> f32 {
        self.form
    }

    pub fn set_form(&mut self, form: f32) {
        self.form = form.clamp(0.0, 1.0);
    }

    pub fn is_flip_vertical(&self) -> bool {
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

    pub fn modules(&self) -> &Vec<(usize, usize, u8)> {
        &self.modules
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

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = status;
    }

    pub fn audio_status(&self) -> &Status {
        &self.audio_status
    }

    pub fn set_audio_status(&mut self, status: Status) {
        self.audio_status = status;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            content_version: 1,
            enabled_content_indices: vec![0, 1, 2, 3],
            selected_content_index: 0,
            brightness: 0.5,
            volume: 0.0,
            tone: 0.0,
            heat: 0.0,
            flow: 0.0,
            form: 0.0,
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

            status: Status::Unknown,
            audio_status: Status::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_volume_migration_validation_and_status_are_independent() {
        let mut json = serde_json::to_value(AppState::default()).unwrap();
        json.as_object_mut().unwrap().remove("volume");
        let mut state: AppState = serde_json::from_value(json).unwrap();
        assert_eq!(state.volume(), 0.0);
        state.set_volume(0.37);
        state.set_volume(f32::NAN);
        state.set_volume(f32::INFINITY);
        assert_eq!(state.volume(), 0.37);
        let saved = serde_json::to_value(&state).unwrap();
        let restored: AppState = serde_json::from_value(saved).unwrap();
        assert_eq!(restored.volume(), 0.37);
        state.set_volume(2.0);
        assert_eq!(state.volume(), 1.0);
        state.set_volume(-1.0);
        assert_eq!(state.volume(), 0.0);
        state.set_status(Status::Err("Display unavailable".into()));
        state.set_audio_status(Status::Ok("Audio ready".into()));
        assert_eq!(state.status(), &Status::Err("Display unavailable".into()));
        assert_eq!(state.audio_status(), &Status::Ok("Audio ready".into()));
        assert!(
            serde_json::to_value(&state)
                .unwrap()
                .get("audio_status")
                .is_none()
        );
    }

    #[test]
    fn legacy_settings_keep_levels_and_migrate_content() {
        for (old_index, new_index) in [(0, 0), (1, 0), (2, 3), (3, 1), (4, 2), (5, 3), (99, 0)] {
            let mut json = serde_json::to_value(AppState::default()).unwrap();
            let object = json.as_object_mut().unwrap();
            object.remove("content_version");
            object.remove("brightness");
            object.insert("glow".into(), serde_json::json!(0.37));
            object.insert(
                "selected_content_index".into(),
                serde_json::json!(old_index),
            );
            let mut settings: AppState = serde_json::from_value(json).unwrap();
            assert!(settings.migrate_content());
            assert!(!settings.migrate_content());
            assert_eq!(settings.selected_content_index, new_index);
            assert_eq!(settings.enabled_content_indices, [0, 1, 2, 3]);
            assert_eq!(settings.brightness(), 0.37);
            let saved = serde_json::to_value(settings).unwrap();
            assert_eq!(saved["brightness"], 0.37_f32);
            assert!(saved.get("glow").is_none());
        }
    }

    #[test]
    fn current_settings_and_invalid_indices_are_safe() {
        let mut settings = AppState::default();
        settings.set_selected_content_index(3);
        settings.set_selected_content_index(usize::MAX);
        assert_eq!(settings.selected_content_index(), 3);
        settings.set_enabled_content_indices(vec![3, 2, 3, 99]);
        assert_eq!(settings.enabled_content_indices(), &[3, 2]);
        let json = serde_json::to_string(&settings).unwrap();
        let mut restored: AppState = serde_json::from_str(&json).unwrap();
        assert!(!restored.migrate_content());
        assert_eq!(restored.selected_content_index(), 3);
        assert_eq!(restored.enabled_content_indices(), &[3, 2]);
    }

    #[test]
    fn single_enabled_content_is_selected() {
        let mut settings = AppState::default();
        settings.set_selected_content_index(0);
        settings.set_enabled_content_indices(vec![2, 2, 99]);
        assert_eq!(settings.enabled_content_indices(), &[2]);
        assert_eq!(settings.selected_content_index(), 2);
        settings.set_enabled_content_indices(vec![1, 2]);
        assert_eq!(settings.selected_content_index(), 2);
        settings.set_enabled_content_indices(vec![]);
        assert_eq!(settings.selected_content_index(), 2);
    }
}
