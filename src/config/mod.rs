//! Configuration module for display-tui.

pub mod paths;
pub mod settings;
pub mod state;

pub use paths::{get_config_path, get_presets_dir, get_state_path};
pub use settings::Configuration;
pub(crate) use state::apply_preset_in;
pub(crate) use state::save_monitor_state_in;
pub use state::{
    LAST_PRESET_NAME, apply_preset, count_enabled_monitors_in_preset, delete_preset,
    is_last_preset, list_presets, load_preset, override_preset, rename_preset, save_preset,
    save_state_as_last, validate_preset_monitors_match,
};
pub use state::{MonitorState, ResolutionState, load_monitor_state, save_monitor_state};
