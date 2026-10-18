//! UI style definitions.

use ratatui::style::{Color, Style};

// Primary border color.
pub const COLOR_PRIMARY: Color = Color::Yellow;

// Secondary color for highlights.
pub const COLOR_SECONDARY: Color = Color::Blue;

// Success/accent color.
pub const COLOR_SUCCESS: Color = Color::Green;

// Error color.
pub const COLOR_ERROR: Color = Color::Red;

// Text color.
pub const COLOR_TEXT: Color = Color::White;

// Default border style.
pub fn border_style() -> Style {
    Style::default().fg(COLOR_PRIMARY)
}

// Highlight style for selected items.
pub fn highlight_style() -> Style {
    Style::new().fg(Color::Yellow)
}

// Header style.
pub fn header_style() -> Style {
    Style::new().bold().fg(COLOR_SUCCESS).reversed()
}

// The alternating accent of the currently hovered element. Two steps per
// shimmer cycle so the colour visibly runs through the text without needing a
// per-grapheme sweep.
pub fn hover_accent(tick: u32) -> Color {
    if tick.is_multiple_of(2) {
        Color::LightCyan
    } else {
        Color::LightGreen
    }
}

// Style for a hovered interactive element.
pub fn hover_style(tick: u32) -> Style {
    Style::new().fg(hover_accent(tick)).bold()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    #[test]
    fn palette_colors_are_distinct() {
        let colors = [
            COLOR_PRIMARY,
            COLOR_SECONDARY,
            COLOR_SUCCESS,
            COLOR_ERROR,
            COLOR_TEXT,
        ];

        for (i, left) in colors.iter().enumerate() {
            for (j, right) in colors.iter().enumerate() {
                assert_eq!(left == right, i == j, "{:?} vs {:?}", left, right);
            }
        }
    }

    #[test]
    fn palette_colors_are_the_expected_hues() {
        assert_eq!(COLOR_PRIMARY, Color::Yellow);
        assert_eq!(COLOR_SECONDARY, Color::Blue);
        assert_eq!(COLOR_SUCCESS, Color::Green);
        assert_eq!(COLOR_ERROR, Color::Red);
        assert_eq!(COLOR_TEXT, Color::White);
    }

    #[test]
    fn border_style_sets_only_the_foreground() {
        let style = border_style();

        assert_eq!(style.fg, Some(COLOR_PRIMARY));
        assert_eq!(style.bg, None);
        assert!(style.add_modifier.is_empty());
    }

    #[test]
    fn highlight_style_matches_the_primary_color() {
        let style = highlight_style();

        assert_eq!(style.fg, Some(COLOR_PRIMARY));
        assert_eq!(style.bg, None);
        assert!(style.add_modifier.is_empty());
    }

    #[test]
    fn header_style_is_bold_and_reversed_in_the_success_color() {
        let style = header_style();

        assert_eq!(style.fg, Some(COLOR_SUCCESS));
        assert_eq!(style.bg, None);
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert!(style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn styles_are_idempotent() {
        // Rendering the same widget twice must not accumulate attributes.
        assert_eq!(border_style(), border_style());
        assert_eq!(highlight_style(), highlight_style());
        assert_eq!(header_style(), header_style());
    }

    #[test]
    fn the_hover_accent_shimmers_between_two_steps() {
        assert_ne!(hover_accent(0), hover_accent(1));
        assert_eq!(hover_accent(0), hover_accent(2), "the cycle repeats");
        assert_ne!(hover_accent(0), Color::Reset);
    }

    #[test]
    fn hover_style_is_bold_in_the_accent_colour() {
        let style = hover_style(0);
        assert_eq!(style.fg, Some(hover_accent(0)));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
