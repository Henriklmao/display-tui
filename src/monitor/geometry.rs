//! Canvas geometry calculations for monitor layout rendering.

use ratatui::layout::Rect;
use super::types::{Monitor, MonitorCanvas};

impl Monitor {
    // Computes the bounding canvas for all enabled monitors.
    //
    // Returns a `MonitorCanvas` that defines the 2D bounds and offsets needed
    // to render monitors within a given terminal area.
    pub fn get_monitors_canvas(monitors: &Vec<Monitor>, _area: &Rect) -> MonitorCanvas {
        let mut left = 10000.0;
        let mut bottom = 10000.0;
        let mut right = -10000.0;
        let mut top = -10000.0;

        for monitor in monitors {
            if !monitor.enabled {
                continue;
            }
            let (width, height) = monitor.get_logical_dimensions();

            let monitor_left = monitor.position.clone().unwrap().x as f64;
            let monitor_right = monitor_left + width;

            let monitor_bottom = monitor.position.clone().unwrap().y as f64;
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

        let mut offset_y = 0.0;
        if bottom < 0.0 {
            offset_y = -bottom;
        }

        MonitorCanvas {
            top: top as i32,
            x_bounds,
            y_bounds,
            offset_y: offset_y as i32,
        }
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
        let canvas = Monitor::get_monitors_canvas(&vec![monitor("A", 0, 0, 1.0, true)], &area());

        assert_eq!(canvas.x_bounds, [-MARGIN, 1920.0 + MARGIN]);
        assert_eq!(canvas.y_bounds, [-MARGIN, 1080.0 + MARGIN]);
        assert_eq!(canvas.top, (1080.0 + MARGIN) as i32);
        assert_eq!(canvas.offset_y, MARGIN as i32, "origin sits above 0");
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
        assert_eq!(canvas.top, (2160.0 + MARGIN) as i32);
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
    fn offset_y_lifts_negative_bottom_to_the_origin() {
        let monitors = vec![monitor("A", 0, -1080, 1.0, true)];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        // bottom = -1080 - 50, so the offset has to cancel it out.
        assert_eq!(canvas.offset_y, (1080.0 + MARGIN) as i32);
        assert_eq!(canvas.y_bounds[0], -(1080.0 + MARGIN));
    }

    #[test]
    fn offset_y_is_zero_when_bottom_is_already_non_negative() {
        // y = 100 keeps the margin-adjusted bottom at +50, so no lift is needed.
        let monitors = vec![monitor("A", 0, 100, 1.0, true)];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        assert_eq!(canvas.y_bounds[0], 100.0 - MARGIN);
        assert_eq!(canvas.offset_y, 0);
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
    fn bounds_use_logical_size_after_scaling() {
        let monitors = vec![monitor("A", 0, 0, 2.0, true)];

        let canvas = Monitor::get_monitors_canvas(&monitors, &area());

        assert_eq!(canvas.x_bounds, [-MARGIN, 960.0 + MARGIN]);
        assert_eq!(canvas.y_bounds, [-MARGIN, 540.0 + MARGIN]);
        assert_eq!(canvas.top, (540.0 + MARGIN) as i32);
    }

    #[test]
    fn canvas_grows_monotonically_with_more_monitors() {
        let one = Monitor::get_monitors_canvas(&vec![monitor("A", 0, 0, 1.0, true)], &area());
        let two = Monitor::get_monitors_canvas(
            &vec![
                monitor("A", 0, 0, 1.0, true),
                monitor("B", 1920, 0, 1.0, true),
            ],
            &area(),
        );

        assert!(two.x_bounds[1] > one.x_bounds[1]);
        assert!(two.top >= one.top);
    }
}
