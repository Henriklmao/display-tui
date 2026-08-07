use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use crate::monitor::{Monitor, Position};

#[derive(Debug,Default, Clone, Deserialize)]
pub struct Configuration {
    pub monitors_config_path: Option<String>,
    pub lua_monitor_config: Option<String>,

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorState {
    pub name: String,
    pub position: Option<Position>,
    pub scale: Option<f32>,
    pub workspace: Option<u8>,
}
impl Configuration {
    pub fn get() -> Self {
        let config_json_path = dirs::home_dir()
             .map(|p| p.join(".config/display-tui/config.json"))
             .unwrap_or_else(|| Path::new("~/.config/display-tui/config.json").to_path_buf());
        match !config_json_path.exists() {
            true => {
                Configuration::create_default_config(&config_json_path)
            },
            false => {
                Configuration::load_config()
            }
        }
    }

    pub fn load_monitor_state() -> Option<Vec<MonitorState>> {
        let state_path = dirs::home_dir()
            .map(|p| p.join(".config/display-tui/monitor_state.json"))
            .unwrap_or_else(|| Path::new("~/.config/display-tui/monitor_state.json").to_path_buf());
        
        if !state_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&state_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save_monitor_state(monitors: &[Monitor]) -> std::io::Result<()> {
        let state_path = dirs::home_dir()
            .map(|p| p.join(".config/display-tui/monitor_state.json"))
            .unwrap_or_else(|| Path::new("~/.config/display-tui/monitor_state.json").to_path_buf());
        
        fs::create_dir_all(state_path.parent().unwrap())?;
        
        let state: Vec<MonitorState> = monitors
            .iter()
            .map(|m| MonitorState {
                name: m.name.clone(),
                position: m.position.clone(),
                scale: m.scale,
                workspace: m.workspace,
            })
            .collect();
        
        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(state_path, json)?;
        
        Ok(())
    }

    pub fn create_default_config(config_json_path: &PathBuf) -> Self {
        fs::create_dir_all(config_json_path.parent().unwrap()).expect("Failed to create config directory");
        fs::write(config_json_path, "").expect("Failed to write default config file");
        Configuration {
            monitors_config_path: None,
            lua_monitor_config: None,
        } 
    }

    pub fn load_config() -> Self {
        let config_json_path = dirs::home_dir()
            .map(|p| p.join(".config/display-tui/config.json"))
            .unwrap_or_else(|| Path::new("~/.config/display-tui/config.json").to_path_buf());
        
        let config_content = fs::read_to_string(config_json_path)
            .expect("Failed to read config file");
        
        let config: Configuration = serde_json::from_str(&config_content).unwrap_or(Configuration {
            monitors_config_path: None,
            lua_monitor_config: None,
        });
        
        if config.monitors_config_path.is_none() && config.lua_monitor_config.is_none() {
             return Configuration {
                monitors_config_path: Some("~/.config/hypr/monitors.conf".to_string()),
                lua_monitor_config: None,
            };
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{Monitor, Position};

    #[test]
    fn test_save_and_load_monitor_state() {
        // Create mock monitors
        let monitors = vec![
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

        // Save
        Configuration::save_monitor_state(&monitors).expect("Failed to save");

        // Load
        let loaded = Configuration::load_monitor_state().expect("Failed to load");

        // Verify
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "HDMI-A-1");
        assert_eq!(loaded[0].position, Some(Position { x: 100, y: 200 }));
        assert_eq!(loaded[0].scale, Some(1.5));
        assert_eq!(loaded[0].workspace, Some(1));

        assert_eq!(loaded[1].name, "DP-1");
        assert_eq!(loaded[1].position, Some(Position { x: 300, y: 400 }));
        assert_eq!(loaded[1].scale, Some(1.0));
        assert_eq!(loaded[1].workspace, None);
    }
}
