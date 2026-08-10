//! Display TUI - Monitor configuration for Hyprland.
//!
//! A terminal user interface for managing monitor configurations
//! in the Hyprland compositor.

use display_tui::app::App;
use display_tui::CliAction;
use std::io;

fn main() -> io::Result<()> {
    let action = display_tui::cli::parse_args();

    match action {
        CliAction::Help => {
            display_tui::cli::print_help();
            Ok(())
        }
        CliAction::ListPresets => {
            list_presets_cli();
            Ok(())
        }
        CliAction::LoadPreset(ref name) => {
            run_with_action(Some(CliAction::LoadPreset(name.clone())))
        }
        CliAction::OpenPresetMenu => {
            run_with_action(Some(CliAction::OpenPresetMenu))
        }
        CliAction::Normal => {
            run_with_action(None)
        }
    }
}

fn run_with_action(action: Option<CliAction>) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::new(action).run(&mut terminal);
    ratatui::restore();
    app_result
}

// List presets with coloured terminal output (no TUI).
fn list_presets_cli() {
    let presets = display_tui::config::list_presets();
    if presets.is_empty() {
        println!("No presets found.");
        println!("Presets are stored in ~/.config/display-tui/presets/");
        return;
    }

    // Get connected monitor names for mismatch detection
    let monitors = display_tui::Monitor::get_monitors();
    let connected_names: Vec<String> = monitors.iter().map(|m| m.name.clone()).collect();

    println!("Presets ({})", presets.len());
    println!("{:-<50}", "");

    for name in &presets {
        let enabled_count = display_tui::config::count_enabled_monitors_in_preset(name).unwrap_or(0);
        let is_last = display_tui::config::is_last_preset(name);

        // Check for hardware mismatch
        let has_mismatch = match display_tui::config::load_preset(name) {
            Some(state) => {
                let enabled_in_preset: Vec<&str> = state
                    .iter()
                    .filter(|m| m.enabled)
                    .map(|m| m.name.as_str())
                    .collect();
                !enabled_in_preset.is_empty()
                    && !enabled_in_preset
                        .iter()
                        .all(|n| connected_names.iter().any(|c| c == *n))
            }
            None => false,
        };

        let display = if is_last {
            format!("{} [read-only]", name)
        } else {
            format!("{} ({} monitor{})",
                name,
                enabled_count,
                if enabled_count == 1 { "" } else { "s" }
            )
        };

        if enabled_count == 0 {
            // Dimmed for 0-monitor presets
            println!("[2m  {}[0m", display);
        } else if has_mismatch {
            // Red for hardware mismatch
            println!("[31m  {}[0m", display);
        } else {
            // Normal
            println!("  {}", display);
        }
    }
    println!("{:-<50}", "");
}
