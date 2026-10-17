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
//
// The bounds are the only bookkeeping needed: the map mirrors monitor
// positions around the centre of `y_bounds`, so monitor coordinates that
// grow downwards render from top to bottom.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MonitorCanvas {
    pub x_bounds: [f64; 2],
    pub y_bounds: [f64; 2],
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
    //
    // A monitor without a position (or without a usable resolution) has no
    // place on the canvas, so its geometry is zeroed out instead of panicking.
    pub fn get_geometry(&self) -> (f64, f64, f64, f64) {
        let (logical_width, logical_height) = self.get_logical_dimensions();
        let (x, y) = match &self.position {
            Some(position) => (position.x as f64, position.y as f64),
            None => return (0.0, 0.0, 0.0, 0.0),
        };
        if logical_width == 0.0 && logical_height == 0.0 {
            return (0.0, 0.0, 0.0, 0.0);
        }

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

#[cfg(test)]
mod tests {
    use super::{Monitor, Position, Resolution};

    fn resolution(width: i32, height: i32, refresh: f32) -> Resolution {
        Resolution {
            width,
            height,
            refresh,
            preferred: false,
            current: false,
        }
    }

    fn monitor(modes: Vec<Resolution>) -> Monitor {
        Monitor {
            name: "TEST-1".to_string(),
            enabled: true,
            position: Some(Position { x: 0, y: 0 }),
            scale: Some(1.0),
            modes,
            ..Default::default()
        }
    }

    // A monitor whose first mode is both current and preferred, so that
    // get_best_resolution() has something to return.
    fn active(mut m: Monitor) -> Monitor {
        if let Some(first) = m.modes.first_mut() {
            first.current = true;
            first.preferred = true;
        }
        m
    }

    #[test]
    fn get_current_resolution_returns_the_marked_mode() {
        let mut m = monitor(vec![resolution(1920, 1080, 60.0), resolution(1280, 720, 75.0)]);
        m.modes[1].current = true;

        let current = m.get_current_resolution().expect("current mode");
        assert_eq!((current.width, current.height), (1280, 720));
        assert_eq!(current.refresh, 75.0);
    }

    #[test]
    fn get_current_resolution_is_none_when_nothing_is_marked() {
        let m = monitor(vec![resolution(1920, 1080, 60.0)]);
        assert!(m.get_current_resolution().is_none());
        assert!(m.get_prefered_resolution().is_none());
    }

    #[test]
    fn get_current_resolution_is_none_without_modes() {
        let m = monitor(vec![]);
        assert!(m.get_current_resolution().is_none());
        assert!(m.get_best_resolution().is_none());
    }

    #[test]
    fn get_prefered_resolution_returns_the_preferred_mode() {
        let mut m = monitor(vec![resolution(1920, 1080, 60.0), resolution(1280, 720, 60.0)]);
        m.modes[1].preferred = true;

        let preferred = m.get_prefered_resolution().expect("preferred mode");
        assert_eq!((preferred.width, preferred.height), (1280, 720));
    }

    #[test]
    fn get_best_resolution_prefers_the_current_mode() {
        let mut m = monitor(vec![resolution(1920, 1080, 60.0), resolution(1280, 720, 60.0)]);
        m.modes[0].preferred = true;
        m.modes[1].current = true;

        let best = m.get_best_resolution().expect("best mode");
        assert_eq!((best.width, best.height), (1280, 720));
    }

    #[test]
    fn get_best_resolution_falls_back_to_preferred() {
        let mut m = monitor(vec![resolution(1920, 1080, 60.0), resolution(1280, 720, 60.0)]);
        m.modes[1].preferred = true;

        let best = m.get_best_resolution().expect("best mode");
        assert_eq!((best.width, best.height), (1280, 720));
    }

    #[test]
    fn set_current_resolution_switches_exactly_one_mode() {
        let mut m = monitor(vec![
            resolution(1920, 1080, 60.0),
            resolution(1280, 720, 60.0),
            resolution(800, 600, 60.0),
        ]);
        m.modes[0].current = true;

        m.set_current_resolution(2);

        let current: Vec<usize> = m
            .modes
            .iter()
            .enumerate()
            .filter(|(_, mode)| mode.current)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(current, vec![2]);
        assert_eq!(
            (m.modes[2].width, m.modes[2].height),
            (800, 600),
            "flags must not disturb the mode itself"
        );
    }

    #[test]
    fn set_current_resolution_ignores_out_of_bounds_indices() {
        let mut m = monitor(vec![resolution(1920, 1080, 60.0)]);
        m.modes[0].current = true;

        m.set_current_resolution(5);
        m.set_current_resolution(usize::MAX);

        assert!(m.modes[0].current, "existing selection must survive");
        assert_eq!(m.modes.len(), 1);
    }

    #[test]
    fn logical_dimensions_use_physical_size_at_scale_one() {
        let m = active(monitor(vec![resolution(1920, 1080, 60.0)]));
        assert_eq!(m.get_logical_dimensions(), (1920.0, 1080.0));
    }

    #[test]
    fn logical_dimensions_are_divided_by_scale() {
        let mut m = active(monitor(vec![resolution(1920, 1080, 60.0)]));
        m.scale = Some(2.0);
        assert_eq!(m.get_logical_dimensions(), (960.0, 540.0));

        m.scale = Some(0.5);
        assert_eq!(m.get_logical_dimensions(), (3840.0, 2160.0));
    }

    #[test]
    fn logical_dimensions_default_to_scale_one() {
        let mut m = active(monitor(vec![resolution(1920, 1080, 60.0)]));
        m.scale = None;
        assert_eq!(m.get_logical_dimensions(), (1920.0, 1080.0));
    }

    #[test]
    fn logical_dimensions_swap_on_quarter_turns() {
        let mut m = active(monitor(vec![resolution(1920, 1080, 60.0)]));

        m.transform = Some("normal".to_string());
        assert_eq!(m.get_logical_dimensions(), (1920.0, 1080.0));

        m.transform = Some("180".to_string());
        assert_eq!(m.get_logical_dimensions(), (1920.0, 1080.0));

        m.transform = Some("90".to_string());
        assert_eq!(m.get_logical_dimensions(), (1080.0, 1920.0));

        m.transform = Some("270".to_string());
        assert_eq!(m.get_logical_dimensions(), (1080.0, 1920.0));
    }

    #[test]
    fn logical_dimensions_apply_scale_after_rotation() {
        let mut m = active(monitor(vec![resolution(1920, 1080, 60.0)]));
        m.transform = Some("90".to_string());
        m.scale = Some(2.0);
        assert_eq!(m.get_logical_dimensions(), (540.0, 960.0));
    }

    #[test]
    fn logical_dimensions_are_zero_without_modes() {
        let m = monitor(vec![]);
        assert_eq!(m.get_logical_dimensions(), (0.0, 0.0));
    }

    #[test]
    fn geometry_is_position_plus_logical_size() {
        let mut m = active(monitor(vec![resolution(1920, 1080, 60.0)]));
        m.position = Some(Position { x: -1920, y: 540 });
        m.scale = Some(2.0);

        assert_eq!(m.get_geometry(), (-1920.0, 540.0, 960.0, 540.0));
    }

    #[test]
    fn geometry_is_zeroed_without_modes() {
        let mut m = monitor(vec![]);
        m.position = Some(Position { x: 100, y: 200 });
        assert_eq!(m.get_geometry(), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn geometry_is_zeroed_without_a_position() {
        // The parser always fills the position in, but losing it must not
        // take the whole layout down with a panic.
        let mut m = active(monitor(vec![resolution(1920, 1080, 60.0)]));
        m.position = None;

        assert_eq!(m.get_geometry(), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn movement_adjusts_the_position_in_place() {
        let mut m = monitor(vec![]);
        m.position = Some(Position { x: 100, y: 100 });

        m.move_horizontal(50);
        m.move_vertical(-25);
        m.move_horizontal(0);
        m.move_vertical(0);

        assert_eq!(m.position, Some(Position { x: 150, y: 75 }));
    }

    #[test]
    fn movement_handles_negative_coordinates() {
        let mut m = monitor(vec![]);
        m.position = Some(Position { x: 0, y: 0 });

        m.move_horizontal(-1);
        m.move_vertical(-1);

        assert_eq!(m.position, Some(Position { x: -1, y: -1 }));
    }

    #[test]
    fn movement_without_position_is_a_no_op() {
        let mut m = monitor(vec![]);
        m.position = None;

        m.move_horizontal(100);
        m.move_vertical(100);

        assert_eq!(m.position, None);
    }

    #[test]
    fn monitor_deserializes_with_defaults_for_missing_optional_fields() {
        let json = r#"{
            "name": "DP-1",
            "enabled": true,
            "modes": [],
            "position": null,
            "scale": null,
            "transform": null,
            "workspace": null
        }"#;
        let m: Monitor = serde_json::from_str(json).expect("valid monitor json");
        assert_eq!(m.name, "DP-1");
        assert!(m.description.is_none());
        assert!(m.saved_position.is_none());
        assert!(m.saved_scale.is_none());
    }

    #[test]
    fn saved_state_is_not_serialized() {
        let mut m = monitor(vec![resolution(1920, 1080, 60.0)]);
        m.saved_position = Some(Position { x: 10, y: 20 });
        m.saved_scale = Some(1.5);

        let json = serde_json::to_string(&m).expect("serializable");
        assert!(!json.contains("saved_position"));
        assert!(!json.contains("saved_scale"));
    }

    #[test]
    fn position_serializes_as_a_plain_object() {
        let pos = Position { x: -5, y: 10 };
        let json = serde_json::to_string(&pos).unwrap();
        assert_eq!(json, r#"{"x":-5,"y":10}"#);
        let back: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pos);
    }
}
