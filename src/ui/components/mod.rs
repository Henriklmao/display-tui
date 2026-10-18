//! UI component modules.

pub mod help;
pub mod list;
pub mod map;
pub mod popup;
pub mod preset;
pub mod resolution_table;
pub mod scale_table;

pub use help::HelpModal;
pub use list::MonitorList;
pub use map::Map;
pub use popup::{PopupAction, PopupFrame};
pub use preset::{MenuEvent, PresetAction, PresetEntry, PresetMenu};
pub use resolution_table::Resolutions;
pub use scale_table::Scale;
