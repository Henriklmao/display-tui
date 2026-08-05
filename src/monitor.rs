use crate::rotation::Rotation;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::io::Write;
use ratatui::layout::Rect;
#[derive(Debug,Default, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Position{
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Resolution {
    pub width: i32,
    pub height: i32,
    pub refresh: f32,
    pub preferred: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitorCanvas{
    pub top: i32,
    pub x_bounds: [f64; 2],
    pub y_bounds: [f64; 2],
    pub offset_y: i32,
}



#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HyprMonitor {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    make: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    serial: String,
    width: i32,
    height: i32,
    refresh_rate: f32,
    x: i32,
    y: i32,
    scale: f32,
    transform: i32,
    disabled: bool,
    #[serde(default)]
    available_modes: Vec<String>,
    active_workspace: Option<HyprWorkspace>,
}

#[derive(Debug, Deserialize)]
struct HyprWorkspace {
    id: i32,
    #[allow(dead_code)]
    name: String,
}

impl Monitor {

    pub fn get_monitors() -> Vec<Monitor> {
        let output = Command::new("hyprctl")
            .args(["monitors", "all", "-j"])
            .output().expect("Failed to execute hyprctl command");
        let stdout = String::from_utf8(output.stdout).expect("Failed to convert output to string");
        Self::parse_hyprctl_output(&stdout)
    }

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

                let current = width == hm.width && height == hm.height && (refresh - hm.refresh_rate).abs() < 0.1;
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
            }.to_string();

            let workspace = hm.active_workspace.and_then(|w| {
                if w.id > 0 { Some(w.id as u8) } else { None }
            });

            let make_opt = if hm.make.is_empty() { None } else { Some(hm.make) };
            let model_opt = if hm.model.is_empty() { None } else { Some(hm.model) };
            let serial_opt = if hm.serial.is_empty() { None } else { Some(hm.serial) };
            let desc_opt = if hm.description.is_empty() { None } else { Some(hm.description) };

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
    pub fn get_monitors_canvas(monitors: &Vec<Monitor>, _area: &Rect) -> MonitorCanvas {
        let mut left = 10000.0;
        let mut bottom = 10000.0;
        let mut right = -10000.0;
        let mut top = -10000.0;

        for monitor in monitors {
            if !monitor.enabled {
                continue;
            }
            let mut mode = monitor.get_current_resolution();
            if mode.is_none() {
                mode = monitor.get_prefered_resolution();
            }

            let rotation = Rotation::from_transform(&monitor.transform);
            let (width, height) = if rotation == Rotation::Deg90 || rotation == Rotation::Deg270 {
                (mode.unwrap().height, mode.unwrap().width)
            } else {
                (mode.unwrap().width, mode.unwrap().height)
            };

            let monitor_left = monitor.position.clone().unwrap().x as f64;
            let monitor_right = monitor_left  + (width as f64 / monitor.scale.unwrap() as f64);

            let monitor_bottom = monitor.position.clone().unwrap().y as f64;
            let monitor_top = monitor_bottom + (height as f64 / monitor.scale.unwrap() as f64);
            
            if monitor_right > right {
                right= monitor_right;
            }
            if monitor_top > top {
                top= monitor_top;
            }
            if monitor_left < left {
                left= monitor_left;
            }
            if monitor_bottom < bottom {
                bottom= monitor_bottom;
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

    pub fn get_current_resolution(&self) -> Option<&Resolution> {
        self.modes
            .iter()
            .find(|m| m.current)
    }

    pub fn get_prefered_resolution(&self) -> Option<&Resolution> {
        self.modes
            .iter()
            .find(|m| m.preferred)
    }
    
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

    pub fn to_hyprland_config(&self) -> String {
        let mode = match self.get_current_resolution() {
            Some(m) => m,
            None => {
                self.get_prefered_resolution().expect("No preferred resolution found")
            }
        };
        if self.enabled {
            let rotation = Rotation::from_transform(&self.transform);
            format!(
                "monitor = {}, {}x{}@{}, {}x{}, {}, {}",
                self.name,
                mode.width, mode.height, mode.refresh,
                self.position.clone().unwrap().x, self.position.clone().unwrap().y,
                self.scale.unwrap_or(1.0),
                rotation.to_hyprland()
            )
        } else {
            format!(
                "monitor = {}, disabled",
                self.name
            )
        }
        
    }

    pub fn to_hyprland_lua_config(&self) -> String {
        if !self.enabled {
            return format!("hl.monitor({{\n  output = \"{}\",\n  disabled = true\n}})", self.name);
        }

        let mode = match self.get_current_resolution() {
            Some(m) => m,
            None => self.get_prefered_resolution().expect("No preferred resolution found"),
        };

        let rotation = Rotation::from_transform(&self.transform);
        let scale = self.scale.unwrap_or(1.0);
        let pos_x = self.position.clone().unwrap_or(Position { x: 0, y: 0 }).x;
        let pos_y = self.position.clone().unwrap_or(Position { x: 0, y: 0 }).y;

        format!(
            "hl.monitor({{\n  output = \"{}\",\n  mode = \"{}x{}@{}\",\n  position = \"{}x{}\",\n  scale = {},\n  transform = {}\n}})",
            self.name,
            mode.width, mode.height, mode.refresh,
            pos_x, pos_y,
            scale,
            rotation.to_hyprland_lua()
        )
    }

    pub fn to_hyprland_lua_workspace_rule(&self) -> Option<String> {
        self.workspace.map(|ws| {
            format!("hl.workspace_rule({{ workspace = \"{}\", monitor = \"{}\", default = true }})", ws, self.name)
        })
    }

    pub fn save_hyprland_config(path: &String, monitors: &Vec<Monitor>) -> std::io::Result<()> {
        let hyprland_lua_path = shellexpand::tilde("~/.config/hypr/hyprland.lua").to_string();
        
        if std::path::Path::new(&hyprland_lua_path).exists() {
            // Use Lua formatting
            let content = std::fs::read_to_string(&hyprland_lua_path)?;
            let mut monitors_module = None;
            
            for line in content.lines() {
                if line.contains("require") && line.contains("monitor") {
                    let parts: Vec<&str> = line.split('"').collect();
                    if parts.len() >= 3 {
                        monitors_module = Some(parts[1].to_string());
                    } else {
                        let parts: Vec<&str> = line.split('\'').collect();
                        if parts.len() >= 3 {
                            monitors_module = Some(parts[1].to_string());
                        }
                    }
                }
            }
            
            let module_name = monitors_module.unwrap_or_else(|| {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&hyprland_lua_path)
                    .unwrap();
                let _ = writeln!(file, "\nrequire(\"lua.monitors\")");
                "lua.monitors".to_string()
            });
            
            let relative_path = if module_name == "hypr.monitors" {
                "monitors.lua".to_string()
            } else {
                module_name.replace(".", "/") + ".lua"
            };
            let hypr_dir = shellexpand::tilde("~/.config/hypr/").to_string();
            let final_path = std::path::Path::new(&hypr_dir).join(relative_path);
            
            if let Some(parent) = final_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(final_path)?;
                
            writeln!(file, "-- Monitors generated by display-tui")?;
            for monitor in monitors {
                let config_line = monitor.to_hyprland_lua_config();
                writeln!(file, "{}", config_line)?;
            }
            
            let mut has_workspaces = false;
            for monitor in monitors {
                if let Some(rule) = monitor.to_hyprland_lua_workspace_rule() {
                    if !has_workspaces {
                        writeln!(file, "\n-- Workspace assignments")?;
                        has_workspaces = true;
                    }
                    writeln!(file, "{}", rule)?;
                }
            }
        } else {
            // Use legacy .conf formatting
            let expanded_path = shellexpand::tilde(path).to_string();
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(expanded_path)?;
            for monitor in monitors {
                let config_line = monitor.to_hyprland_config();
                writeln!(file, "{}", config_line)?;
            }
            
            let mut has_workspaces = false;
            for monitor in monitors {
                if monitor.workspace.is_some() {
                    if !has_workspaces {
                        writeln!(file, "\n# Workspace assignments")?;
                        has_workspaces = true;
                    }
                    writeln!(file, "workspace = {}, monitor:{}", monitor.workspace.unwrap(), monitor.name)?;
                }
            }
        }
        Ok(())
    }

    pub fn move_vertical(&mut self, direction: i32) {
        if let Some(ref mut pos) = self.position { pos.y += direction};
    }

    pub fn move_horizontal(&mut self, direction: i32) {
        if let Some(ref mut pos) = self.position { pos.x += direction};
    }

    pub fn get_geometry(&self) -> (f64, f64, f64, f64) {
        let mut mode = self.get_current_resolution();
        if mode.is_none() {
            mode = self.get_prefered_resolution();
        }
        
        if mode.is_none() { return (0.0,0.0,0.0,0.0); }

        let rotation = Rotation::from_transform(&self.transform);
        let (width, height) = if rotation == Rotation::Deg90 || rotation == Rotation::Deg270 {
            (mode.unwrap().height, mode.unwrap().width)
        } else {
            (mode.unwrap().width, mode.unwrap().height)
        };

        let scale = self.scale.unwrap_or(1.0);
        let logical_width = width as f64 / scale as f64;
        let logical_height = height as f64 / scale as f64;
        let x = self.position.clone().unwrap().x as f64;
        let y = self.position.clone().unwrap().y as f64;

        (x, y, logical_width, logical_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(m.enabled, true);
    }
}
