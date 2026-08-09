//! Configuration module for display-tui.

pub mod paths;
pub mod settings;
pub mod state;

pub use settings::Configuration;
pub use state::{save_monitor_state, load_monitor_state, MonitorState};
pub use state::{save_preset, load_preset, apply_preset, override_preset, count_enabled_monitors_in_preset, list_presets, delete_preset, rename_preset, is_last_preset, save_state_as_last, LAST_PRESET_NAME};
pub use paths::{get_config_path, get_state_path, get_presets_dir};
