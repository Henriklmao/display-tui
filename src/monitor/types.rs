//! Monitor data types and core operations.
//!
//! Core data structures and methods for monitor representation, resolution
//! management, movement, and geometry calculations.

use crate::rotation::Rotation;
use serde::{Deserialize, Serialize};

// Represents a physical or virtual monitor.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Monitor {
    pub name: String,
    pub description: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub enabled: bool,
    pub modes: Vec<Resolution>,
    pub position: Option<Position>,
    pub scale: Option<f32>,
    pub transform: Option<String>,
    pub workspace: Option<u8>,
    #[serde(skip)]
    pub saved_position: Option<Position>,
    #[serde(skip)]
    pub saved_scale: Option<f32>,
}

// Monitor position on the virtual desktop.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

// Available resolution mode for a monitor.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Resolution {
    pub width: i32,
    pub height: i32,
    pub refresh: f32,
    pub preferred: bool,
    pub current: bool,
}

// Canvas representation of monitor layout for 2D rendering.
#[derive(Debug, Clone, Deserialize)]
pub struct MonitorCanvas {
    pub top: i32,
    pub x_bounds: [f64; 2],
    pub y_bounds: [f64; 2],
    pub offset_y: i32,
}

// JSON structure from hyprctl monitors -j output.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HyprMonitor {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub serial: String,
    pub width: i32,
    pub height: i32,
    pub refresh_rate: f32,
    pub x: i32,
    pub y: i32,
    pub scale: f32,
    pub transform: i32,
    pub disabled: bool,
    #[serde(default)]
    pub available_modes: Vec<String>,
    pub active_workspace: Option<HyprWorkspace>,
}

// Workspace info from hyprctl JSON output.
#[derive(Debug, Deserialize)]
pub(crate) struct HyprWorkspace {
    pub id: i32,
    #[allow(dead_code)]
    pub name: String,
}

impl Monitor {
    // Returns the currently active resolution.
    pub fn get_current_resolution(&self) -> Option<&Resolution> {
        self.modes.iter().find(|m| m.current)
    }

    // Returns the preferred (native) resolution.
    pub fn get_prefered_resolution(&self) -> Option<&Resolution> {
        self.modes.iter().find(|m| m.preferred)
    }

    // Returns the best available resolution (current, falling back to preferred).
    pub fn get_best_resolution(&self) -> Option<&Resolution> {
        self.get_current_resolution()
            .or_else(|| self.get_prefered_resolution())
    }

    // Returns logical dimensions accounting for rotation and scale.
    pub fn get_logical_dimensions(&self) -> (f64, f64) {
        let mode = match self.get_best_resolution() {
            Some(m) => m,
            None => return (0.0, 0.0),
        };

        let rotation = Rotation::from_transform(&self.transform);
        let (width, height) = if rotation == Rotation::Deg90 || rotation == Rotation::Deg270 {
            (mode.height as f64, mode.width as f64)
        } else {
            (mode.width as f64, mode.height as f64)
        };

        let scale = self.scale.unwrap_or(1.0) as f64;
        (width / scale, height / scale)
    }

    // Sets the active resolution by index.
    pub fn set_current_resolution(&mut self, index: usize) {
        if index < self.modes.len() {
            for mode in &mut self.modes {
                mode.current = false;
            }
            self.modes[index].current = true;
        } else {
            eprintln!("Index out of bounds: {}", index);
        }
    }

    // Returns the monitor's 2D geometry (x, y, width, height) as floats.
    pub fn get_geometry(&self) -> (f64, f64, f64, f64) {
        let (logical_width, logical_height) = self.get_logical_dimensions();
        if logical_width == 0.0 && logical_height == 0.0 {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let x = self.position.clone().unwrap().x as f64;
        let y = self.position.clone().unwrap().y as f64;

        (x, y, logical_width, logical_height)
    }

    // Moves the monitor vertically by the given pixel delta.
    pub fn move_vertical(&mut self, direction: i32) {
        if let Some(ref mut pos) = self.position {
            pos.y += direction;
        }
    }

    // Moves the monitor horizontally by the given pixel delta.
    pub fn move_horizontal(&mut self, direction: i32) {
        if let Some(ref mut pos) = self.position {
            pos.x += direction;
        }
    }
}
