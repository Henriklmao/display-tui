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

#[cfg(test)]
mod tests {
    use super::TUIMode;

    const ALL: [TUIMode; 4] = [
        TUIMode::View,
        TUIMode::Move,
        TUIMode::Resolution,
        TUIMode::Scale,
    ];

    #[test]
    fn default_mode_is_view() {
        assert_eq!(TUIMode::default(), TUIMode::View);
    }

    #[test]
    fn modes_are_distinct() {
        for (i, a) in ALL.iter().enumerate() {
            for (j, b) in ALL.iter().enumerate() {
                assert_eq!(a == b, i == j, "{:?} vs {:?}", a, b);
            }
        }
    }

    #[test]
    fn modes_are_copyable_and_comparable() {
        // Compile-time contract relied on by the app state.
        let mode = TUIMode::View;
        let alias = mode;
        assert_eq!(mode, alias);
        assert_eq!(format!("{:?}", mode), "View");
    }
}
