use std::fs;

use serde::{Deserialize, Serialize};

use crate::content::get_all_content_names;
use crate::palettes::PALETTES;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Status {
    #[default]
    Unknown,
    Ok(String),
    Err(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    // Content and palettes are referenced by name, so saved settings survive entries
    // being added, removed, reordered or renamed. Missing fields (older settings files)
    // fall back to defaults when loading.
    #[serde(default)]
    enabled_content: Vec<String>,
    #[serde(default)]
    selected_content: String,

    brightness: f32,
    volume: f32,
    #[serde(default)]
    palette: String,
    heat: f32,
    flow: f32,
    form: f32,
    // Older settings files have no void; keep them loadable.
    #[serde(default = "AppState::default_void")]
    void: f32,
    flip_vertical: bool,

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
    #[serde(skip)]
    hardware_reset_requested: bool,
}

impl AppState {
    pub const MODULE_GRID_MAX: usize = 10;

    pub const MODULE_X_RES: usize = 5;
    pub const MODULE_Y_RES: usize = 5;
    pub const MODULE_Z_RES: usize = 10;

    pub const MODULE_DEFAULT_ADDRESS: u8 = 1;

    const SETTINGS_FILE: &str = "nova_settings.json";

    pub fn load() -> Self {
        if let Ok(json_string) = fs::read_to_string(Self::SETTINGS_FILE) {
            if let Ok(saved) = serde_json::from_str::<AppState>(&json_string) {
                return Self::from_saved(saved);
            }
            log::error!("Failed to parse settings, using defaults");
        } else {
            log::error!("Failed to load settings, using defaults");
        }
        let settings = Self::default();
        settings.save();
        settings
    }

    /// Settings restored from a saved file, validated through the setters. Unknown
    /// content and palette names fall back to defaults.
    fn from_saved(saved: AppState) -> Self {
        let mut settings = Self::default();
        settings.set_enabled_content(&saved.enabled_content);
        settings.set_selected_content(&saved.selected_content);
        settings.set_brightness(saved.brightness);
        settings.set_volume(saved.volume);
        settings.set_palette(&saved.palette);
        settings.set_heat(saved.heat);
        settings.set_flow(saved.flow);
        settings.set_form(saved.form);
        settings.set_void(saved.void);
        settings.set_flip_vertical(saved.flip_vertical);
        settings.set_ethernet_interface(&saved.ethernet_interface);

        let mut max_x = 0;
        let mut max_y = 0;
        let mut modules: Vec<(usize, usize, u8)> = Vec::new();
        for module in saved.modules.iter() {
            if module.0 >= AppState::MODULE_GRID_MAX || module.1 >= AppState::MODULE_GRID_MAX {
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

        settings.set_webserver_port(saved.webserver_port);
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

    /// Content module names, in the order of `get_all_content()`.
    pub fn available_content(&self) -> &[String] {
        &self.available_content
    }

    pub fn knows_content(&self, name: &str) -> bool {
        self.available_content
            .iter()
            .any(|available| available == name)
    }

    /// Enabled content module names, in the order of `available_content()`.
    pub fn enabled_content(&self) -> &[String] {
        &self.enabled_content
    }

    /// Enables the named modules. Unknown names are dropped; if none remain, every
    /// module is enabled. If the selected module is no longer enabled, the first
    /// enabled one is selected.
    pub fn set_enabled_content<S: AsRef<str>>(&mut self, names: &[S]) {
        self.enabled_content = self
            .available_content
            .iter()
            .filter(|available| names.iter().any(|name| name.as_ref() == available.as_str()))
            .cloned()
            .collect();
        if self.enabled_content.is_empty() {
            self.enabled_content = self.available_content.clone();
        }
        if !self.enabled_content.contains(&self.selected_content) {
            self.selected_content = self.enabled_content[0].clone();
        }
    }

    pub fn selected_content(&self) -> &str {
        &self.selected_content
    }

    /// Selects an enabled content module. Returns false, changing nothing, for any other name.
    pub fn set_selected_content(&mut self, name: &str) -> bool {
        let enabled = self.enabled_content.iter().any(|enabled| enabled == name);
        if enabled {
            self.selected_content = name.to_string();
        }
        enabled
    }

    /// Position of the selected module in `available_content()`, for the renderer.
    pub fn selected_content_index(&self) -> usize {
        self.available_content
            .iter()
            .position(|available| *available == self.selected_content)
            .unwrap_or(0)
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

    pub fn palette(&self) -> &str {
        &self.palette
    }

    /// Selects a palette by name. Returns false, changing nothing, for unknown names.
    pub fn set_palette(&mut self, name: &str) -> bool {
        let known = PALETTES.iter().any(|palette| palette.name == name);
        if known {
            self.palette = name.to_string();
        }
        known
    }

    /// Position of the selected palette in `PALETTES`.
    pub fn palette_index(&self) -> usize {
        PALETTES
            .iter()
            .position(|palette| palette.name == self.palette)
            .unwrap_or(0)
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

    pub fn void(&self) -> f32 {
        self.void
    }

    pub fn set_void(&mut self, void: f32) {
        if void.is_finite() {
            self.void = void.clamp(0.0, 1.0);
        }
    }

    fn default_void() -> f32 {
        0.5
    }

    pub fn is_flip_vertical(&self) -> bool {
        self.flip_vertical
    }

    pub fn set_flip_vertical(&mut self, flip_vertical: bool) {
        self.flip_vertical = flip_vertical;
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

    pub fn request_hardware_reset(&mut self) {
        self.hardware_reset_requested = true;
    }

    pub fn take_hardware_reset_request(&mut self) -> bool {
        std::mem::take(&mut self.hardware_reset_requested)
    }
}

impl Default for AppState {
    fn default() -> Self {
        let available_content = get_all_content_names();
        Self {
            enabled_content: available_content.clone(),
            selected_content: available_content[0].clone(),
            // New installations and Restore defaults start every control in the middle,
            // except Volume, which starts muted.
            brightness: 0.5,
            volume: 0.0,
            palette: PALETTES[0].name.to_string(),
            heat: 0.5,
            flow: 0.5,
            form: 0.5,
            void: AppState::default_void(),
            flip_vertical: false,
            ethernet_interface: "eth0".to_string(),
            modules: vec![(0, 0, AppState::MODULE_DEFAULT_ADDRESS)],

            webserver_port: 8080,

            available_content,

            dim: (
                AppState::MODULE_X_RES,
                AppState::MODULE_Y_RES,
                AppState::MODULE_Z_RES,
            ),

            status: Status::Unknown,
            audio_status: Status::Unknown,
            hardware_reset_requested: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_reset_request_is_consumed_once_and_not_persisted() {
        let mut state = AppState::default();
        assert!(!state.take_hardware_reset_request());
        state.request_hardware_reset();
        state.request_hardware_reset();
        let saved = serde_json::to_value(&state).unwrap();
        assert!(saved.get("hardware_reset_requested").is_none());
        let mut restored: AppState = serde_json::from_value(saved).unwrap();
        assert!(!restored.take_hardware_reset_request());
        assert!(state.take_hardware_reset_request());
        assert!(!state.take_hardware_reset_request());
    }

    #[test]
    fn audio_volume_validation_and_status_are_independent() {
        let mut state = AppState::default();
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
    fn unknown_names_fall_back_and_settings_round_trip() {
        let mut settings = AppState::default();
        let first = settings.available_content()[0].clone();
        assert!(!settings.set_selected_content("Renamed"));
        assert!(!settings.set_palette("Renamed"));
        assert_eq!(settings.selected_content(), first);
        settings.set_enabled_content(&["Renamed"]);
        assert_eq!(settings.enabled_content(), settings.available_content());
        assert!(settings.set_palette(PALETTES[3].name));
        settings.set_void(0.3);
        let json = serde_json::to_string(&settings).unwrap();
        let restored = AppState::from_saved(serde_json::from_str(&json).unwrap());
        assert_eq!(restored.palette(), PALETTES[3].name);
        assert_eq!(restored.palette_index(), 3);
        assert_eq!(restored.selected_content(), first);
        assert_eq!(restored.void(), 0.3);
    }

    #[test]
    fn older_settings_files_still_load() {
        // Saved before content and palettes were referenced by name, and before Void.
        let mut saved = serde_json::to_value(AppState::default()).unwrap();
        let fields = saved.as_object_mut().unwrap();
        for field in ["enabled_content", "selected_content", "palette", "void"] {
            fields.remove(field);
        }
        fields.insert("enabled_content_indices".into(), serde_json::json!([0, 3]));
        fields.insert("selected_content_index".into(), serde_json::json!(3));
        fields.insert("tone".into(), serde_json::json!(0.4));
        let restored = AppState::from_saved(serde_json::from_value(saved).unwrap());
        assert_eq!(restored.enabled_content(), restored.available_content());
        assert_eq!(restored.selected_content(), restored.available_content()[0]);
        assert_eq!(restored.palette(), PALETTES[0].name);
        assert_eq!(restored.void(), AppState::default_void());
    }
}
