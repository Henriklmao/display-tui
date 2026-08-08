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
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::DuplicateWorkspace { workspace, monitors } => {
                write!(f, "Workspace {} is assigned to multiple monitors: {}",
                    workspace, monitors.join(", "))
            }
            ValidationError::OverlappingMonitors { monitor1, monitor2 } => {
                write!(f, "Monitors overlap: {} and {}", monitor1, monitor2)
            }
            ValidationError::NonContiguousMonitors { disconnected } => {
                write!(f, "Monitors not contiguous. Disconnected: {}",
                    disconnected.join(", "))
            }
        }
    }
}
