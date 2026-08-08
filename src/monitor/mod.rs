//! Monitor module for display-tui.
//!
//! Provides monitor discovery, geometry calculations, and Hyprland
//! configuration generation.

pub mod types;
pub mod parser;
pub mod geometry;
pub mod config_gen;

pub use types::{Monitor, MonitorCanvas, Position, Resolution};
