//! Command-line argument parsing for display-tui.
//!
//! Parses CLI flags and returns a [`CliAction`] directing the
//! application to the appropriate mode.

use std::process;

// Actions the CLI can request before (or instead of) the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    parse_args_from(std::env::args().skip(1))
}

// Parse an explicit argument list and return the appropriate action.
//
// Split out of [`parse_args`] so the mapping from arguments to
// [`CliAction`] can be tested without touching the process environment.
pub fn parse_args_from<I>(args: I) -> CliAction
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();

    if args.is_empty() {
        return CliAction::Normal;
    }

    match args[0].as_str() {
        "--help" | "-h" => CliAction::Help,

        // Combined short forms: -pl, -pm, -p<name>
        s if s.starts_with("-p") && s.len() > 2 => {
            let rest = &s[2..];
            match rest {
                "l" => CliAction::ListPresets,
                "m" => CliAction::OpenPresetMenu,
                _ => CliAction::LoadPreset(rest.to_string()),
            }
        }

        "-p" => {
            if args.len() < 2 {
                eprintln!("display-tui: -p requires an argument (preset name, -l, or -m)");
                return CliAction::Help;
            }
            match args[1].as_str() {
                "-l" | "l" => CliAction::ListPresets,
                "-m" | "m" => CliAction::OpenPresetMenu,
                name => CliAction::LoadPreset(name.to_string()),
            }
        }

        other => {
            eprintln!("display-tui: unknown argument '{}'", other);
            CliAction::Help
        }
    }
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
    println!("  -pm, -p -m         Open the TUI with the preset menu open");
    println!();
    println!("If no options are given, the TUI opens in normal view mode.");
    process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::{CliAction, parse_args_from};

    fn parse(args: &[&str]) -> CliAction {
        parse_args_from(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_arguments_start_the_tui() {
        assert_eq!(parse(&[]), CliAction::Normal);
    }

    #[test]
    fn help_is_recognised_in_both_spellings() {
        assert_eq!(parse(&["--help"]), CliAction::Help);
        assert_eq!(parse(&["-h"]), CliAction::Help);
    }

    #[test]
    fn combined_flags_select_list_and_menu() {
        assert_eq!(parse(&["-pl"]), CliAction::ListPresets);
        assert_eq!(parse(&["-pm"]), CliAction::OpenPresetMenu);
    }

    #[test]
    fn combined_flag_loads_a_preset_by_name() {
        assert_eq!(
            parse(&["-pgaming-desk"]),
            CliAction::LoadPreset("gaming-desk".to_string())
        );
        assert_eq!(parse(&["-px"]), CliAction::LoadPreset("x".to_string()));
    }

    #[test]
    fn separate_preset_flag_accepts_list_and_menu() {
        assert_eq!(parse(&["-p", "-l"]), CliAction::ListPresets);
        assert_eq!(parse(&["-p", "l"]), CliAction::ListPresets);
        assert_eq!(parse(&["-p", "-m"]), CliAction::OpenPresetMenu);
        assert_eq!(parse(&["-p", "m"]), CliAction::OpenPresetMenu);
    }

    #[test]
    fn separate_preset_flag_loads_a_named_preset() {
        assert_eq!(
            parse(&["-p", "my-setup"]),
            CliAction::LoadPreset("my-setup".to_string())
        );
        // Only the second token is consumed; later ones are ignored.
        assert_eq!(
            parse(&["-p", "my-setup", "--help"]),
            CliAction::LoadPreset("my-setup".to_string())
        );
    }

    #[test]
    fn preset_flag_without_an_argument_falls_back_to_help() {
        assert_eq!(parse(&["-p"]), CliAction::Help);
    }

    #[test]
    fn unknown_arguments_fall_back_to_help() {
        assert_eq!(parse(&["--verbose"]), CliAction::Help);
        assert_eq!(parse(&["-x"]), CliAction::Help);
        assert_eq!(parse(&["preset-name"]), CliAction::Help);
    }

    #[test]
    fn only_the_first_argument_is_consulted() {
        assert_eq!(parse(&["--help", "--version"]), CliAction::Help);
        assert_eq!(parse(&["-pl", "extra"]), CliAction::ListPresets);
    }

    #[test]
    fn only_the_first_token_matters_for_combined_flags() {
        // "-pfoo" is a preset named "foo"; a bare "-plm" is not the list or
        // menu shorthand, so the whole remainder becomes the preset name.
        assert_eq!(parse(&["-pfoo"]), CliAction::LoadPreset("foo".to_string()));
        assert_eq!(parse(&["-plm"]), CliAction::LoadPreset("lm".to_string()));
    }

    #[test]
    fn cli_action_variants_are_distinct() {
        let variants = [
            CliAction::Help,
            CliAction::ListPresets,
            CliAction::LoadPreset("x".to_string()),
            CliAction::OpenPresetMenu,
            CliAction::Normal,
        ];

        for (i, left) in variants.iter().enumerate() {
            for (j, right) in variants.iter().enumerate() {
                assert_eq!(left == right, i == j, "{:?} vs {:?}", left, right);
            }
        }
    }
}
