//! Monitor module for display-tui.
//!
//! Provides monitor discovery, geometry calculations, and Hyprland
//! configuration generation.

pub mod config_gen;
pub mod geometry;
pub mod parser;
pub mod types;

pub use types::{Monitor, MonitorCanvas, Position, Resolution};
