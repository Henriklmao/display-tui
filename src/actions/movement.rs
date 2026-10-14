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

#[cfg(test)]
mod tests {
    use super::{move_horizontal, move_vertical, snap_horizontal, snap_vertical};
    use crate::monitor::{Monitor, Position, Resolution};

    // A monitor of 1920x1080 logical pixels placed at (x, y).
    fn monitor(name: &str, x: i32, y: i32) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
            position: Some(Position { x, y }),
            scale: Some(1.0),
            modes: vec![Resolution {
                width: 1920,
                height: 1080,
                refresh: 60.0,
                preferred: true,
                current: true,
            }],
            ..Default::default()
        }
    }

    fn x_of(m: &Monitor) -> i32 {
        m.position.clone().unwrap().x
    }

    fn y_of(m: &Monitor) -> i32 {
        m.position.clone().unwrap().y
    }

    #[test]
    fn move_actions_shift_the_position() {
        let mut m = monitor("A", 100, 100);

        move_horizontal(&mut m, 500);
        move_vertical(&mut m, -250);

        assert_eq!((x_of(&m), y_of(&m)), (600, -150));
    }

    #[test]
    fn move_actions_ignore_monitors_without_position() {
        let mut m = monitor("A", 0, 0);
        m.position = None;

        move_horizontal(&mut m, 10);
        move_vertical(&mut m, 10);

        assert_eq!(m.position, None);
    }

    #[test]
    fn snap_horizontal_aligns_the_left_edge_with_the_neighbour() {
        // B sits 1080px to the right of A's right edge; snapping left puts
        // B's left edge exactly on A's right edge.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];

        snap_horizontal(&mut monitors, 1, -1);

        assert_eq!(x_of(&monitors[1]), 1920);
        assert_eq!(x_of(&monitors[0]), 0, "other monitors must not move");
    }

    #[test]
    fn snap_horizontal_never_moves_against_the_direction() {
        // Everything is to the left of B, so snapping right finds no target.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];

        snap_horizontal(&mut monitors, 1, 1);

        assert_eq!(x_of(&monitors[1]), 3000);
    }

    #[test]
    fn snap_vertical_aligns_with_the_neighbour() {
        // A spans y 0..1080; the nearest edge above B (y=2000) is 1080.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 0, 2000)];

        snap_vertical(&mut monitors, 1, -1);

        assert_eq!(y_of(&monitors[1]), 1080);
    }

    #[test]
    fn snap_vertical_can_move_downwards_too() {
        // B sits above A; snapping down stacks it directly on top.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 0, -3000)];

        snap_vertical(&mut monitors, 1, 1);

        assert!(y_of(&monitors[1]) > -3000, "B must move down");
        assert_eq!(y_of(&monitors[1]), -1080, "B ends up flush on top of A");
    }

    #[test]
    fn origin_is_always_a_snap_target() {
        // Nothing else to snap against, so the single monitor lands on 0,0.
        let mut monitors = vec![monitor("A", 137, 42)];

        snap_horizontal(&mut monitors, 0, -1);
        snap_vertical(&mut monitors, 0, -1);

        assert_eq!((x_of(&monitors[0]), y_of(&monitors[0])), (0, 0));
    }

    #[test]
    fn disabled_monitors_are_not_snap_targets() {
        // A is disabled, so B may only snap to the origin.
        let mut monitors = vec![
            {
                let mut m = monitor("A", 0, 0);
                m.enabled = false;
                m
            },
            monitor("B", 3000, 0),
        ];

        snap_horizontal(&mut monitors, 1, -1);

        assert_eq!(x_of(&monitors[1]), 0);
    }

    #[test]
    fn a_disabled_selected_monitor_still_snaps() {
        // Only the *targets* exclude disabled monitors; the selected one is
        // still allowed to move.
        let mut monitors = vec![
            monitor("A", 0, 0),
            {
                let mut m = monitor("B", 3000, 0);
                m.enabled = false;
                m
            },
        ];

        snap_horizontal(&mut monitors, 1, -1);

        assert_eq!(x_of(&monitors[1]), 1920);
    }

    #[test]
    fn snap_targets_use_logical_size_of_the_neighbour() {
        // A is scaled to 2x, so its logical width is 960 and B lands on 960
        // instead of the 1920 it would snap to at scale 1.
        let mut scaled = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];
        scaled[0].scale = Some(2.0);
        snap_horizontal(&mut scaled, 1, -1);
        assert_eq!(x_of(&scaled[1]), 960);

        let mut unscaled = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];
        snap_horizontal(&mut unscaled, 1, -1);
        assert_eq!(x_of(&unscaled[1]), 1920);
    }

    #[test]
    fn snap_rounds_fractional_deltas_to_whole_pixels() {
        // A at 175% scale is 1097.14... logical pixels wide, so the snap gap
        // is fractional; the result must still be a whole pixel.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 1000, 0)];
        monitors[0].scale = Some(1.75);

        snap_horizontal(&mut monitors, 1, 1);

        assert_eq!(x_of(&monitors[1]), 1097);
    }
}
