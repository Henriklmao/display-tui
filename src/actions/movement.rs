//! Monitor movement and snapping actions.

use crate::monitor::Monitor;
use crate::utils::find_best_delta;

// Moves a monitor vertically.
pub fn move_vertical(monitor: &mut Monitor, delta: i32) {
    if let Some(ref mut pos) = monitor.position {
        pos.y += delta;
    }
}

// Moves a monitor horizontally.
pub fn move_horizontal(monitor: &mut Monitor, delta: i32) {
    if let Some(ref mut pos) = monitor.position {
        pos.x += delta;
    }
}

// Snaps a monitor vertically to align with other monitors.
pub fn snap_vertical(monitors: &mut [Monitor], selected: usize, direction: i32) {
    let mut targets = vec![0.0];
    for (i, m) in monitors.iter().enumerate() {
        if i == selected || !m.enabled { continue; }
        let (_, y, _, h) = m.get_geometry();
        targets.push(y);
        targets.push(y + h);
        targets.push(y + h / 2.0);
    }

    let (_, sy, _, sh) = monitors[selected].get_geometry();
    let sources = vec![sy, sy + sh, sy + sh / 2.0];

    if let Some(delta) = find_best_delta(&sources, &targets, direction) {
        move_vertical(&mut monitors[selected], delta.round() as i32);
    }
}

// Snaps a monitor horizontally.
pub fn snap_horizontal(monitors: &mut [Monitor], selected: usize, direction: i32) {
    let mut targets = vec![0.0];
    for (i, m) in monitors.iter().enumerate() {
        if i == selected || !m.enabled { continue; }
        let (x, _, w, _) = m.get_geometry();
        targets.push(x);
        targets.push(x + w);
        targets.push(x + w / 2.0);
    }

    let (sx, _, sw, _) = monitors[selected].get_geometry();
    let sources = vec![sx, sx + sw, sx + sw / 2.0];

    if let Some(delta) = find_best_delta(&sources, &targets, direction) {
        move_horizontal(&mut monitors[selected], delta.round() as i32);
    }
}
