//! Monitor state persistence.
//!
//! Handles saving and loading saved monitor positions, scales, and
//! workspace assignments across sessions.

use serde::{Deserialize, Serialize};
use std::fs;
use crate::monitor::{Monitor, Position};
use super::paths::get_state_path;

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
