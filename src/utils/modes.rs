//! TUI mode enum.

// Current interaction mode of the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TUIMode {
    // View/browse mode (default).
    #[default]
    View,
    // Move mode for repositioning monitors.
    Move,
    // Resolution selection mode.
    Resolution,
    // Scale selection mode.
    Scale,
}
