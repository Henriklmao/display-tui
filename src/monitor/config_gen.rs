//! Hyprland configuration file generation.
//!
//! Generates both legacy .conf and Lua-style monitor configuration files.

use std::io::Write;
use crate::rotation::Rotation;
use super::types::{Monitor, Position};

impl Monitor {
    // Generates a legacy-style hyprland.conf monitor line.
    pub fn to_hyprland_config(&self) -> String {
        let mode = match self.get_current_resolution() {
            Some(m) => m,
            None => self
                .get_prefered_resolution()
                .expect("No preferred resolution found"),
        };
        if self.enabled {
            let rotation = Rotation::from_transform(&self.transform);
            format!(
                "monitor = {}, {}x{}@{}, {}x{}, {}, {}",
                self.name,
                mode.width,
                mode.height,
                mode.refresh,
                self.position.clone().unwrap().x,
                self.position.clone().unwrap().y,
                self.scale.unwrap_or(1.0),
                rotation.to_hyprland()
            )
        } else {
            format!("monitor = {}, disabled", self.name)
        }
    }

    // Generates a Lua-style (Hyprland 0.55+) monitor configuration block.
    pub fn to_hyprland_lua_config(&self) -> String {
        if !self.enabled {
            return format!(
                "hl.monitor({{\n  output = \"{}\",\n  disabled = true\n}})",
                self.name
            );
        }

        let mode = match self.get_current_resolution() {
            Some(m) => m,
            None => self
                .get_prefered_resolution()
                .expect("No preferred resolution found"),
        };

        let rotation = Rotation::from_transform(&self.transform);
        let scale = self.scale.unwrap_or(1.0);
        let pos_x = self.position.clone().unwrap_or(Position { x: 0, y: 0 }).x;
        let pos_y = self.position.clone().unwrap_or(Position { x: 0, y: 0 }).y;

        format!(
            "hl.monitor({{\n  output = \"{}\",\n  mode = \"{}x{}@{}\",\n  position = \"{}x{}\",\n  scale = {},\n  transform = {}\n}})",
            self.name,
            mode.width,
            mode.height,
            mode.refresh,
            pos_x,
            pos_y,
            scale,
            rotation.to_hyprland_lua()
        )
    }

    // Generates a Lua-style workspace rule for this monitor.
    pub fn to_hyprland_lua_workspace_rule(&self) -> Option<String> {
        self.workspace.map(|ws| {
            format!(
                "hl.workspace_rule({{ workspace = \"{}\", monitor = \"{}\", default = true }})",
                ws, self.name
            )
        })
    }

    // Saves the monitor configuration to the Hyprland config file.
    //
    // Supports both legacy `.conf` format and newer Lua-based configuration
    // (Hyprland 0.55+). Automatically detects which format to use based on
    // config settings and the presence of hyprland.lua.
    pub fn save_hyprland_config(
        path: &str,
        monitors: &Vec<Monitor>,
        lua_monitor_config: Option<&str>,
    ) -> std::io::Result<()> {
        let use_lua = if let Some(_lua_path) = lua_monitor_config {
            true
        } else {
            let hyprland_lua_path = shellexpand::tilde("~/.config/hypr/hyprland.lua").to_string();
            std::path::Path::new(&hyprland_lua_path).exists()
        };

        if use_lua {
            let hyprland_lua_path = shellexpand::tilde("~/.config/hypr/hyprland.lua").to_string();
            let lua_config_path = lua_monitor_config
                .map(|s| shellexpand::tilde(s).to_string())
                .unwrap_or_else(|| {
                    // Use Lua formatting, find or require monitors module from hyprland.lua
                    let mut monitors_module = None;
                    if let Ok(content) = std::fs::read_to_string(&hyprland_lua_path) {
                        monitors_module = content
                            .lines()
                            .filter(|l| l.contains("require") && l.contains("monitor"))
                            .find_map(|l| l.split(&['"', '\''][..]).nth(1))
                            .map(|s| s.to_string());
                    }

                    let module_name = monitors_module.unwrap_or_else(|| {
                        if std::path::Path::new(&hyprland_lua_path).exists() {
                            let mut file = std::fs::OpenOptions::new()
                                .append(true)
                                .open(&hyprland_lua_path)
                                .unwrap();
                            let _ = writeln!(file, "\nrequire(\"lua.monitors\")");
                        }
                        "lua.monitors".to_string()
                    });

                    let relative_path = if module_name == "hypr.monitors" {
                        "monitors.lua".to_string()
                    } else {
                        module_name.replace(".", "/") + ".lua"
                    };
                    let hypr_dir = shellexpand::tilde("~/.config/hypr/").to_string();
                    std::path::Path::new(&hypr_dir)
                        .join(relative_path)
                        .to_string_lossy()
                        .into_owned()
                });

            if let Some(parent) = std::path::Path::new(&lua_config_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(&lua_config_path)?;

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
                if let Some(ws) = monitor.workspace {
                    if !has_workspaces {
                        writeln!(file, "\n# Workspace assignments")?;
                        has_workspaces = true;
                    }
                    writeln!(file, "workspace = {}, monitor:{}", ws, monitor.name)?;
                }
            }
        }
        Ok(())
    }
}
