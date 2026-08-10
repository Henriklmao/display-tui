//! Help modal component.
//!
//! The help popup adapts to the terminal size: it grows up to a maximum
//! size (30 rows x 60 cols) but shrinks gracefully on small terminals.
//! On wide terminals the keybindings are shown in two side-by-side
//! columns; below [`TWO_COLUMN_MIN_WIDTH`] every section is stacked into
//! a single column instead.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

// Help modal display.
pub struct HelpModal;

// Terminal width at which the help switches from a single stacked column
// to the two-column layout.
const TWO_COLUMN_MIN_WIDTH: u16 = 80;

impl HelpModal {
    pub fn render(area: Rect, buf: &mut Buffer) {
        let popup_area = responsive_help_rect(area);

        if area.width >= TWO_COLUMN_MIN_WIDTH {
            render_columns(popup_area, buf);
        } else {
            render_stacked(popup_area, buf);
        }
    }
}

// Adaptive popup rect. Vertically the help uses at most 30 rows and
// horizontally at most 60 columns; on smaller terminals it shrinks to
// fit (with at least one row/column of breathing room on each side).
fn responsive_help_rect(area: Rect) -> Rect {
    // Vertical: at most 30 rows, with at least 1 row of padding above
    // and below.
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Max(30), // Help content max 30 rows
            Constraint::Min(1),
        ])
        .split(area);

    // Horizontal: at most 60 columns, with at least 1 column of padding
    // on each side.
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Max(60), // Help content max 60 cols
            Constraint::Min(1),
        ])
        .split(vertical[1])[1]
}

// Left column: global actions and view-mode navigation.
fn left_sections() -> Vec<Line<'static>> {
    vec![
        Line::from(" --- Global --- ".bold().yellow()),
        Line::from("Save <w> | Quit <q>"),
        Line::from(""),
        Line::from(" --- View Mode --- ".bold().yellow()),
        Line::from("Up <k> | Down <j>"),
        Line::from("Move <m> | Resolution <r> | Scale <s>"),
        Line::from("Rotate <o>"),
        Line::from("Set Workspace (0 to clear) <0-9>"),
        Line::from("Enable <e> | Disable <d>"),
        Line::from("Preset Menu <p>"),
        Line::from("Show Keybindings <K>"),
    ]
}

// Right column: mode-specific actions and a usage tip.
fn right_sections() -> Vec<Line<'static>> {
    vec![
        Line::from(" --- Move Mode --- ".bold().yellow()),
        Line::from("Freemove <H/J/K/L>"),
        Line::from("Snapmove <h/j/k/l>"),
        Line::from("Quit <Esc/Enter>"),
        Line::from(""),
        Line::from(" --- Res / Scale Mode --- ".bold().yellow()),
        Line::from("Select <Space/Enter>"),
        Line::from("Quit <Esc> | Up/Down <k/j>"),
        Line::from(""),
        Line::from(" --- Tip --- ".bold().yellow()),
        Line::from("Arrow keys work as alternative".light_blue()),
        Line::from("to h/j/k/l in all modes.".light_blue()),
    ]
}

// Shared bordered frame used by both layouts.
fn help_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(" Keybindings/Help ".bold().white())
        .border_style(Style::default().fg(Color::Cyan))
}

// Renders a centered version line spanning the full popup width.
fn version_line() -> Line<'static> {
    Line::from(format!("Display-TUI v{}", env!("CARGO_PKG_VERSION")).bold().blue())
}

// Two-column layout for terminals wide enough to fit both columns.
fn render_columns(popup_area: Rect, buf: &mut Buffer) {
    let inner = popup_area.inner(Margin::new(1, 1));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    Clear.render(popup_area, buf);
    help_block().render(popup_area, buf);

    Paragraph::new(version_line())
        .alignment(Alignment::Center)
        .render(rows[0], buf);

    let left = Paragraph::new(left_sections()).wrap(Wrap { trim: false });
    let right = Paragraph::new(right_sections()).wrap(Wrap { trim: false });
    left.render(columns[0], buf);
    right.render(columns[1], buf);
}

// Fallback for narrow terminals: every section stacked in one column.
fn render_stacked(popup_area: Rect, buf: &mut Buffer) {
    let mut lines = vec![version_line(), Line::from("")];
    lines.extend(left_sections());
    lines.push(Line::from(""));
    lines.extend(right_sections());

    let p = Paragraph::new(lines)
        .block(help_block())
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Center);

    Clear.render(popup_area, buf);
    p.render(popup_area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_help(width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| HelpModal::render(f.area(), f.buffer_mut()))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn as_text(buf: &Buffer) -> String {
        buf.content.iter().map(|cell| cell.symbol()).collect()
    }

    fn find(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect();
            if let Some(x) = row.find(needle) {
                return Some((x as u16, y));
            }
        }
        None
    }

    #[test]
    fn help_rect_caps_at_max_size_on_large_terminals() {
        assert_eq!(
            responsive_help_rect(Rect::new(0, 0, 200, 60)),
            Rect::new(70, 15, 60, 30)
        );
    }

    #[test]
    fn help_rect_grows_to_fill_small_terminals() {
        assert_eq!(
            responsive_help_rect(Rect::new(0, 0, 40, 15)),
            Rect::new(1, 1, 38, 13)
        );
    }

    #[test]
    fn help_renders_two_columns_on_wide_terminal() {
        let buf = render_help(120, 40);
        let text = as_text(&buf);
        assert!(text.contains("Keybindings/Help"));
        assert!(text.contains("Display-TUI"));
        assert!(text.contains("View Mode"));
        assert!(text.contains("Res / Scale Mode"));

        // "Move Mode" lives in the right column, "View Mode" in the left.
        let (view_x, _) = find(&buf, "View Mode").unwrap();
        let (move_x, _) = find(&buf, "Move Mode").unwrap();
        let popup_x = responsive_help_rect(Rect::new(0, 0, 120, 40)).x;
        let popup_width = responsive_help_rect(Rect::new(0, 0, 120, 40)).width;
        assert!(view_x < popup_x + popup_width / 2);
        assert!(move_x >= popup_x + popup_width / 2);
    }

    #[test]
    fn help_renders_single_column_on_narrow_terminal() {
        // 60 cols < TWO_COLUMN_MIN_WIDTH so the fallback stacking is used.
        let buf = render_help(60, 30);
        let text = as_text(&buf);
        assert!(text.contains("Keybindings/Help"));
        assert!(text.contains("View Mode"));
        assert!(text.contains("Tip"));

        // Everything is stacked: "View Mode" appears above "Move Mode".
        let (_, view_y) = find(&buf, "View Mode").unwrap();
        let (_, move_y) = find(&buf, "Move Mode").unwrap();
        assert!(view_y < move_y);
    }

    #[test]
    fn help_renders_without_panicking_on_tiny_terminal() {
        let buf = render_help(20, 5);
        let text = as_text(&buf);
        // The full title is truncated on an 18-column popup, but the
        // modal still renders (border + bulk of the title) without panicking.
        assert!(text.contains("Keybindings"));
    }
}
