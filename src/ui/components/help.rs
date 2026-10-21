//! Help modal component.
//!
//! The help popup adapts to the terminal size: it grows up to a maximum
//! size (30 rows x 60 cols) but shrinks gracefully on small terminals.
//! On wide terminals the keybindings are shown in two side-by-side
//! columns; below [`TWO_COLUMN_MIN_WIDTH`] every section is stacked into
//! a single column instead.
//!
//! The frame is a double-line box drawn by hand so the title, the version
//! and the close hint can sit inside the border itself: the title is embedded
//! top-left, the version top-right and the footer is centred in the bottom
//! line.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Clear, Paragraph, Widget, Wrap},
};

// Help modal display.
pub struct HelpModal;

// Terminal width at which the help switches from a single stacked column
// to the two-column layout.
const TWO_COLUMN_MIN_WIDTH: u16 = 80;

// Width below which the version is dropped to keep the full title visible.
const VERSION_MIN_WIDTH: u16 = 42;

// The close hint embedded in the bottom border.
const CLOSE_BUTTON: &str = "[ Esc ]";
const CLOSE_TEXT: &str = "Close";

impl HelpModal {
    pub fn render(area: Rect, buf: &mut Buffer) {
        let popup_area = responsive_help_rect(area);

        Clear.render(popup_area, buf);

        if area.width >= TWO_COLUMN_MIN_WIDTH {
            render_columns(popup_area, buf);
        } else {
            render_stacked(popup_area, buf);
        }

        // The frame is drawn last so the content never overwrites a border.
        draw_frame(popup_area, buf);
    }

    // The popup rectangle for a terminal area. Shared by the renderer and the
    // mouse hit-testing so a click lands on exactly what was drawn.
    pub fn rect(area: Rect) -> Rect {
        responsive_help_rect(area)
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

// Two-column layout for terminals wide enough to fit both columns.
fn render_columns(popup_area: Rect, buf: &mut Buffer) {
    let inner = popup_area.inner(Margin::new(1, 1));
    if inner.height == 0 {
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left = Paragraph::new(left_sections()).wrap(Wrap { trim: false });
    let right = Paragraph::new(right_sections()).wrap(Wrap { trim: false });
    left.render(columns[0], buf);
    right.render(columns[1], buf);
}

// Fallback for narrow terminals: every section stacked in one column.
fn render_stacked(popup_area: Rect, buf: &mut Buffer) {
    let inner = popup_area.inner(Margin::new(1, 1));
    if inner.height == 0 {
        return;
    }

    let mut lines = left_sections();
    lines.push(Line::from(""));
    lines.extend(right_sections());
    lines.retain(|line| line.width() > 0);

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .alignment(ratatui::layout::Alignment::Left);

    p.render(inner, buf);
}

// Draws the double-line frame, the embedded title/version and the footer.
fn draw_frame(popup: Rect, buf: &mut Buffer) {
    if popup.width < 2 || popup.height < 2 {
        return;
    }

    let border = Style::default().fg(Color::Cyan);
    let left = popup.left();
    let right = popup.right() - 1;
    let top = popup.top();
    let bottom = popup.bottom() - 1;

    for x in (left + 1)..right {
        put(buf, x, top, "═", border);
        put(buf, x, bottom, "═", border);
    }
    for y in (top + 1)..bottom {
        put(buf, left, y, "║", border);
        put(buf, right, y, "║", border);
    }

    put(buf, left, top, "╔", border);
    put(buf, right, top, "╗", border);
    put(buf, left, bottom, "╚", border);
    put(buf, right, bottom, "╝", border);

    // The version is dropped on narrow popups so the title keeps its room.
    let version = format!(" v{} ", env!("CARGO_PKG_VERSION"));
    let version_start = if popup.width >= VERSION_MIN_WIDTH {
        let start = right.saturating_sub(version.len() as u16);
        draw_spans(
            buf,
            top,
            start,
            right,
            &[(&version, Style::default().fg(Color::Gray))],
        );
        start
    } else {
        right
    };

    // Title, embedded top-left; clipped so it never runs into the version.
    let title: &[(&str, Style)] = &[
        ("⌨", Style::default().fg(Color::LightCyan).bold()),
        (" Keybindings/Help ", Style::default().white().bold()),
    ];
    draw_spans(buf, top, left + 2, version_start, title);

    // Footer, centred in the bottom border.
    let footer: &[(&str, Style)] = &[
        (CLOSE_BUTTON, Style::default().fg(Color::LightCyan).bold()),
        (" ", Style::default().fg(Color::Gray)),
        (CLOSE_TEXT, Style::default().white()),
    ];
    let width = footer.iter().map(|(text, _)| text.len() as u16).sum::<u16>();
    let start = left + popup.width.saturating_sub(width) / 2;
    draw_spans(buf, bottom, start, right, footer);
}

// Writes a styled string into a row, stopping before `limit` (exclusive).
fn draw_spans(buf: &mut Buffer, y: u16, mut x: u16, limit: u16, spans: &[(&str, Style)]) {
    for (text, style) in spans {
        for ch in text.chars() {
            if x >= limit {
                return;
            }
            put(buf, x, y, &ch.to_string(), *style);
            x += 1;
        }
    }
}

// Writes one cell, ignoring coordinates outside the buffer.
fn put(buf: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if x < buf.area.right() && y < buf.area.bottom() {
        buf[(x, y)].set_symbol(symbol);
        buf[(x, y)].set_style(style);
    }
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
        // Multi-byte symbols (⌨) shift byte offsets, so reuse the column-aware
        // helper from the test utilities.
        crate::test_utils::tests::find_text(buf, needle)
    }

    // The row text inside the popup border columns, excluding the frame.
    fn inner_row(buf: &Buffer, popup: Rect, y: u16) -> String {
        (popup.left() + 1..popup.right() - 1)
            .map(|x| buf[(x, y)].symbol())
            .collect()
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
        assert!(text.contains("Display-TUI") || text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains("View Mode"));
        assert!(text.contains("Res / Scale Mode"));

        // "Move Mode" lives in the right column, "View Mode" in the left.
        let (view_x, _) = find(&buf, "View Mode").unwrap();
        let (move_x, _) = find(&buf, "Move Mode").unwrap();
        let popup = responsive_help_rect(Rect::new(0, 0, 120, 40));
        assert!(view_x < popup.x + popup.width / 2);
        assert!(move_x >= popup.x + popup.width / 2);
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
    fn the_frame_uses_double_lines() {
        let buf = render_help(120, 40);
        let popup = responsive_help_rect(Rect::new(0, 0, 120, 40));

        assert_eq!(buf[(popup.x, popup.y)].symbol(), "╔");
        assert_eq!(buf[(popup.right() - 1, popup.y)].symbol(), "╗");
        assert_eq!(buf[(popup.x, popup.bottom() - 1)].symbol(), "╚");
        assert_eq!(
            buf[(popup.right() - 1, popup.bottom() - 1)].symbol(),
            "╝"
        );
        assert_eq!(buf[(popup.x, popup.y + 1)].symbol(), "║");
        assert_eq!(buf[(popup.x + 1, popup.y)].symbol(), "═");

        // A horizontal cell outside the embedded title keeps the border style.
        assert_eq!(buf[(popup.x + 1, popup.y)].style().fg, Some(Color::Cyan));
    }

    #[test]
    fn the_title_and_version_are_embedded_in_the_top_border() {
        let buf = render_help(120, 40);
        let popup = responsive_help_rect(Rect::new(0, 0, 120, 40));
        let top = inner_row(&buf, popup, popup.y);

        assert!(
            top.contains("⌨ Keybindings/Help"),
            "title is not embedded: {top:?}"
        );
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        assert!(top.contains(&version), "version is not embedded: {top:?}");

        // The title sits left, the version right.
        let title_at = top.find("Keybindings/Help").unwrap();
        let version_at = top.find(&version).unwrap();
        assert!(title_at < version_at, "title must come before version");
        assert!(
            top[version_at..].trim_end_matches('═').trim_end().len() <= version.len() + 2,
            "version is not right-aligned: {top:?}"
        );

        let (vx, vy) = find(&buf, &version).unwrap();
        assert_eq!(vy, popup.y, "version is on the top border");
        assert_eq!(
            buf[(vx, vy)].style().fg,
            Some(Color::Gray),
            "version must be dezent grey"
        );
    }

    #[test]
    fn the_title_and_version_fit_without_overlapping() {
        // On a 60-column popup there is room for both, separated.
        let buf = render_help(80, 30);
        let popup = responsive_help_rect(Rect::new(0, 0, 80, 30));
        let top = inner_row(&buf, popup, popup.y);

        assert!(top.contains("Keybindings/Help"), "{top:?}");
        assert!(top.contains(env!("CARGO_PKG_VERSION")), "{top:?}");
    }

    #[test]
    fn the_version_is_dropped_on_a_narrow_popup() {
        // The title has priority: at 40 columns the version would crowd it out.
        let buf = render_help(40, 20);
        let text = as_text(&buf);

        assert!(text.contains("Keybindings/Help"), "{text}");
    }

    #[test]
    fn the_close_hint_is_embedded_in_the_bottom_border() {
        let buf = render_help(120, 40);
        let popup = responsive_help_rect(Rect::new(0, 0, 120, 40));
        let bottom = inner_row(&buf, popup, popup.bottom() - 1);

        assert!(
            bottom.contains(&format!("{CLOSE_BUTTON} {CLOSE_TEXT}")),
            "footer is not embedded: {bottom:?}"
        );
        let at = bottom.find(CLOSE_BUTTON).unwrap();
        let before = bottom[..at].chars().count();
        let trailing =
            bottom.chars().count() - (before + CLOSE_BUTTON.len() + 1 + CLOSE_TEXT.len());
        assert!(
            before.abs_diff(trailing) <= 1,
            "footer is not centred: {before} before, {trailing} after"
        );

        let (bx, by) = find(&buf, CLOSE_BUTTON).unwrap();
        assert_eq!(by, popup.bottom() - 1, "footer is on the bottom border");
        assert_eq!(
            buf[(bx, by)].style().fg,
            Some(Color::LightCyan),
            "the key is accented"
        );
    }

    #[test]
    fn the_close_hint_survives_a_short_terminal() {
        // The footer lives in the border, so a short popup cannot drop it.
        assert!(as_text(&render_help(80, 6)).contains("[ Esc ] Close"));
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
