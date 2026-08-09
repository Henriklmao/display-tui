//! Monitor state persistence.
//!
//! Handles saving and loading saved monitor positions, scales, and
//! workspace assignments across sessions.

use serde::{Deserialize, Serialize};
use std::fs;
use crate::monitor::{Monitor, Position};
use super::paths::{get_state_path, get_presets_dir};
use crate::validation::{validate, validate_preset_name};

// Persistent monitor state for saving/restoring configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorState {
    pub name: String,
    pub position: Option<Position>,
    pub scale: Option<f32>,
    pub workspace: Option<u8>,
}

// Save monitor state to file.
pub fn save_monitor_state(monitors: &[Monitor]) -> std::io::Result<()> {
    let state_path = get_state_path();

    if let Some(parent) = state_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

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

// Load saved monitor state from file.
pub fn load_monitor_state() -> Option<Vec<MonitorState>> {
    let state_path = get_state_path();

    if !state_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&state_path).ok()?;
    serde_json::from_str(&content).ok()
}

// Save a preset to the presets directory.
pub fn save_preset(name: &str, monitors: &[Monitor]) -> std::io::Result<()> {
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

// Apply a preset to the current monitors configuration.
pub fn apply_preset(name: &str, monitors: &mut [Monitor]) -> Result<(), String> {
    if let Some(state) = load_preset(name) {
        for monitor_state in state {
            if let Some(monitor) = monitors.iter_mut().find(|m| m.name == monitor_state.name) {
                monitor.position = monitor_state.position;
                monitor.scale = monitor_state.scale;
                monitor.workspace = monitor_state.workspace;
            }
        }
        Ok(())
    } else {
        Err(format!("Preset '{}' not found", name))
    }
}

// Override current configuration with a preset (merge preset settings on top of current config).
// Currently behaves the same as apply_preset, but reserved for future merge behavior.
pub fn override_preset(name: &str, monitors: &mut [Monitor]) -> Result<(), String> {
    apply_preset(name, monitors)
}

// Count the number of enabled monitors in a preset.
pub fn count_enabled_monitors_in_preset(name: &str) -> Option<usize> {
    let state = load_preset(name)?;
    // Count monitors that have a position set (indicating they're enabled in the preset)
    Some(state.iter().filter(|m| m.position.is_some()).count())
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
