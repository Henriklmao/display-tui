//! display-tui: A terminal user interface for managing Hyprland monitor configurations.

pub mod actions;
pub mod app;
pub mod config;
pub mod errors;
pub mod monitor;
pub mod rotation;
pub mod test_utils;
pub mod ui;
pub mod utils;
pub mod validation;

pub use app::App;
pub use errors::{AppError, AppResult};
pub use monitor::Monitor;
pub use utils::TUIMode;
