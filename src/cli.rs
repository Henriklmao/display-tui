//! Command-line argument parsing for display-tui.
//!
//! Parses CLI flags and returns a [`CliAction`] directing the
//! application to the appropriate mode.

use std::process;

// Actions the CLI can request before (or instead of) the TUI.
pub enum CliAction {
    // Print help text and exit.
    Help,
    // List presets with coloured output and exit.
    ListPresets,
    // Load a preset by name, apply it, write config, then exit.
    // On failure, open the TUI with an error modal.
    LoadPreset(String),
    // Open the TUI directly with the preset menu visible.
    OpenPresetMenu,
    // Normal TUI startup (no CLI action).
    Normal,
}

// Parse command-line arguments and return the appropriate action.
pub fn parse_args() -> CliAction {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        return CliAction::Normal;
    }

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return CliAction::Help,

            // Combined short forms: -pl, -pm
            s if s.starts_with("-p") && s.len() > 2 => {
                let rest = &s[2..];
                match rest {
                    "l" => return CliAction::ListPresets,
                    "m" => return CliAction::OpenPresetMenu,
                    _ => return CliAction::LoadPreset(rest.to_string()),
                }
            }

            "-p" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("display-tui: -p requires an argument (preset name, -l, or -m)");
                    return CliAction::Help;
                }
                match args[i].as_str() {
                    "-l" | "l" => return CliAction::ListPresets,
                    "-m" | "m" => return CliAction::OpenPresetMenu,
                    name => return CliAction::LoadPreset(name.to_string()),
                }
            }

            other => {
                eprintln!("display-tui: unknown argument '{}'", other);
                return CliAction::Help;
            }
        }
    }

    CliAction::Normal
}

// Print the CLI help text and exit.
pub fn print_help() {
    println!("display-tui {}", env!("CARGO_PKG_VERSION"));
    println!("A terminal user interface for managing Hyprland monitor configurations.");
    println!();
    println!("USAGE:");
    println!("  display-tui [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("  --help, -h         Show this help message and exit");
    println!("  -p <preset-name>   Load and apply the named preset, then exit");
    println!("  -pl, -p -l         List available presets and exit");
    println!("  -pm                Open the TUI with the preset menu open");
    println!();
    println!("If no options are given, the TUI opens in normal view mode.");
    process::exit(0);
}
