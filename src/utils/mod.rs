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
        let _ = crate::config::save_monitor_state(&app.monitors);
    }
    app.mode = mode;
}
