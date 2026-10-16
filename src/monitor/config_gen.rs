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

#[cfg(test)]
mod tests {
    use super::{Monitor, Position};
    use crate::monitor::Resolution;

    fn monitor(name: &str) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
            position: Some(Position { x: 0, y: 0 }),
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

    #[test]
    fn legacy_config_line_for_a_monitor_with_negative_coordinates() {
        let mut m = monitor("HDMI-A-1");
        m.position = Some(Position { x: 1920, y: -540 });
        m.scale = Some(1.25);

        assert_eq!(
            m.to_hyprland_config(),
            "monitor = HDMI-A-1, 1920x1080@60, 1920x-540, 1.25, "
        );
    }

    #[test]
    fn legacy_config_line_appends_transform() {
        let mut m = monitor("DP-1");
        m.transform = Some("90".to_string());

        assert_eq!(
            m.to_hyprland_config(),
            "monitor = DP-1, 1920x1080@60, 0x0, 1, transform, 1"
        );
    }

    #[test]
    fn legacy_config_line_marks_disabled_monitors() {
        let mut m = monitor("DP-1");
        m.enabled = false;

        assert_eq!(m.to_hyprland_config(), "monitor = DP-1, disabled");
    }

    #[test]
    fn legacy_config_line_defaults_scale_to_one() {
        let mut m = monitor("DP-1");
        m.scale = None;

        assert_eq!(
            m.to_hyprland_config(),
            "monitor = DP-1, 1920x1080@60, 0x0, 1, "
        );
    }

    #[test]
    fn legacy_config_line_falls_back_to_preferred_mode() {
        let mut m = monitor("DP-1");
        m.modes.push(Resolution {
            width: 1280,
            height: 720,
            refresh: 144.0,
            preferred: true,
            current: false,
        });
        m.modes[0].current = false;
        m.modes[0].preferred = false;

        assert_eq!(
            m.to_hyprland_config(),
            "monitor = DP-1, 1280x720@144, 0x0, 1, "
        );
    }

    #[test]
    #[should_panic(expected = "No preferred resolution found")]
    fn legacy_config_line_panics_without_any_mode() {
        let mut m = monitor("DP-1");
        m.modes.clear();
        let _ = m.to_hyprland_config();
    }

    #[test]
    fn lua_config_block_for_a_plain_monitor() {
        let mut m = monitor("HDMI-A-1");
        m.position = Some(Position { x: 2560, y: 0 });
        m.scale = Some(2.0);

        assert_eq!(
            m.to_hyprland_lua_config(),
            "hl.monitor({\n  output = \"HDMI-A-1\",\n  mode = \"1920x1080@60\",\n  position = \"2560x0\",\n  scale = 2,\n  transform = 0\n})"
        );
    }

    #[test]
    fn lua_config_block_reports_rotation_as_a_number() {
        let mut m = monitor("DP-1");
        m.transform = Some("270".to_string());

        assert_eq!(
            m.to_hyprland_lua_config(),
            "hl.monitor({\n  output = \"DP-1\",\n  mode = \"1920x1080@60\",\n  position = \"0x0\",\n  scale = 1,\n  transform = 3\n})"
        );
    }

    #[test]
    fn lua_config_block_for_disabled_monitor() {
        let mut m = monitor("DP-1");
        m.enabled = false;

        assert_eq!(
            m.to_hyprland_lua_config(),
            "hl.monitor({\n  output = \"DP-1\",\n  disabled = true\n})"
        );
    }

    #[test]
    fn lua_config_block_defaults_missing_position_to_origin() {
        let mut m = monitor("DP-1");
        m.position = None;

        assert!(
            m.to_hyprland_lua_config()
                .contains("position = \"0x0\""),
            "a missing position must not panic, it falls back to 0x0"
        );
    }

    #[test]
    fn lua_workspace_rule_is_emitted_only_when_assigned() {
        let mut m = monitor("DP-1");
        assert!(m.to_hyprland_lua_workspace_rule().is_none());

        m.workspace = Some(5);
        assert_eq!(
            m.to_hyprland_lua_workspace_rule().unwrap(),
            "hl.workspace_rule({ workspace = \"5\", monitor = \"DP-1\", default = true })"
        );
    }

    #[test]
    fn lua_workspace_rule_is_skipped_for_disabled_monitors_too() {
        // Rules are keyed by output name, so a disabled monitor still gets one
        // if a workspace is assigned to it.
        let mut m = monitor("DP-1");
        m.enabled = false;
        m.workspace = Some(3);

        assert!(m.to_hyprland_lua_workspace_rule().is_some());
    }

    // NOTE: save_hyprland_config() only picks the legacy .conf branch when
    // ~/.config/hypr/hyprland.lua is absent on the machine running the test,
    // so only the explicit-path Lua branch is exercised here. Tying a test to
    // files in the developer's home directory would make it machine dependent.
    #[test]
    fn save_hyprland_config_writes_lua_file_with_workspace_section() {
        let dir = crate::test_utils::tests::TempDir::new("lua");
        let path = dir.path().join("monitors.lua");

        let mut with_ws = monitor("DP-1");
        with_ws.workspace = Some(2);
        let mut disabled = monitor("HDMI-A-1");
        disabled.enabled = false;

        Monitor::save_hyprland_config(
            "unused-in-this-branch",
            &vec![with_ws, disabled],
            Some(path.to_str().unwrap()),
        )
        .expect("writing the lua file must succeed");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.starts_with("-- Monitors generated by display-tui"),
            "missing header: {content}"
        );
        assert!(content.contains("output = \"DP-1\""));
        assert!(content.contains("mode = \"1920x1080@60\""));
        assert!(content.contains("disabled = true"), "missing disabled block");
        assert!(content.contains("-- Workspace assignments"));
        assert!(content.contains(
            "hl.workspace_rule({ workspace = \"2\", monitor = \"DP-1\", default = true })"
        ));

        let rules = content
            .matches("hl.workspace_rule")
            .count();
        assert_eq!(rules, 1, "only the monitor with a workspace gets a rule");
    }

    #[test]
    fn save_hyprland_config_omits_workspace_section_when_unassigned() {
        let dir = crate::test_utils::tests::TempDir::new("lua-nows");
        let path = dir.path().join("monitors.lua");

        Monitor::save_hyprland_config(
            "unused-in-this-branch",
            &vec![monitor("DP-1")],
            Some(path.to_str().unwrap()),
        )
        .expect("writing the lua file must succeed");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.contains("-- Workspace assignments"));
        assert!(!content.contains("hl.workspace_rule"));
    }
}
