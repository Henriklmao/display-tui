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
