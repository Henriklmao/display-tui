//! Workspace assignment actions.

use crate::monitor::Monitor;

// Sets a monitor's workspace assignment.
pub fn set_workspace(monitor: &mut Monitor, workspace: u8) {
    if workspace == 0 {
        monitor.workspace = None;
    } else {
        monitor.workspace = Some(workspace);
    }
}

// Clears a monitor's workspace assignment.
pub fn clear_workspace(monitor: &mut Monitor) {
    monitor.workspace = None;
}
