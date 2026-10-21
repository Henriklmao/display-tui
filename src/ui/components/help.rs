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
        Line::from(""),
        Line::from(" --- Mouse --- ".bold().yellow()),
        Line::from("Click to select; click again toggles"),
        Line::from("Drag to move; Apply saves"),
        Line::from("Pending: Apply <Enter/Space>/right-click"),
        Line::from("Cancel/Revert all <Esc/q>"),
    ]
}

// Right column: mode-specific actions and a usage tip.
fn right_sections() -> Vec<Line<'static>> {
    vec![
        Line::from(" --- Move Mode --- ".bold().yellow()),
        Line::from("Freemove <H/J/K/L>"),
        Line::from("Snapmove <h/j/k/l>"),
        Line::from("Done <Esc/Enter/q>"),
        Line::from(""),
        Line::from(" --- Res / Scale Mode --- ".bold().yellow()),
        Line::from("Apply <Enter/Space>"),
        Line::from("Up/Down <k/j>"),
        Line::from("Cancel <Esc/q>"),
        Line::from("[f] force write  [Space] auto fix"),
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

// The footer button line, mirroring the popup buttons (help closes on any
// click, so this is purely visual parity).
fn close_hint() -> Line<'static> {
    Line::from("[ Esc close ]").gray()
}

// Renders a centered version line spanning the full popup width.
fn version_line() -> Line<'static> {
    Line::from(
        format!("Display-TUI v{}", env!("CARGO_PKG_VERSION"))
            .bold()
            .blue(),
    )
}

// Draws the version line into the popup's first interior row and returns the
// interior area below it. The header gets the row unconditionally so a short
// terminal shrinks the sections instead of dropping the version; when there
// is no interior row at all the header collapses to zero height and the
// paragraph simply draws nothing.
fn render_version_header(inner: Rect, buf: &mut Buffer) -> Rect {
    let header = Rect {
        height: inner.height.min(1),
        ..inner
    };
    Paragraph::new(version_line())
        .alignment(Alignment::Center)
        .render(header, buf);

    Rect {
        y: header.bottom(),
        height: inner.height - header.height,
        ..inner
    }
}

// Two-column layout for terminals wide enough to fit both columns.
fn render_columns(popup_area: Rect, buf: &mut Buffer) {
    let inner = popup_area.inner(Margin::new(1, 1));

    Clear.render(popup_area, buf);
    help_block().render(popup_area, buf);

    let rest = render_version_header(inner, buf);
    if rest.height == 0 {
        return;
    }

    // The close hint keeps the bottom row; the sections get everything in
    // between and are simply skipped when the popup is too short.
    let footer = Rect {
        y: rest.bottom() - 1,
        height: 1,
        ..rest
    };
    let body = Rect {
        height: rest.height - 1,
        ..rest
    };

    if body.height > 0 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(body);

        let left = Paragraph::new(left_sections()).wrap(Wrap { trim: false });
        let right = Paragraph::new(right_sections()).wrap(Wrap { trim: false });
        left.render(columns[0], buf);
        right.render(columns[1], buf);
    }

    Paragraph::new(close_hint())
        .alignment(Alignment::Center)
        .render(footer, buf);
}

// Fallback for narrow terminals: every section stacked in one column.
//
// The popup is capped in height, so the blank separators are dropped to keep
// every labelled section visible. Unlike the two-column layout the version is
// part of the wrapped paragraph: on narrow terminals the full string is worth
// a second row, while the two-column header promises exactly one.
fn render_stacked(popup_area: Rect, buf: &mut Buffer) {
    let mut lines = vec![version_line(), Line::from("")];
    lines.extend(left_sections());
    lines.push(Line::from(""));
    lines.extend(right_sections());
    lines.retain(|line| line.width() > 0);
    lines.push(Line::from(""));
    lines.push(close_hint());

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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            if let Some(x) = row.find(needle) {
                return Some((x as u16, y));
            }
        }
        None
    }

    // Padding left and right of the version line within the popup interior,
    // plus the row it was drawn on.
    fn version_padding(buf: &Buffer, popup: Rect) -> (u16, u16, u16) {
        let needle = format!("Display-TUI v{}", env!("CARGO_PKG_VERSION"));
        let inner = popup.inner(Margin::new(1, 1));
        for y in inner.top()..inner.bottom() {
            let row: String = (inner.left()..inner.right())
                .map(|x| buf[(x, y)].symbol())
                .collect();
            if let Some(x) = row.find(&needle) {
                let lead = row[..x].chars().count() as u16;
                let trail = inner.width - lead - needle.chars().count() as u16;
                return (lead, trail, y);
            }
        }
        panic!(
            "version line missing at {}x{}",
            buf.area.width, buf.area.height
        );
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

    #[test]
    fn every_section_is_listed_in_both_layouts() {
        let sections = [
            "Global",
            "View Mode",
            "Mouse",
            "Move Mode",
            "Res / Scale Mode",
            "Tip",
        ];
        let bindings = [
            "Save <w>",
            "Quit <q>",
            "Preset Menu <p>",
            "Freemove",
            "Snapmove",
            "Apply <Enter",
            "Cancel <Esc/q>",
            "Pending: Apply",
            "right-click",
            "Cancel/Revert all <Esc/q>",
            "Up/Down <k/j>",
            "[f] force write",
        ];

        for (width, height) in [(120u16, 40u16), (60, 30)] {
            let text = as_text(&render_help(width, height));
            for needle in sections.iter().chain(bindings.iter()) {
                assert!(
                    text.contains(needle),
                    "{:?} missing at {}x{}",
                    needle,
                    width,
                    height
                );
            }
        }
    }

    #[test]
    fn version_line_reports_the_package_version() {
        let text = as_text(&render_help(120, 40));

        assert!(
            text.contains(&format!("Display-TUI v{}", env!("CARGO_PKG_VERSION"))),
            "version line missing: {text}"
        );
    }

    #[test]
    fn version_line_is_centred_at_the_top_in_both_layouts() {
        for (width, height) in [(120u16, 40u16), (60, 30), (40, 20)] {
            let buf = render_help(width, height);
            let popup = responsive_help_rect(Rect::new(0, 0, width, height));
            let inner = popup.inner(Margin::new(1, 1));
            let (lead, trail, y) = version_padding(&buf, popup);

            assert_eq!(
                y, inner.y,
                "version is not on the first interior row at {}x{}",
                width, height
            );
            assert!(
                lead.abs_diff(trail) <= 1,
                "version is off-centre at {}x{}: {lead} left, {trail} right",
                width,
                height
            );
        }
    }

    #[test]
    fn version_line_survives_a_short_wide_popup() {
        // Whatever vertical space is left, the two-column layout must give
        // the first interior row to the version instead of the sections.
        let width = 80;
        for height in 3..=8 {
            let buf = render_help(width, height);
            let popup = responsive_help_rect(Rect::new(0, 0, width, height));
            let inner = popup.inner(Margin::new(1, 1));

            assert!(
                popup.y + popup.height <= height,
                "popup escapes the area at 80x{height}"
            );
            if inner.height == 0 {
                // 80x3 and 80x4 leave no interior row at all; not panicking
                // is the only assertion these two sizes can make.
                continue;
            }

            let (lead, trail, y) = version_padding(&buf, popup);
            assert_eq!(y, inner.y, "version is not on the first row at 80x{height}");
            assert!(
                lead.abs_diff(trail) <= 1,
                "version is off-centre at 80x{height}: {lead} left, {trail} right"
            );
        }
    }

    #[test]
    fn a_two_row_interior_keeps_the_close_hint() {
        // At 80x6 only the footer fits below the header; it must win over a
        // single section row instead of being dropped.
        assert!(as_text(&render_help(80, 6)).contains("[ Esc close ]"));
    }

    #[test]
    fn help_rect_stays_inside_the_area_and_respects_its_caps() {
        for (width, height) in [
            (1u16, 1u16),
            (10, 4),
            (20, 8),
            (40, 15),
            (60, 30),
            (80, 24),
            (120, 40),
            (200, 60),
        ] {
            let area = Rect::new(0, 0, width, height);
            let popup = responsive_help_rect(area);

            assert!(
                popup.x >= area.x
                    && popup.y >= area.y
                    && popup.x + popup.width <= area.x + area.width
                    && popup.y + popup.height <= area.y + area.height,
                "{:?} escapes {:?}",
                popup,
                area
            );
            assert!(
                popup.width <= 60 && popup.height <= 30,
                "{:?} exceeds its caps",
                popup
            );
        }
    }

    #[test]
    fn help_frame_uses_the_cyan_border() {
        let buf = render_help(120, 40);
        let popup = responsive_help_rect(Rect::new(0, 0, 120, 40));

        assert_eq!(buf[(popup.x, popup.y)].style().fg, Some(Color::Cyan));
    }

    #[test]
    fn help_clears_only_the_popup_area() {
        let area = Rect::new(0, 0, 120, 40);
        let popup = responsive_help_rect(area);
        let mut buf = Buffer::empty(area);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_symbol("#");
            }
        }

        HelpModal::render(area, &mut buf);

        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let inside = x >= popup.x
                    && x < popup.x + popup.width
                    && y >= popup.y
                    && y < popup.y + popup.height;
                let symbol = buf[(x, y)].symbol();
                if inside {
                    assert_ne!(symbol, "#", "popup did not overwrite ({}, {})", x, y);
                } else {
                    assert_eq!(symbol, "#", "background was clobbered at ({}, {})", x, y);
                }
            }
        }
    }
}
