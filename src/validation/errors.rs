//! Validation error types.

use std::fmt;

// Errors that can occur during monitor configuration validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    // A workspace is assigned to multiple monitors.
    DuplicateWorkspace {
        workspace: u8,
        monitors: Vec<String>,
    },
    // Two or more monitors have overlapping display areas.
    OverlappingMonitors {
        monitor1: String,
        monitor2: String,
    },
    // Enabled monitors do not form a contiguous display area.
    NonContiguousMonitors {
        disconnected: Vec<String>,
    },
    // A preset name is invalid.
    InvalidName,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::DuplicateWorkspace {
                workspace,
                monitors,
            } => {
                write!(
                    f,
                    "Workspace {} is assigned to multiple monitors: {}",
                    workspace,
                    monitors.join(", ")
                )
            }
            ValidationError::OverlappingMonitors { monitor1, monitor2 } => {
                write!(f, "Monitors overlap: {} and {}", monitor1, monitor2)
            }
            ValidationError::NonContiguousMonitors { disconnected } => {
                write!(
                    f,
                    "Monitors not contiguous. Disconnected: {}",
                    disconnected.join(", ")
                )
            }
            ValidationError::InvalidName => {
                write!(
                    f,
                    "Preset name is invalid (must contain only letters, numbers, and hyphens)"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ValidationError;

    #[test]
    fn duplicate_workspace_message_lists_all_monitors() {
        let error = ValidationError::DuplicateWorkspace {
            workspace: 3,
            monitors: vec!["DP-1".to_string(), "HDMI-A-1".to_string()],
        };

        assert_eq!(
            error.to_string(),
            "Workspace 3 is assigned to multiple monitors: DP-1, HDMI-A-1"
        );
    }

    #[test]
    fn overlap_message_names_both_monitors() {
        let error = ValidationError::OverlappingMonitors {
            monitor1: "DP-1".to_string(),
            monitor2: "eDP-1".to_string(),
        };

        assert_eq!(error.to_string(), "Monitors overlap: DP-1 and eDP-1");
    }

    #[test]
    fn non_contiguous_message_lists_disconnected_monitors() {
        let error = ValidationError::NonContiguousMonitors {
            disconnected: vec!["DP-2".to_string(), "DP-3".to_string()],
        };

        assert_eq!(
            error.to_string(),
            "Monitors not contiguous. Disconnected: DP-2, DP-3"
        );
    }

    #[test]
    fn invalid_name_message_explains_the_rule() {
        assert_eq!(
            ValidationError::InvalidName.to_string(),
            "Preset name is invalid (must contain only letters, numbers, and hyphens)"
        );
    }

    #[test]
    fn variants_are_equality_comparable() {
        let a = ValidationError::DuplicateWorkspace {
            workspace: 1,
            monitors: vec!["A".to_string()],
        };
        let b = a.clone();

        assert_eq!(a, b);
        assert_ne!(a, ValidationError::InvalidName);
        assert_ne!(
            ValidationError::InvalidName,
            ValidationError::DuplicateWorkspace {
                workspace: 2,
                monitors: vec!["A".to_string()],
            }
        );
    }

    #[test]
    fn variants_produce_distinct_debug_output() {
        let names = [
            format!("{:?}", ValidationError::InvalidName),
            format!(
                "{:?}",
                ValidationError::OverlappingMonitors {
                    monitor1: "A".to_string(),
                    monitor2: "B".to_string(),
                }
            ),
            format!(
                "{:?}",
                ValidationError::NonContiguousMonitors {
                    disconnected: vec!["C".to_string()],
                }
            ),
        ];

        for (i, left) in names.iter().enumerate() {
            for (j, right) in names.iter().enumerate() {
                assert_eq!(left == right, i == j, "{:?} vs {:?}", left, right);
            }
        }
    }
}
