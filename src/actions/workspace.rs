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

#[cfg(test)]
mod tests {
    use super::{clear_workspace, set_workspace};
    use crate::monitor::Monitor;

    fn monitor() -> Monitor {
        Monitor {
            name: "DP-1".to_string(),
            enabled: true,
            workspace: Some(1),
            ..Default::default()
        }
    }

    #[test]
    fn set_workspace_assigns_a_number() {
        let mut m = monitor();

        set_workspace(&mut m, 9);

        assert_eq!(m.workspace, Some(9));
    }

    #[test]
    fn set_workspace_replaces_an_existing_assignment() {
        let mut m = monitor();

        set_workspace(&mut m, 42);

        assert_eq!(m.workspace, Some(42));
    }

    #[test]
    fn set_workspace_with_zero_clears_the_assignment() {
        // Hyprland has no workspace 0, so it is the "unset" sentinel.
        let mut m = monitor();

        set_workspace(&mut m, 0);

        assert_eq!(m.workspace, None);
    }

    #[test]
    fn set_workspace_accepts_the_maximum_workspace_number() {
        let mut m = monitor();

        set_workspace(&mut m, u8::MAX);

        assert_eq!(m.workspace, Some(u8::MAX));
    }

    #[test]
    fn clear_workspace_removes_the_assignment() {
        let mut m = monitor();

        clear_workspace(&mut m);

        assert_eq!(m.workspace, None);
    }

    #[test]
    fn clear_workspace_on_an_unassigned_monitor_is_a_no_op() {
        let mut m = monitor();
        m.workspace = None;

        clear_workspace(&mut m);

        assert_eq!(m.workspace, None);
    }

    #[test]
    fn assigning_after_clearing_starts_fresh() {
        let mut m = monitor();
        clear_workspace(&mut m);

        set_workspace(&mut m, 5);

        assert_eq!(m.workspace, Some(5));
    }
}
