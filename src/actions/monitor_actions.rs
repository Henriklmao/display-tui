//! Monitor state modification actions.

use crate::monitor::{Monitor, Position};

// Enables a monitor.
pub fn enable_monitor(monitor: &mut Monitor) {
    monitor.enabled = true;
    if let Some(saved_pos) = &monitor.saved_position {
        monitor.position = Some(Position { x: saved_pos.x, y: saved_pos.y });
    }
    monitor.scale = monitor.saved_scale.or(monitor.scale).or(Some(1.0));
}

// Disables a monitor.
pub fn disable_monitor(monitor: &mut Monitor) {
    monitor.enabled = false;
    monitor.saved_position = monitor.position.clone();
    monitor.saved_scale = monitor.scale;
}

// Cycles monitor rotation.
pub fn cycle_rotation(monitor: &mut Monitor) {
    use crate::rotation::Rotation;
    let current = Rotation::from_transform(&monitor.transform);
    let next = current.cycle();
    monitor.transform = Some(next.to_transform().to_string());
}

// Sets monitor resolution.
pub fn set_resolution(monitor: &mut Monitor, index: usize) {
    if index < monitor.modes.len() {
        for mode in &mut monitor.modes {
            mode.current = false;
        }
        monitor.modes[index].current = true;
    }
}

// Sets monitor scale.
pub fn set_scale(monitor: &mut Monitor, scale: f32) {
    monitor.scale = Some(scale);
}
