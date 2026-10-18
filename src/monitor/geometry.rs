//! Canvas geometry calculations for monitor layout rendering.

use super::types::{Monitor, MonitorCanvas};
use ratatui::layout::Rect;

impl Monitor {
    // Computes the bounding canvas for all enabled monitors.
    //
    // Returns a `MonitorCanvas` that defines the 2D bounds needed to render
    // monitors within a given terminal area.
    pub fn get_monitors_canvas(monitors: &[Monitor], _area: &Rect) -> MonitorCanvas {
        let mut left = 10000.0;
        let mut bottom = 10000.0;
        let mut right = -10000.0;
        let mut top = -10000.0;

        for monitor in monitors {
            if !monitor.enabled {
                continue;
            }
            // A monitor without a position cannot be placed on the canvas.
            let position = match &monitor.position {
                Some(position) => position,
                None => continue,
            };
            let (width, height) = monitor.get_logical_dimensions();

            let monitor_left = position.x as f64;
            let monitor_right = monitor_left + width;

            let monitor_bottom = position.y as f64;
            let monitor_top = monitor_bottom + height;

            if monitor_right > right {
                right = monitor_right;
            }
            if monitor_top > top {
                top = monitor_top;
            }
            if monitor_left < left {
                left = monitor_left;
            }
            if monitor_bottom < bottom {
                bottom = monitor_bottom;
            }
        }

        let margin = 50.0;
        left -= margin;
        bottom -= margin;
        right += margin;
        top += margin;

        let x_bounds = [left, right];
        let y_bounds = [bottom, top];

        MonitorCanvas { x_bounds, y_bounds }
    }
}

#[cfg(test)]
mod tests {
    use super::Monitor;
    use crate::monitor::{Position, Resolution};
    use ratatui::layout::Rect;

    const MARGIN: f64 = 50.0;

    fn monitor(name: &str, x: i32, y: i32, scale: f32, enabled: bool) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled,
            position: Some(Position { x, y }),
            scale: Some(scale),
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

    fn area() -> Rect {
        Rect::new(0, 0, 80, 24)
    }

    #[test]
    fn single_monitor_bounds_include_margin() {
        let canvas = Monitor::get_monitors_canvas(&[monitor("A", 0, 0, 1.0, true)], &area());

        assert_eq!(canvas.x_bounds, [-MARGIN, 1920.0 + MARGIN]);
        // y = 0 sits exactly one margin above the bottom of the canvas: the
        // margin alone must not push any origin-lifting bookkeeping around.
        assert_eq!(canvas.y_bounds, [-MARGIN, 1080.0 + MARGIN]);
    }

    #[test]
    fn bounds_span_all_enabled_monitors() {
        let monitors = vec![
            monitor("A", 0, 0, 1.0, true),
            monitor("B", 1920, 0, 1.0, true),
            monitor("C", 0, 1080, 1.0, true),
        ];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        assert_eq!(canvas.x_bounds, [-MARGIN, 3840.0 + MARGIN]);
        assert_eq!(canvas.y_bounds, [-MARGIN, 2160.0 + MARGIN]);
    }

    #[test]
    fn bounds_span_negative_coordinates() {
        let monitors = vec![
            monitor("A", -1920, -1080, 1.0, true),
            monitor("B", 0, 0, 1.0, true),
        ];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        assert_eq!(canvas.x_bounds, [-1920.0 - MARGIN, 1920.0 + MARGIN]);
        assert_eq!(canvas.y_bounds, [-1080.0 - MARGIN, 1080.0 + MARGIN]);
    }

    #[test]
    fn negative_monitors_keep_the_margin_on_both_sides() {
        let monitors = vec![monitor("A", 0, -1080, 1.0, true)];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        assert_eq!(canvas.y_bounds, [-(1080.0 + MARGIN), MARGIN]);
    }

    #[test]
    fn disabled_monitors_are_ignored() {
        let monitors = vec![
            monitor("A", 0, 0, 1.0, true),
            // Far away and disabled: must not stretch the canvas.
            monitor("B", 100_000, 100_000, 1.0, false),
        ];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        assert_eq!(canvas.x_bounds, [-MARGIN, 1920.0 + MARGIN]);
        assert_eq!(canvas.y_bounds, [-MARGIN, 1080.0 + MARGIN]);
    }

    #[test]
    fn monitors_without_a_position_are_ignored() {
        let mut placed = monitor("A", 0, 0, 1.0, true);
        placed.position = Some(Position { x: -1920, y: 0 });
        let mut unplaced = monitor("B", 0, 0, 1.0, true);
        unplaced.position = None;

        let canvas = Monitor::get_monitors_canvas(&[unplaced, placed], &area());

        assert_eq!(canvas.x_bounds, [-1920.0 - MARGIN, MARGIN]);
        assert_eq!(canvas.y_bounds, [-MARGIN, 1080.0 + MARGIN]);
    }

    #[test]
    fn bounds_use_logical_size_after_scaling() {
        let monitors = vec![monitor("A", 0, 0, 2.0, true)];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        assert_eq!(canvas.x_bounds, [-MARGIN, 960.0 + MARGIN]);
        assert_eq!(canvas.y_bounds, [-MARGIN, 540.0 + MARGIN]);
    }

    #[test]
    fn canvas_grows_monotonically_with_more_monitors() {
        let one = Monitor::get_monitors_canvas(&[monitor("A", 0, 0, 1.0, true)], &area());
        let two = Monitor::get_monitors_canvas(
            &[
                monitor("A", 0, 0, 1.0, true),
                monitor("B", 1920, 0, 1.0, true),
            ],
            &area(),
        );

        assert!(two.x_bounds[1] > one.x_bounds[1]);
        assert!(two.y_bounds[1] >= one.y_bounds[1]);
    }
}
