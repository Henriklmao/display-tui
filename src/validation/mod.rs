//! Monitor configuration validation module.

pub mod diagnostics;
pub mod errors;
pub mod rules;

pub use errors::ValidationError;

use self::rules::{
    validate_contiguous, validate_no_overlap, validate_origin,
    validate_preset_name as validate_preset_name_rule, validate_workspaces_unique,
};
use crate::monitor::Monitor;

// Validates a monitor configuration.
pub fn validate(monitors: &[Monitor]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Err(validation_errors) = validate_workspaces_unique(monitors) {
        for err in validation_errors {
            errors.push(err.to_string());
        }
    }

    if let Err(validation_errors) = validate_no_overlap(monitors) {
        for err in validation_errors {
            errors.push(err.to_string());
        }
    }

    if let Err(validation_errors) = validate_contiguous(monitors) {
        for err in validation_errors {
            errors.push(err.to_string());
        }
    }

    if let Err(validation_errors) = validate_origin(monitors) {
        for err in validation_errors {
            errors.push(err.to_string());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// Validates a preset name (alphanumeric characters and hyphens only).
pub fn validate_preset_name(name: &str) -> Result<(), Vec<String>> {
    match validate_preset_name_rule(name) {
        Ok(()) => Ok(()),
        Err(validation_errors) => Err(validation_errors
            .into_iter()
            .map(|e| e.to_string())
            .collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::{validate, validate_preset_name};
    use crate::monitor::{Monitor, Position, Resolution};

    // A rectangular enabled monitor occupying (x, y) .. (x+w, y+h).
    fn box_at(name: &str, x: i32, y: i32, workspace: Option<u8>) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
            position: Some(Position { x, y }),
            scale: Some(1.0),
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

    #[test]
    fn valid_layout_is_accepted() {
        let monitors = vec![
            box_at("A", 0, 0, Some(1)),
            box_at("B", 1920, 0, Some(2)),
            box_at("C", 3840, 0, None),
        ];

        assert_eq!(validate(&monitors), Ok(()));
    }

    #[test]
    fn empty_configuration_is_accepted() {
        assert_eq!(validate(&[]), Ok(()));
    }

    #[test]
    fn every_violation_is_collected_as_a_message() {
        // A and B share a workspace and overlap; C is far away.
        let monitors = vec![
            box_at("A", 0, 0, Some(1)),
            box_at("B", 100, 0, Some(1)),
            box_at("C", 50_000, 0, None),
        ];

        let errors = validate(&monitors).unwrap_err();

        assert_eq!(errors.len(), 3, "got: {errors:?}");
        assert_eq!(
            errors[0],
            "Workspace 1 is assigned to multiple monitors: A, B"
        );
        assert_eq!(errors[1], "Monitors overlap: A and B");
        assert_eq!(errors[2], "Monitors not contiguous. Disconnected: C");
    }

    #[test]
    fn messages_are_reported_in_rule_order() {
        // Overlap and contiguity both fail; rules run in a fixed order.
        let monitors = vec![
            box_at("A", 0, 0, None),
            box_at("B", 100, 0, None),
            box_at("C", 50_000, 0, None),
        ];

        let errors = validate(&monitors).unwrap_err();

        assert_eq!(
            errors,
            vec![
                "Monitors overlap: A and B".to_string(),
                "Monitors not contiguous. Disconnected: C".to_string(),
            ]
        );
    }

    #[test]
    fn preset_name_wrapper_accepts_valid_names() {
        assert_eq!(validate_preset_name("gaming-desk"), Ok(()));
    }

    #[test]
    fn a_layout_off_the_origin_is_reported_after_the_geometry_rules() {
        // Valid geometry, but shifted away from (0,0).
        let monitors = vec![box_at("A", 1000, 0, Some(1)), box_at("B", 2920, 0, Some(2))];

        let errors = validate(&monitors).unwrap_err();

        assert_eq!(
            errors,
            vec!["No monitor starts at the origin (0,0)".to_string()]
        );
    }

    #[test]
    fn preset_name_wrapper_explains_rejections() {
        let errors = validate_preset_name("my preset").unwrap_err();
        assert_eq!(
            errors,
            vec![
                "Preset name is invalid (must contain only letters, numbers, and hyphens)"
                    .to_string()
            ]
        );
    }
}
