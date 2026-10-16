//! Monitor state persistence.
//!
//! Handles saving and loading saved monitor positions, scales, and
//! workspace assignments across sessions.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
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
    save_monitor_state_in(&get_state_path(), monitors)
}

pub(crate) fn save_monitor_state_in(
    path: &Path,
    monitors: &[Monitor],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let state = monitors_to_state(monitors);

    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)?;

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
    save_state_as_last_in(&get_presets_dir(), monitors)
}

fn save_state_as_last_in(
    dir: &Path,
    monitors: &[Monitor],
) -> std::io::Result<()> {
    let preset_path = dir.join("last.json");
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
    save_preset_in(&get_presets_dir(), name, monitors)
}

fn save_preset_in(
    dir: &Path,
    name: &str,
    monitors: &[Monitor],
) -> std::io::Result<()> {
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

    let preset_path = dir.join(format!("{}.json", name));

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
    load_preset_in(&get_presets_dir(), name)
}

fn load_preset_in(dir: &Path, name: &str) -> Option<Vec<MonitorState>> {
    if validate_preset_name(name).is_err() {
        return None;
    }
    let preset_path = dir.join(format!("{}.json", name));

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
    validate_preset_monitors_match_in(&get_presets_dir(), preset_name, connected_monitors)
}

fn validate_preset_monitors_match_in(
    dir: &Path,
    preset_name: &str,
    connected_monitors: &[Monitor],
) -> Result<(), Vec<String>> {
    let state = match load_preset_in(dir, preset_name) {
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
    apply_preset_in(&get_presets_dir(), name, monitors)
}

fn apply_preset_in(
    dir: &Path,
    name: &str,
    monitors: &mut [Monitor],
) -> Result<(), String> {
    if let Some(state) = load_preset_in(dir, name) {
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
    override_preset_in(&get_presets_dir(), name, monitors)
}

fn override_preset_in(
    dir: &Path,
    name: &str,
    monitors: &[Monitor],
) -> Result<(), String> {
    save_preset_in(dir, name, monitors)
        .map_err(|e| format!("Failed to save preset '{}': {}", name, e))
}

// Count the number of enabled monitors in a preset.
pub fn count_enabled_monitors_in_preset(name: &str) -> Option<usize> {
    count_enabled_monitors_in_preset_in(&get_presets_dir(), name)
}

fn count_enabled_monitors_in_preset_in(dir: &Path, name: &str) -> Option<usize> {
    let state = load_preset_in(dir, name)?;
    Some(state.iter().filter(|m| m.enabled).count())
}

// List all preset names in the presets directory.
pub fn list_presets() -> Vec<String> {
    list_presets_in(&get_presets_dir())
}

fn list_presets_in(dir: &Path) -> Vec<String> {
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
                        let stem = name[..name.len() - 5].to_string();
                        // Only include names that pass preset name validation
                        if validate_preset_name(&stem).is_ok() {
                            Some(stem)
                        } else {
                            None
                        }
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
    delete_preset_in(&get_presets_dir(), name)
}

fn delete_preset_in(dir: &Path, name: &str) -> std::io::Result<()> {
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
    let preset_path = dir.join(format!("{}.json", name));

    if preset_path.exists() {
        fs::remove_file(preset_path)?;
    }

    Ok(())
}

// Rename a preset: copy the old file to the new name, then remove the old file.
// Non-destructive: refuses to overwrite an existing preset or rename to the same name.
pub fn rename_preset(old_name: &str, new_name: &str) -> std::io::Result<()> {
    rename_preset_in(&get_presets_dir(), old_name, new_name)
}

fn rename_preset_in(
    dir: &Path,
    old_name: &str,
    new_name: &str,
) -> std::io::Result<()> {
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

    let old_path = dir.join(format!("{}.json", old_name));
    let new_path = dir.join(format!("{}.json", new_name));

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::Resolution;
    use crate::test_utils::tests::TempDir;

    // A fresh, isolated presets directory for one test; it removes itself
    // when the test ends, even on a panic.
    fn temp_dir(tag: &str) -> TempDir {
        TempDir::new(&format!("state-{}", tag))
    }

    fn monitor(name: &str, x: i32, workspace: Option<u8>) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
            position: Some(Position { x, y: 0 }),
            scale: Some(1.25),
            workspace,
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

    // Two monitors side by side: passes every validation rule.
    // At 125% scale each monitor is 1536 logical pixels wide, so the second
    // one has to start at 1536 to stay contiguous.
    fn valid_layout() -> Vec<Monitor> {
        vec![
            monitor("DP-1", 0, Some(1)),
            monitor("HDMI-A-1", 1536, Some(2)),
        ]
    }

    // Two monitors stacked on top of each other: fails validate().
    fn overlapping_layout() -> Vec<Monitor> {
        vec![monitor("DP-1", 0, Some(1)), monitor("HDMI-A-1", 0, Some(2))]
    }

    #[test]
    fn only_the_exact_last_name_is_reserved() {
        assert!(is_last_preset(LAST_PRESET_NAME));
        assert!(is_last_preset("last"));
        assert!(!is_last_preset("Last"));
        assert!(!is_last_preset("last-2"));
        assert!(!is_last_preset("latest"));
        assert!(!is_last_preset(""));
    }

    #[test]
    fn save_then_load_round_trips_every_field() {
        let dir = temp_dir("roundtrip");

        save_preset_in(&dir, "desk", &valid_layout()).expect("save");

        let loaded = load_preset_in(&dir, "desk").expect("preset must load");
        assert_eq!(loaded.len(), 2);

        let first = &loaded[0];
        assert_eq!(first.name, "DP-1");
        assert!(first.enabled);
        assert_eq!(first.position, Some(Position { x: 0, y: 0 }));
        assert_eq!(first.scale, Some(1.25));
        assert_eq!(first.workspace, Some(1));
        assert_eq!(first.rotation.as_deref(), None);
        let resolution = first.resolution.as_ref().expect("resolution saved");
        assert_eq!((resolution.width, resolution.height), (1920, 1080));
        assert_eq!(resolution.refresh_rate, 60.0);

        assert_eq!(loaded[1].name, "HDMI-A-1");
        assert_eq!(loaded[1].position, Some(Position { x: 1536, y: 0 }));

    }

    #[test]
    fn save_writes_a_json_file_named_after_the_preset() {
        let dir = temp_dir("file");

        save_preset_in(&dir, "gaming", &valid_layout()).expect("save");

        let path = dir.join("gaming.json");
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        let parsed: Vec<MonitorState> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 2);

    }

    #[test]
    fn save_rejects_the_reserved_last_preset() {
        let dir = temp_dir("lastsave");

        let err = save_preset_in(&dir, "last", &valid_layout()).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("read-only preset 'last'"));
        assert!(!dir.join("last.json").exists(), "nothing may be written");

    }

    #[test]
    fn save_rejects_invalid_names() {
        let dir = temp_dir("badname");

        for name in ["", "with space", "../escape", "under_score"] {
            let err = save_preset_in(&dir, name, &valid_layout()).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "name: {:?}", name);
            assert!(
                err.to_string().contains("Preset name validation failed"),
                "unexpected message: {}",
                err
            );
        }
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);

    }

    #[test]
    fn save_rejects_an_invalid_layout() {
        let dir = temp_dir("badlayout");

        let err = save_preset_in(&dir, "overlapping", &overlapping_layout()).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string()
                .contains("Monitor configuration validation failed"),
            "unexpected message: {}",
            err
        );
        assert!(!dir.join("overlapping.json").exists());

    }

    #[test]
    fn save_creates_the_directory_when_missing() {
        let dir = temp_dir("state-nested");
        let nested = dir.path().join("deep");

        save_preset_in(&nested, "desk", &valid_layout()).expect("parent dirs are created");

        assert!(nested.join("desk.json").exists());
    }

    #[test]
    fn save_overwrites_an_existing_preset() {
        let dir = temp_dir("overwrite");

        save_preset_in(&dir, "desk", &valid_layout()).unwrap();
        let mut replacement = valid_layout();
        replacement[0].workspace = Some(9);

        save_preset_in(&dir, "desk", &replacement).unwrap();

        let loaded = load_preset_in(&dir, "desk").unwrap();
        assert_eq!(loaded[0].workspace, Some(9), "the second save must win");

    }

    #[test]
    fn load_returns_none_for_unknown_or_invalid_names() {
        let dir = temp_dir("loadmiss");

        assert!(load_preset_in(&dir, "does-not-exist").is_none());
        assert!(load_preset_in(&dir, "bad name").is_none());
        assert!(load_preset_in(&dir, "../escape").is_none());

    }

    #[test]
    fn load_returns_none_for_corrupt_json() {
        let dir = temp_dir("corrupt");
        fs::write(dir.join("broken.json"), "{ not json").unwrap();

        assert!(load_preset_in(&dir, "broken").is_none());

    }

    #[test]
    fn load_returns_none_for_a_mismatched_json_shape() {
        let dir = temp_dir("shape");
        fs::write(dir.join("wrong.json"), r#"{"name": "DP-1"}"#).unwrap();

        assert!(load_preset_in(&dir, "wrong").is_none());

    }

    #[test]
    fn list_presets_is_empty_for_missing_or_unused_directories() {
        let missing = std::env::temp_dir().join(format!(
            "display-tui-test-{}-state-missing",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&missing);
        assert_eq!(list_presets_in(&missing), Vec::<String>::new());

        let empty = temp_dir("empty");
        assert_eq!(list_presets_in(&empty), Vec::<String>::new());
    }

    #[test]
    fn list_presets_returns_sorted_valid_names_only() {
        let dir = temp_dir("listing");
        for name in ["b", "a", "M-1"] {
            save_preset_in(&dir, name, &valid_layout()).unwrap();
        }
        // "last" is written through the dedicated entry point.
        save_state_as_last_in(&dir, &valid_layout()).unwrap();
        fs::write(dir.join("notes.txt"), "").unwrap();
        fs::write(dir.join("bad name.json"), "[]").unwrap();
        fs::write(dir.join(".hidden.json"), "[]").unwrap();

        assert_eq!(
            list_presets_in(&dir),
            vec![
                "M-1".to_string(),
                "a".to_string(),
                "b".to_string(),
                "last".to_string()
            ]
        );

    }

    #[test]
    fn delete_removes_only_the_named_preset() {
        let dir = temp_dir("delete");
        save_preset_in(&dir, "keep", &valid_layout()).unwrap();
        save_preset_in(&dir, "drop", &valid_layout()).unwrap();

        delete_preset_in(&dir, "drop").expect("delete");

        assert!(!dir.join("drop.json").exists());
        assert!(dir.join("keep.json").exists());
        assert_eq!(list_presets_in(&dir), vec!["keep".to_string()]);

    }

    #[test]
    fn delete_is_idempotent_for_missing_presets() {
        let dir = temp_dir("delete-missing");

        assert!(delete_preset_in(&dir, "never-existed").is_ok());

    }

    #[test]
    fn delete_refuses_the_reserved_preset_and_invalid_names() {
        let dir = temp_dir("delete-guard");

        let err = delete_preset_in(&dir, "last").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

        let err = delete_preset_in(&dir, "bad name").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

    }

    #[test]
    fn rename_moves_the_file_and_keeps_its_content() {
        let dir = temp_dir("rename");
        save_preset_in(&dir, "old", &valid_layout()).unwrap();

        rename_preset_in(&dir, "old", "new").expect("rename");

        assert!(!dir.join("old.json").exists());
        assert!(dir.join("new.json").exists());
        let loaded = load_preset_in(&dir, "new").expect("renamed preset loads");
        assert_eq!(loaded.len(), 2);
        assert_eq!(list_presets_in(&dir), vec!["new".to_string()]);

    }

    #[test]
    fn rename_rejects_a_collision_without_touching_either_file() {
        let dir = temp_dir("rename-collision");
        save_preset_in(&dir, "a", &valid_layout()).unwrap();
        save_preset_in(&dir, "b", &valid_layout()).unwrap();

        let err = rename_preset_in(&dir, "a", "b").unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(dir.join("a.json").exists(), "source must survive");
        assert!(dir.join("b.json").exists(), "target must survive");

    }

    #[test]
    fn rename_rejects_degenerate_targets() {
        let dir = temp_dir("rename-guards");
        save_preset_in(&dir, "a", &valid_layout()).unwrap();

        let same = rename_preset_in(&dir, "a", "a").unwrap_err();
        assert_eq!(same.kind(), std::io::ErrorKind::InvalidInput);

        let bad_name = rename_preset_in(&dir, "a", "bad name").unwrap_err();
        assert_eq!(bad_name.kind(), std::io::ErrorKind::InvalidData);

        let reserved_old = rename_preset_in(&dir, "last", "b").unwrap_err();
        assert_eq!(reserved_old.kind(), std::io::ErrorKind::InvalidInput);

        let reserved_new = rename_preset_in(&dir, "a", "last").unwrap_err();
        assert_eq!(reserved_new.kind(), std::io::ErrorKind::InvalidInput);

    }

    #[test]
    fn rename_reports_a_missing_source() {
        let dir = temp_dir("rename-missing");

        let err = rename_preset_in(&dir, "ghost", "target").unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(!dir.join("target.json").exists());

    }

    #[test]
    fn apply_preset_overwrites_every_tracked_field() {
        let dir = temp_dir("apply");

        let mut source = valid_layout();
        source[0].position = Some(Position { x: 5, y: 6 });
        source[0].scale = Some(0.5);
        source[0].workspace = Some(4);
        source[0].transform = Some("90".to_string());
        source[1].enabled = false;
        save_preset_in(&dir, "p", &source).unwrap();

        let mut live = valid_layout();
        apply_preset_in(&dir, "p", &mut live).unwrap();

        assert_eq!(live[0].position, Some(Position { x: 5, y: 6 }));
        assert_eq!(live[0].scale, Some(0.5));
        assert_eq!(live[0].workspace, Some(4));
        assert_eq!(live[0].transform.as_deref(), Some("90"));
        assert!(!live[1].enabled, "preset may disable a monitor");

    }

    #[test]
    fn apply_preset_switches_to_the_saved_resolution() {
        let dir = temp_dir("apply-res");

        let mut two_modes = valid_layout();
        two_modes[0].modes.push(Resolution {
            width: 1280,
            height: 720,
            refresh: 75.0,
            preferred: false,
            current: true,
        });
        two_modes[0].modes[0].current = false;
        // With the second mode current, DP-1 is only 1024 logical pixels
        // wide, so HDMI has to sit at 1024 for the layout to stay contiguous.
        two_modes[1].position = Some(Position { x: 1024, y: 0 });
        save_preset_in(&dir, "p", &two_modes).unwrap();

        // Live monitor still runs on its first mode.
        let mut live = valid_layout();
        live[0].modes.push(Resolution {
            width: 1280,
            height: 720,
            refresh: 75.0,
            preferred: false,
            current: false,
        });
        assert!(live[0].modes[0].current);

        apply_preset_in(&dir, "p", &mut live).unwrap();

        assert!(!live[0].modes[0].current);
        assert!(live[0].modes[1].current, "1280x720@75 must become current");

    }

    #[test]
    fn apply_preset_keeps_the_current_mode_when_no_mode_matches() {
        let dir = temp_dir("apply-nomatch");

        let mut source = valid_layout();
        source[0].modes.push(Resolution {
            width: 800,
            height: 600,
            refresh: 60.0,
            preferred: false,
            current: true,
        });
        source[0].modes[0].current = false;
        // 800x600 at 125% scale is 640 logical pixels wide.
        source[1].position = Some(Position { x: 640, y: 0 });
        save_preset_in(&dir, "p", &source).unwrap();

        // Live monitor has no 800x600 mode at all.
        let mut live = valid_layout();
        apply_preset_in(&dir, "p", &mut live).unwrap();

        assert!(live[0].modes[0].current, "unmatched mode is ignored");
        assert_eq!(live[0].modes.len(), 1);

    }

    #[test]
    fn apply_preset_leaves_unknown_monitors_alone() {
        let dir = temp_dir("apply-unknown");

        save_preset_in(&dir, "p", &valid_layout()).unwrap();

        let mut live = vec![
            monitor("DP-1", 999, None),
            monitor("NOT-IN-PRESET", 0, Some(7)),
        ];
        apply_preset_in(&dir, "p", &mut live).unwrap();

        assert_eq!(
            live[0].position,
            Some(Position { x: 0, y: 0 }),
            "DP-1 was in the preset, so it was reset"
        );
        assert_eq!(
            live[1].position,
            Some(Position { x: 0, y: 0 }),
            "monitors absent from the preset keep their values"
        );
        assert_eq!(live[1].workspace, Some(7));

    }

    #[test]
    fn apply_preset_reports_a_missing_preset() {
        let dir = temp_dir("apply-missing");

        let mut live = valid_layout();
        let err = apply_preset_in(&dir, "ghost", &mut live).unwrap_err();

        assert_eq!(err, "Preset 'ghost' not found");
        assert_eq!(
            live[0].position,
            Some(Position { x: 0, y: 0 }),
            "nothing may change on failure"
        );

    }

    #[test]
    fn count_only_includes_enabled_monitors() {
        let dir = temp_dir("count");

        let mut layout = valid_layout();
        layout[1].enabled = false;
        save_preset_in(&dir, "p", &layout).unwrap();

        assert_eq!(count_enabled_monitors_in_preset_in(&dir, "p"), Some(1));

        save_preset_in(&dir, "all", &valid_layout()).unwrap();
        assert_eq!(count_enabled_monitors_in_preset_in(&dir, "all"), Some(2));

        assert_eq!(count_enabled_monitors_in_preset_in(&dir, "ghost"), None);
        assert_eq!(count_enabled_monitors_in_preset_in(&dir, "bad name"), None);

    }

    #[test]
    fn preset_match_is_ok_when_every_enabled_monitor_is_connected() {
        let dir = temp_dir("match-ok");

        let mut layout = valid_layout();
        layout[1].enabled = false;
        save_preset_in(&dir, "p", &layout).unwrap();

        // Only DP-1 is enabled in the preset, so it is the only requirement.
        let connected = vec![monitor("DP-1", 0, None)];
        assert_eq!(
            validate_preset_monitors_match_in(&dir, "p", &connected),
            Ok(())
        );

    }

    #[test]
    fn preset_match_lists_the_missing_monitors() {
        let dir = temp_dir("match-missing");
        save_preset_in(&dir, "p", &valid_layout()).unwrap();

        let connected = vec![monitor("DP-1", 0, None)];
        let missing = validate_preset_monitors_match_in(&dir, "p", &connected).unwrap_err();

        assert_eq!(missing, vec!["HDMI-A-1".to_string()]);

    }

    #[test]
    fn preset_match_reports_an_unknown_preset() {
        let dir = temp_dir("match-unknown");

        let err = validate_preset_monitors_match_in(&dir, "ghost", &[]).unwrap_err();

        assert_eq!(err, vec!["Preset 'ghost' not found".to_string()]);

    }

    #[test]
    fn override_wraps_save_errors_with_context() {
        let dir = temp_dir("override-err");

        let err = override_preset_in(&dir, "broken", &overlapping_layout()).unwrap_err();

        assert!(
            err.starts_with("Failed to save preset 'broken': "),
            "unexpected message: {}",
            err
        );
        assert!(!dir.join("broken.json").exists());

    }

    #[test]
    fn override_saves_a_valid_layout() {
        let dir = temp_dir("override-ok");

        override_preset_in(&dir, "desk", &valid_layout()).expect("save");

        assert_eq!(list_presets_in(&dir), vec!["desk".to_string()]);

    }

    #[test]
    fn save_state_as_last_skips_layout_validation() {
        let dir = temp_dir("last-invalid");

        // An overlapping layout may never be rejected: it is the live state.
        save_state_as_last_in(&dir, &overlapping_layout()).expect("must not validate");

        let loaded = load_preset_in(&dir, "last").expect("last preset loads");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].position, loaded[1].position);

    }

    #[test]
    fn save_state_as_last_replaces_a_previous_snapshot() {
        let dir = temp_dir("last-replace");

        save_state_as_last_in(&dir, &valid_layout()).unwrap();
        let mut changed = valid_layout();
        changed[0].scale = Some(2.0);

        save_state_as_last_in(&dir, &changed).unwrap();

        let loaded = load_preset_in(&dir, "last").unwrap();
        assert_eq!(loaded[0].scale, Some(2.0));

    }
}
