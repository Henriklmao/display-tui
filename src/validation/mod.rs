//! Monitor configuration validation module.

pub mod errors;
pub mod rules;

pub use errors::ValidationError;

use crate::monitor::Monitor;
use self::rules::{
    validate_workspaces_unique,
    validate_no_overlap,
    validate_contiguous,
    validate_preset_name as validate_preset_name_rule,
};

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
        Err(validation_errors) => {
            Err(validation_errors.into_iter().map(|e| e.to_string()).collect())
        }
    }
}
