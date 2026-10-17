//! Monitor discovery and JSON parsing.
//!
//! Handles fetching monitor data from hyprctl and parsing its JSON output.

use super::types::{HyprMonitor, Monitor, Position, Resolution};
use std::process::Command;

impl Monitor {
    // Fetches the current monitor configuration from hyprctl.
    pub fn get_monitors() -> Vec<Monitor> {
        let output = Command::new("hyprctl")
            .args(["monitors", "all", "-j"])
            .output()
            .expect("Failed to execute hyprctl command");
        let stdout = String::from_utf8(output.stdout).expect("Failed to convert output to string");
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

            let workspace = hm
                .active_workspace
                .and_then(|w| if w.id > 0 { Some(w.id as u8) } else { None });

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
    use serde_json::{Value, json};

    // A minimal but valid hyprctl monitor entry; tests tweak single fields.
    fn base() -> Value {
        json!({
            "name": "DP-1",
            "description": "",
            "make": "",
            "model": "",
            "serial": "",
            "width": 1920,
            "height": 1080,
            "refreshRate": 60.0,
            "x": 0,
            "y": 0,
            "scale": 1.0,
            "transform": 0,
            "disabled": false,
            "availableModes": ["1920x1080@60.00Hz"],
            "activeWorkspace": null
        })
    }

    fn parse_one(mut entry: Value) -> Monitor {
        entry["name"] = json!("DP-1");
        let json = json!([entry]).to_string();
        let mut monitors = Monitor::parse_hyprctl_output(&json);
        assert_eq!(monitors.len(), 1, "expected exactly one monitor");
        monitors.pop().unwrap()
    }

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

    #[test]
    fn transform_codes_map_to_rotation_strings() {
        let expected = ["normal", "90", "180", "270", "normal", "90", "180", "270"];

        let entries: Vec<Value> = (0..8)
            .map(|t| {
                let mut entry = base();
                entry["transform"] = json!(t);
                entry
            })
            .collect();

        let monitors = Monitor::parse_hyprctl_output(&json!(entries).to_string());

        assert_eq!(monitors.len(), 8);
        for (i, monitor) in monitors.iter().enumerate() {
            assert_eq!(
                monitor.transform.as_deref(),
                Some(expected[i]),
                "transform {} parsed incorrectly",
                i
            );
        }
    }

    #[test]
    fn position_scale_and_identity_are_carried_over() {
        let mut entry = base();
        entry["name"] = json!("HDMI-A-1");
        entry["description"] = json!("Dell U2720Q");
        entry["make"] = json!("Dell");
        entry["model"] = json!("U2720Q");
        entry["serial"] = json!("ABC123");
        entry["x"] = json!(-1920);
        entry["y"] = json!(240);
        entry["scale"] = json!(1.5);

        let json = json!([entry]).to_string();
        let monitors = Monitor::parse_hyprctl_output(&json);
        let m = &monitors[0];

        assert_eq!(m.name, "HDMI-A-1");
        assert_eq!(m.description.as_deref(), Some("Dell U2720Q"));
        assert_eq!(m.make.as_deref(), Some("Dell"));
        assert_eq!(m.model.as_deref(), Some("U2720Q"));
        assert_eq!(m.serial.as_deref(), Some("ABC123"));
        let position = m.position.clone().expect("position is set");
        assert_eq!((position.x, position.y), (-1920, 240));
        assert_eq!(m.scale, Some(1.5));
    }

    #[test]
    fn empty_metadata_strings_become_none() {
        let m = parse_one(base());

        assert!(m.description.is_none());
        assert!(m.make.is_none());
        assert!(m.model.is_none());
        assert!(m.serial.is_none());
    }

    #[test]
    fn missing_metadata_fields_are_tolerated() {
        let mut entry = base();
        for field in ["description", "make", "model", "serial", "availableModes"] {
            entry.as_object_mut().unwrap().remove(field);
        }

        let m = parse_one(entry);

        assert!(m.description.is_none());
        assert!(m.make.is_none());
        assert!(m.modes.is_empty());
        assert!(m.enabled);
    }

    #[test]
    fn disabled_monitor_is_marked_and_gets_no_forced_current_mode() {
        let mut entry = base();
        entry["disabled"] = json!(true);
        // Reported resolution matches nothing in availableModes.
        entry["width"] = json!(0);
        entry["height"] = json!(0);
        entry["refreshRate"] = json!(0.0);
        entry["availableModes"] = json!(["1920x1080@60.00Hz", "1280x720@60.00Hz"]);

        let m = parse_one(entry);

        assert!(!m.enabled);
        assert_eq!(m.modes.len(), 2);
        assert!(
            m.modes.iter().all(|mode| !mode.current),
            "a disabled monitor must not be forced onto a mode"
        );
    }

    #[test]
    fn falls_back_to_first_mode_when_nothing_matches() {
        let mut entry = base();
        entry["width"] = json!(0);
        entry["height"] = json!(0);
        entry["refreshRate"] = json!(0.0);
        entry["availableModes"] = json!(["1920x1080@60.00Hz", "1280x720@60.00Hz"]);

        let m = parse_one(entry);

        assert!(m.modes[0].current, "first mode must be selected");
        assert!(!m.modes[1].current);
    }

    #[test]
    fn current_mode_is_matched_by_width_height_and_refresh() {
        let mut entry = base();
        entry["availableModes"] = json!(["1920x1080@60.00Hz", "1920x1080@75.00Hz"]);
        entry["refreshRate"] = json!(75.0);

        let m = parse_one(entry);

        assert!(!m.modes[0].current);
        assert!(m.modes[1].current);
        assert_eq!(m.modes[1].refresh, 75.0);
    }

    #[test]
    fn current_mode_match_tolerates_small_refresh_rounding() {
        let mut entry = base();
        entry["refreshRate"] = json!(59.94);

        let m = parse_one(entry);

        assert!(
            m.modes[0].current,
            "59.94Hz must still match the 60.00Hz mode string"
        );
    }

    #[test]
    fn only_the_first_mode_is_preferred() {
        let mut entry = base();
        entry["availableModes"] =
            json!(["1920x1080@60.00Hz", "1280x720@60.00Hz", "800x600@60.00Hz"]);

        let m = parse_one(entry);

        let preferred: Vec<usize> = m
            .modes
            .iter()
            .enumerate()
            .filter(|(_, mode)| mode.preferred)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(preferred, vec![0]);
    }

    #[test]
    fn workspace_is_none_for_missing_zero_and_special_ids() {
        let no_workspace = parse_one(base());
        assert_eq!(no_workspace.workspace, None);

        let mut zero = base();
        zero["activeWorkspace"] = json!({ "id": 0, "name": "special:scratchpad" });
        assert_eq!(parse_one(zero).workspace, None);

        let mut negative = base();
        negative["activeWorkspace"] = json!({ "id": -99, "name": "special:magic" });
        assert_eq!(parse_one(negative).workspace, None);
    }

    #[test]
    fn workspace_is_captured_for_regular_ids() {
        let mut entry = base();
        entry["activeWorkspace"] = json!({ "id": 7, "name": "7" });

        assert_eq!(parse_one(entry).workspace, Some(7));
    }

    #[test]
    fn malformed_mode_strings_degrade_to_zero_values() {
        let mut entry = base();
        entry["width"] = json!(0);
        entry["height"] = json!(0);
        entry["refreshRate"] = json!(0.0);
        entry["availableModes"] = json!([
            "garbage",
            "1920",
            "1920x1080",
            "1920x1080@",
            "1920x1080@60.00Hz"
        ]);

        let m = parse_one(entry);

        assert_eq!(m.modes.len(), 5);
        assert_eq!((m.modes[0].width, m.modes[0].height), (0, 0));
        assert_eq!(m.modes[0].refresh, 0.0);
        assert_eq!((m.modes[1].width, m.modes[1].height), (0, 0));
        assert_eq!((m.modes[2].width, m.modes[2].height), (1920, 0));
        assert_eq!(m.modes[2].refresh, 0.0);
        assert_eq!((m.modes[3].width, m.modes[3].height), (1920, 1080));
        assert_eq!(m.modes[3].refresh, 0.0);
        assert_eq!((m.modes[4].width, m.modes[4].height), (1920, 1080));
        assert_eq!(m.modes[4].refresh, 60.0);
    }

    #[test]
    fn multiple_monitors_are_parsed_in_order() {
        let entries: Vec<Value> = ["DP-1", "HDMI-A-1", "eDP-1"]
            .iter()
            .map(|name| {
                let mut entry = base();
                entry["name"] = json!(*name);
                entry
            })
            .collect();

        let monitors = Monitor::parse_hyprctl_output(&json!(entries).to_string());

        let names: Vec<&str> = monitors.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["DP-1", "HDMI-A-1", "eDP-1"]);
    }

    #[test]
    fn malformed_json_yields_no_monitors() {
        assert!(Monitor::parse_hyprctl_output("").is_empty());
        assert!(Monitor::parse_hyprctl_output("not json at all").is_empty());
        assert!(Monitor::parse_hyprctl_output("{}").is_empty());
        assert!(Monitor::parse_hyprctl_output("[{\"name\": \"DP-1\"}]").is_empty());
        assert!(Monitor::parse_hyprctl_output("[]").is_empty());
    }

    #[test]
    fn empty_mode_list_is_handled() {
        let mut entry = base();
        entry["availableModes"] = json!([]);

        let m = parse_one(entry);

        assert!(m.modes.is_empty());
        assert!(m.get_best_resolution().is_none());
    }
}
