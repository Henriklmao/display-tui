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

#[cfg(test)]
mod tests {
    use super::{cycle_rotation, disable_monitor, enable_monitor, set_resolution, set_scale};
    use crate::monitor::{Monitor, Position, Resolution};

    fn monitor() -> Monitor {
        Monitor {
            name: "DP-1".to_string(),
            enabled: true,
            position: Some(Position { x: 500, y: 300 }),
            scale: Some(1.25),
            modes: vec![
                Resolution {
                    width: 1920,
                    height: 1080,
                    refresh: 60.0,
                    preferred: true,
                    current: true,
                },
                Resolution {
                    width: 1280,
                    height: 720,
                    refresh: 75.0,
                    preferred: false,
                    current: false,
                },
            ],
            ..Default::default()
        }
    }

    fn current_index(m: &Monitor) -> usize {
        m.modes
            .iter()
            .position(|mode| mode.current)
            .expect("a mode must be selected")
    }

    #[test]
    fn enable_marks_the_monitor_as_active() {
        let mut m = monitor();
        m.enabled = false;

        enable_monitor(&mut m);

        assert!(m.enabled);
    }

    #[test]
    fn disable_remembers_position_and_scale() {
        let mut m = monitor();

        disable_monitor(&mut m);

        assert!(!m.enabled);
        assert_eq!(m.saved_position, Some(Position { x: 500, y: 300 }));
        assert_eq!(m.saved_scale, Some(1.25));
    }

    #[test]
    fn enable_restores_the_geometry_saved_on_disable() {
        let mut m = monitor();
        disable_monitor(&mut m);

        // Simulate a later change to the (now disabled) monitor.
        m.position = Some(Position { x: 0, y: 0 });
        m.scale = None;

        enable_monitor(&mut m);

        assert!(m.enabled);
        assert_eq!(m.position, Some(Position { x: 500, y: 300 }));
        assert_eq!(m.scale, Some(1.25));
    }

    #[test]
    fn enable_keeps_the_current_position_when_nothing_was_saved() {
        let mut m = monitor();
        m.saved_position = None;
        m.saved_scale = None;

        enable_monitor(&mut m);

        assert_eq!(m.position, Some(Position { x: 500, y: 300 }));
        assert_eq!(m.scale, Some(1.25), "current scale is kept");
    }

    #[test]
    fn enable_supplies_a_scale_of_one_when_none_exists() {
        let mut m = monitor();
        m.scale = None;
        m.saved_scale = None;

        enable_monitor(&mut m);

        assert_eq!(m.scale, Some(1.0));
    }

    #[test]
    fn cycle_rotation_walks_through_every_state_and_wraps() {
        let mut m = monitor();
        m.transform = None;

        let expected = ["90", "180", "270", "normal"];
        for want in expected {
            cycle_rotation(&mut m);
            assert_eq!(m.transform.as_deref(), Some(want));
        }
    }

    #[test]
    fn cycle_rotation_recovers_from_an_unknown_transform() {
        let mut m = monitor();
        m.transform = Some("garbage".to_string());

        cycle_rotation(&mut m);

        assert_eq!(m.transform.as_deref(), Some("90"));
    }

    #[test]
    fn set_resolution_switches_the_current_mode() {
        let mut m = monitor();

        set_resolution(&mut m, 1);

        assert_eq!(current_index(&m), 1);
        assert_eq!(
            (m.modes[1].width, m.modes[1].refresh),
            (1280, 75.0),
            "mode data must be untouched"
        );
    }

    #[test]
    fn set_resolution_out_of_bounds_is_ignored() {
        let mut m = monitor();
        set_resolution(&mut m, 1);

        set_resolution(&mut m, 99);

        assert_eq!(current_index(&m), 1, "selection must survive");
        assert_eq!(m.modes.len(), 2);
    }

    #[test]
    fn set_scale_overwrites_any_previous_value() {
        let mut m = monitor();

        set_scale(&mut m, 2.0);
        assert_eq!(m.scale, Some(2.0));

        set_scale(&mut m, 0.5);
        assert_eq!(m.scale, Some(0.5));
    }
}
