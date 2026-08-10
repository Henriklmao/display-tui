//! Monitor state persistence.
//!
//! Handles saving and loading saved monitor positions, scales, and
//! workspace assignments across sessions.

use serde::{Deserialize, Serialize};
use std::fs;
use crate::monitor::{Monitor, Position};
use super::paths::{get_state_path, get_presets_dir};
use crate::validation::{validate, validate_preset_name};

pub const LAST_PRESET_NAME: &str = "last";

pub fn is_last_preset(name: &str) -> bool {
    name == LAST_PRESET_NAME
}

// Snapshot of a monitor's current resolution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionState {
    pub width: i32,
    pub height: i32,
    #[serde(rename = "refresh")]
    pub refresh_rate: f32,
}

// Persistent monitor state for saving/restoring configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorState {
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    pub position: Option<Position>,
    pub scale: Option<f32>,
    pub workspace: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionState>,
}

// Convert a slice of Monitors into a Vec of MonitorState for persistence.
fn monitors_to_state(monitors: &[Monitor]) -> Vec<MonitorState> {
    monitors
        .iter()
        .map(|m| {
            let resolution = m.get_current_resolution().map(|r| ResolutionState {
                width: r.width,
                height: r.height,
                refresh_rate: r.refresh,
            });
            MonitorState {
                name: m.name.clone(),
                enabled: m.enabled,
                position: m.position.clone(),
                scale: m.scale,
                workspace: m.workspace,
                rotation: m.transform.clone(),
                resolution,
            }
        })
        .collect()
}

// Save monitor state to file.
pub fn save_monitor_state(monitors: &[Monitor]) -> std::io::Result<()> {
    let state_path = get_state_path();

    if let Some(parent) = state_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let state = monitors_to_state(monitors);

    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(state_path, json)?;

    Ok(())
}

// Load saved monitor state from file.
pub fn load_monitor_state() -> Option<Vec<MonitorState>> {
    let state_path = get_state_path();

    if !state_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&state_path).ok()?;
    serde_json::from_str(&content).ok()
}

// Save the current monitor state as "last" preset.
// No layout validation — live state can be invalid.
// No name validation — "last" is always valid.
pub fn save_state_as_last(monitors: &[Monitor]) -> std::io::Result<()> {
    let preset_path = get_presets_dir().join("last.json");
    if let Some(parent) = preset_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let state = monitors_to_state(monitors);
    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(preset_path, json)?;
    Ok(())
}

// Save a preset to the presets directory.
pub fn save_preset(name: &str, monitors: &[Monitor]) -> std::io::Result<()> {
    if is_last_preset(name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Cannot save to read-only preset '{}'", LAST_PRESET_NAME),
        ));
    }
    // Validate name
    if let Err(errors) = validate_preset_name(name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Preset name validation failed: {}", errors.join(", ")),
        ));
    }

    // Validate layout
    if let Err(errors) = validate(monitors) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Monitor configuration validation failed: {}", errors.join(", ")),
        ));
    }

    let preset_path = get_presets_dir().join(format!("{}.json", name));

    if let Some(parent) = preset_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let state = monitors_to_state(monitors);

    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(preset_path, json)?;

    Ok(())
}

// Load a preset from the presets directory.
pub fn load_preset(name: &str) -> Option<Vec<MonitorState>> {
    if validate_preset_name(name).is_err() {
        return None;
    }
    let preset_path = get_presets_dir().join(format!("{}.json", name));

    if !preset_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&preset_path).ok()?;
    serde_json::from_str(&content).ok()
}

// Check whether all enabled monitors in a preset are currently physically connected.
// Returns Ok(()) on a match, otherwise Err with the list of missing monitor names.
pub fn validate_preset_monitors_match(
    preset_name: &str,
    connected_monitors: &[Monitor],
) -> Result<(), Vec<String>> {
    let state = match load_preset(preset_name) {
        Some(state) => state,
        None => return Err(vec![format!("Preset '{}' not found", preset_name)]),
    };

    let connected_names: Vec<&str> = connected_monitors
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    let missing: Vec<String> = state
        .iter()
        .filter(|m| m.enabled)
        .map(|m| m.name.clone())
        .filter(|name| !connected_names.contains(&name.as_str()))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

// Apply a preset to the current monitors configuration.
pub fn apply_preset(name: &str, monitors: &mut [Monitor]) -> Result<(), String> {
    if let Some(state) = load_preset(name) {
        for monitor_state in state {
            if let Some(monitor) = monitors.iter_mut().find(|m| m.name == monitor_state.name) {
                monitor.position = monitor_state.position;
                monitor.scale = monitor_state.scale;
                monitor.workspace = monitor_state.workspace;
                monitor.enabled = monitor_state.enabled;
                monitor.transform = monitor_state.rotation;
                if let Some(ref res) = monitor_state.resolution
                    && let Some(idx) = monitor.modes.iter().position(|m| {
                        m.width == res.width
                            && m.height == res.height
                            && (m.refresh - res.refresh_rate).abs() < 0.1
                    })
                {
                    monitor.set_current_resolution(idx);
                }
            }
        }
        Ok(())
    } else {
        Err(format!("Preset '{}' not found", name))
    }
}

// Override the preset with the current monitor configuration: saves the
// current monitor state under the given preset name, replacing any existing
// preset with that name.
pub fn override_preset(name: &str, monitors: &[Monitor]) -> Result<(), String> {
    save_preset(name, monitors)
        .map_err(|e| format!("Failed to save preset '{}': {}", name, e))
}

// Count the number of enabled monitors in a preset.
pub fn count_enabled_monitors_in_preset(name: &str) -> Option<usize> {
    let state = load_preset(name)?;
    Some(state.iter().filter(|m| m.enabled).count())
}

// List all preset names in the presets directory.
pub fn list_presets() -> Vec<String> {
    let dir = get_presets_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|res| res.ok())
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    if name.ends_with(".json") {
                        Some(name[..name.len() - 5].to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

// Delete a preset from the presets directory.
pub fn delete_preset(name: &str) -> std::io::Result<()> {
    if is_last_preset(name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Cannot delete read-only preset '{}'", LAST_PRESET_NAME),
        ));
    }
    if let Err(errors) = validate_preset_name(name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Preset name validation failed: {}", errors.join(", ")),
        ));
    }
    let preset_path = get_presets_dir().join(format!("{}.json", name));

    if preset_path.exists() {
        fs::remove_file(preset_path)?;
    }

    Ok(())
}

// Rename a preset: copy the old file to the new name, then remove the old file.
// Non-destructive: refuses to overwrite an existing preset or rename to the same name.
pub fn rename_preset(old_name: &str, new_name: &str) -> std::io::Result<()> {
    if is_last_preset(old_name) || is_last_preset(new_name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Cannot rename read-only preset '{}'", LAST_PRESET_NAME),
        ));
    }
    if let Err(errors) = validate_preset_name(new_name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Preset name validation failed: {}", errors.join(", ")),
        ));
    }

    if old_name == new_name {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "New name must differ from the old name",
        ));
    }

    let old_path = get_presets_dir().join(format!("{}.json", old_name));
    let new_path = get_presets_dir().join(format!("{}.json", new_name));

    if !old_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Preset '{}' not found", old_name),
        ));
    }

    if new_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("A preset named '{}' already exists", new_name),
        ));
    }

    fs::copy(&old_path, &new_path)?;
    fs::remove_file(old_path)?;
    Ok(())
}
