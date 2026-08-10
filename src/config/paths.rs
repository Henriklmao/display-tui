//! Configuration file paths for the display-tui application.

use std::path::PathBuf;

// Returns the path to the monitor state JSON file.
pub fn get_state_path() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".config/display-tui/monitor_state.json"))
        .unwrap_or_else(|| PathBuf::from("monitor_state.json"))
}

// Returns the path to the display-tui configuration JSON file.
pub fn get_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".config/display-tui/config.json"))
        .unwrap_or_else(|| PathBuf::from("config.json"))
}

// Returns the path to the presets directory.
pub fn get_presets_dir() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".config/display-tui/presets"))
        .unwrap_or_else(|| PathBuf::from("presets"))
}
