//! UI component modules.

pub mod action_bar;
pub mod help;
pub mod list;
pub mod map;
pub mod popup;
pub mod preset;
pub mod resolution_table;
pub mod scale_table;
pub mod workspace_picker;

pub use action_bar::{ActionBar, ActionBarContext, ActionId};
pub use help::HelpModal;
pub use list::{ListColumn, MonitorList};
pub use map::Map;
pub use popup::{PopupAction, PopupFrame};
pub use preset::{MenuEvent, PresetAction, PresetEntry, PresetMenu};
pub use resolution_table::Resolutions;
pub use scale_table::Scale;
