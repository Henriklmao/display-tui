//! Monitor discovery and JSON parsing.
//!
//! Handles fetching monitor data from hyprctl and parsing its JSON output.

use std::process::Command;
use super::types::{HyprMonitor, Monitor, Position, Resolution};

impl Monitor {
    // Fetches the current monitor configuration from hyprctl.
    pub fn get_monitors() -> Vec<Monitor> {
        let output = Command::new("hyprctl")
            .args(["monitors", "all", "-j"])
            .output()
            .expect("Failed to execute hyprctl command");
        let stdout =
            String::from_utf8(output.stdout).expect("Failed to convert output to string");
        Self::parse_hyprctl_output(&stdout)
    }

    // Parses hyprctl JSON output into Monitor structs.
    pub fn parse_hyprctl_output(json: &str) -> Vec<Monitor> {
        let hypr_monitors: Vec<HyprMonitor> = match serde_json::from_str(json) {
            Ok(monitors) => monitors,
            Err(e) => {
                eprintln!("Deserialization error: {}", e);
                return Vec::new();
            }
        };

        let mut new_monitors = Vec::new();
        for hm in hypr_monitors {
            let mut modes = Vec::new();
            let mut found_current = false;

            for (i, mode_str) in hm.available_modes.iter().enumerate() {
                let mut width = 0;
                let mut height = 0;
                let mut refresh = 0.0;
                if let Some((w_str, rest)) = mode_str.split_once('x') {
                    width = w_str.parse().unwrap_or(0);
                    if let Some((h_str, r_str)) = rest.split_once('@') {
                        height = h_str.parse().unwrap_or(0);
                        refresh = r_str.trim_end_matches("Hz").parse().unwrap_or(0.0);
                    }
                }

                let current = width == hm.width
                    && height == hm.height
                    && (refresh - hm.refresh_rate).abs() < 0.1;
                if current {
                    found_current = true;
                }

                modes.push(Resolution {
                    width,
                    height,
                    refresh,
                    preferred: i == 0,
                    current,
                });
            }

            if !found_current && !modes.is_empty() && !hm.disabled {
                modes[0].current = true;
            }

            let transform_str = match hm.transform {
                1 | 5 => "90",
                2 | 6 => "180",
                3 | 7 => "270",
                _ => "normal",
            }
            .to_string();

            let workspace = hm.active_workspace.and_then(|w| {
                if w.id > 0 {
                    Some(w.id as u8)
                } else {
                    None
                }
            });

            let make_opt = if hm.make.is_empty() {
                None
            } else {
                Some(hm.make)
            };
            let model_opt = if hm.model.is_empty() {
                None
            } else {
                Some(hm.model)
            };
            let serial_opt = if hm.serial.is_empty() {
                None
            } else {
                Some(hm.serial)
            };
            let desc_opt = if hm.description.is_empty() {
                None
            } else {
                Some(hm.description)
            };

            new_monitors.push(Monitor {
                name: hm.name,
                description: desc_opt,
                make: make_opt,
                model: model_opt,
                serial: serial_opt,
                enabled: !hm.disabled,
                modes,
                position: Some(Position { x: hm.x, y: hm.y }),
                scale: Some(hm.scale),
                transform: Some(transform_str),
                workspace,
                saved_position: None,
                saved_scale: None,
            });
        }

        new_monitors
    }
}

#[cfg(test)]
mod tests {
    use super::Monitor;

    #[test]
    fn test_parse_hyprctl_output() {
        let json = r#"[{
            "id": 1,
            "name": "HDMI-A-1",
            "description": "Samsung",
            "make": "Samsung",
            "model": "S22C300",
            "serial": "123",
            "width": 1920,
            "height": 1080,
            "refreshRate": 60.0,
            "x": 0,
            "y": 0,
            "scale": 1.0,
            "transform": 0,
            "disabled": false,
            "availableModes": ["1920x1080@60.00Hz", "1280x720@60.00Hz"],
            "activeWorkspace": { "id": 5, "name": "5" }
        }]"#;

        let monitors = Monitor::parse_hyprctl_output(json);
        assert_eq!(monitors.len(), 1);
        let m = &monitors[0];
        assert_eq!(m.name, "HDMI-A-1");
        assert_eq!(m.modes.len(), 2);
        assert!(m.modes[0].current);
        assert!(m.modes[0].preferred);
        assert_eq!(m.modes[0].width, 1920);
        assert_eq!(m.modes[0].height, 1080);
        assert_eq!(m.modes[0].refresh, 60.0);
        assert_eq!(m.transform.as_deref(), Some("normal"));
        assert_eq!(m.workspace, Some(5));
        assert!(m.enabled);
    }
}
