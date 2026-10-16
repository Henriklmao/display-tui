//! Utility modules for the display-tui application.

pub mod math;
pub mod modes;
pub mod scale;

pub use math::find_best_delta;
pub use modes::TUIMode;
pub use scale::ScaleValue;

// Changes the application mode, saving monitor state if transitioning from Move/Scale.
pub fn change_mode(app: &mut crate::app::App, mode: TUIMode) {
    if app.mode == TUIMode::Move || app.mode == TUIMode::Scale {
        let _ = crate::config::save_monitor_state_in(&app.state_path(), &app.monitors);
    }
    app.mode = mode;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::monitor::{Monitor, Position, Resolution};
    use crate::test_utils::tests::TempDir;

    fn monitor() -> Monitor {
        Monitor {
            name: "DP-1".to_string(),
            enabled: true,
            position: Some(Position { x: 0, y: 0 }),
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

    // An app in the given mode that persists into a temp dir on exit.
    fn app_in(mode: TUIMode) -> (App, TempDir) {
        let dir = TempDir::new("utils-state");
        let mut app = App::new(None);
        app.monitors = vec![monitor()];
        app.mode = mode;
        app.state_path = Some(dir.path().join("monitor_state.json"));
        (app, dir)
    }

    #[test]
    fn change_mode_persists_state_when_leaving_move_or_scale() {
        for mode in [TUIMode::Move, TUIMode::Scale] {
            let (mut app, dir) = app_in(mode);

            change_mode(&mut app, TUIMode::View);

            assert_eq!(app.mode, TUIMode::View, "left {mode:?}");
            assert!(
                dir.path().join("monitor_state.json").exists(),
                "leaving {mode:?} must persist the monitor state"
            );
        }
    }

    #[test]
    fn change_mode_persists_nothing_from_other_modes() {
        for mode in [TUIMode::View, TUIMode::Resolution] {
            let (mut app, dir) = app_in(mode);

            change_mode(&mut app, TUIMode::Move);

            assert_eq!(app.mode, TUIMode::Move, "left {mode:?}");
            assert!(
                !dir.path().join("monitor_state.json").exists(),
                "entering {mode:?} must not persist"
            );
        }
    }

    #[test]
    fn state_path_defaults_to_the_real_config_path() {
        let app = App::new(None);

        assert_eq!(app.state_path(), crate::config::get_state_path());
    }
}
