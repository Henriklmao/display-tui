//! Configuration module for display-tui.

pub mod paths;
pub mod settings;
pub mod state;

pub use settings::Configuration;
pub use state::{save_monitor_state, load_monitor_state, MonitorState};
pub use paths::{get_config_path, get_state_path};
