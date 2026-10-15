//! Configuration file paths for the display-tui application.

use std::path::PathBuf;

// Returns the path to the monitor state JSON file.
pub fn get_state_path() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".config/display-tui/monitor_state.json"))
        .unwrap_or_else(|| PathBuf::from("monitor_state.json"))
}

// Returns the path to the display-tui configuration JSON file.
pub fn get_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".config/display-tui/config.json"))
        .unwrap_or_else(|| PathBuf::from("config.json"))
}

// Returns the path to the presets directory.
pub fn get_presets_dir() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".config/display-tui/presets"))
        .unwrap_or_else(|| PathBuf::from("presets"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_share_the_display_tui_config_directory() {
        let state = get_state_path();
        let config = get_config_path();
        let presets = get_presets_dir();

        assert_eq!(state.parent(), config.parent());
        assert_eq!(state.parent(), presets.parent());
        assert_eq!(
            state.parent().and_then(|p| p.file_name()),
            Some(std::ffi::OsStr::new("display-tui"))
        );
    }

    #[test]
    fn paths_use_the_expected_file_names() {
        assert_eq!(
            get_state_path().file_name(),
            Some(std::ffi::OsStr::new("monitor_state.json"))
        );
        assert_eq!(
            get_config_path().file_name(),
            Some(std::ffi::OsStr::new("config.json"))
        );
        assert_eq!(
            get_presets_dir().file_name(),
            Some(std::ffi::OsStr::new("presets"))
        );
    }

    #[test]
    fn paths_are_nested_under_the_home_config_directory() {
        for path in [get_state_path(), get_config_path(), get_presets_dir()] {
            let as_string = path.to_string_lossy();
            assert!(
                as_string.contains(".config/display-tui"),
                "unexpected path: {}",
                as_string
            );
        }
    }

    #[test]
    fn the_three_paths_are_distinct() {
        let state = get_state_path();
        let config = get_config_path();
        let presets = get_presets_dir();

        assert_ne!(state, config);
        assert_ne!(state, presets);
        assert_ne!(config, presets);
    }
}
