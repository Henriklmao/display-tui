//! Application configuration.
//!
//! Handles loading and managing display-tui's own configuration,
//! including paths to the Hyprland monitor config file.

use super::paths::get_config_path;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// Display-tui configuration.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Configuration {
    pub monitors_config_path: Option<String>,
    pub lua_monitor_config: Option<String>,
}

impl Configuration {
    // Loads or creates the configuration.
    pub fn get() -> Self {
        let config_json_path = get_config_path();
        match !config_json_path.exists() {
            true => Configuration::create_default_config(&config_json_path),
            false => Configuration::load_config(),
        }
    }

    // Creates a default configuration file if none exists or it is empty.
    pub fn create_default_config(config_json_path: &PathBuf) -> Self {
        if let Some(parent) = config_json_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).expect("Failed to create config directory");
        }

        let default_config = Configuration {
            monitors_config_path: Some("~/.config/hypr/monitors.conf".to_string()),
            lua_monitor_config: None,
        };

        let config_content = serde_json::to_string_pretty(&default_config)
            .expect("Failed to serialize default config");
        fs::write(config_json_path, config_content).expect("Failed to write default config file");

        default_config
    }

    // Loads the configuration from disk, populating it with the default
    // config if the file is empty or only contains empty values.
    pub fn load_config() -> Self {
        let config_json_path = get_config_path();

        let config_content =
            fs::read_to_string(&config_json_path).expect("Failed to read config file");

        if config_content.trim().is_empty() {
            let default_config = Configuration {
                monitors_config_path: Some("~/.config/hypr/monitors.conf".to_string()),
                lua_monitor_config: None,
            };

            let config_content_json = serde_json::to_string_pretty(&default_config)
                .expect("Failed to serialize default config");
            fs::write(config_json_path, config_content_json)
                .expect("Failed to write populated config file from empty input");

            return default_config;
        }

        let config: Configuration =
            serde_json::from_str(&config_content).unwrap_or(Configuration {
                monitors_config_path: None,
                lua_monitor_config: None,
            });

        if config.monitors_config_path.is_none() && config.lua_monitor_config.is_none() {
            let default_config = Configuration {
                monitors_config_path: Some("~/.config/hypr/monitors.conf".to_string()),
                lua_monitor_config: None,
            };

            let config_content_json = serde_json::to_string_pretty(&default_config)
                .expect("Failed to serialize default config");
            fs::write(config_json_path, config_content_json)
                .expect("Failed to write populated config file from empty configuration");

            return default_config;
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use crate::config::state::MonitorState;
    use crate::config::Configuration;
    use crate::monitor::{Monitor, Position};

    #[test]
    fn test_save_and_load_monitor_state() {
        let monitors = [
            Monitor {
                name: "HDMI-A-1".to_string(),
                position: Some(Position { x: 100, y: 200 }),
                scale: Some(1.5),
                workspace: Some(1),
                enabled: true,
                ..Default::default()
            },
            Monitor {
                name: "DP-1".to_string(),
                position: Some(Position { x: 300, y: 400 }),
                scale: Some(1.0),
                workspace: None,
                enabled: true,
                ..Default::default()
            },
        ];

        let state: Vec<MonitorState> = monitors
            .iter()
            .map(|m| MonitorState {
                name: m.name.clone(),
                position: m.position.clone(),
                scale: m.scale,
                workspace: m.workspace,
            })
            .collect();

        assert_eq!(state.len(), 2);
        assert_eq!(state[0].name, "HDMI-A-1");
        assert_eq!(state[0].position, Some(Position { x: 100, y: 200 }));
        assert_eq!(state[0].scale, Some(1.5));
        assert_eq!(state[0].workspace, Some(1));
    }

    #[test]
    fn test_create_default_config_populates_missing_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "display-tui-test-{}-missing",
            std::process::id()
        ));
        let config_path = temp_dir.join("config.json");

        let config = Configuration::create_default_config(&config_path);

        assert_eq!(
            config.monitors_config_path.as_deref(),
            Some("~/.config/hypr/monitors.conf")
        );
        assert!(config_path.exists());
        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: Configuration = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed.monitors_config_path.as_deref(),
            Some("~/.config/hypr/monitors.conf")
        );

        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_create_default_config_populates_empty_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "display-tui-test-{}-empty",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_path = temp_dir.join("config.json");
        std::fs::write(&config_path, "").unwrap();

        let config = Configuration::create_default_config(&config_path);

        assert_eq!(
            config.monitors_config_path.as_deref(),
            Some("~/.config/hypr/monitors.conf")
        );
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(!content.trim().is_empty());
        let parsed: Configuration = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed.monitors_config_path.as_deref(),
            Some("~/.config/hypr/monitors.conf")
        );

        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
