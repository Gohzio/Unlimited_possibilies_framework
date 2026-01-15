use std::fs;
use std::path::PathBuf;

use crate::ui::settings::UiSettings;

fn settings_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("your_app_name");
    fs::create_dir_all(&path).ok();
    path.push("ui_settings.json");
    path
}

pub fn load_settings() -> UiSettings {
    let path = settings_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &UiSettings) {
    let path = settings_path();
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, json);
    }
}
