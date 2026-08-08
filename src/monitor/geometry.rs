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
