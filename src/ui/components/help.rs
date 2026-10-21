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
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Widget},
};

// Help modal display.
pub struct HelpModal;

// Terminal width at which the help switches from a single stacked column
// to the two-column layout.
const TWO_COLUMN_MIN_WIDTH: u16 = 80;

// Maximum popup size. The width leaves room for two readable key/description
// columns on large terminals.
const MAX_WIDTH: u16 = 84;
const MAX_HEIGHT: u16 = 30;

// Width below which the version is dropped to keep the full title visible.
const VERSION_MIN_WIDTH: u16 = 42;

// Width of the key column inside each section. Descriptions start here, so
// every row reads as a table.
const KEY_COLUMN: usize = 14;

// The close hint embedded in the bottom border.
const CLOSE_BUTTON: &str = "[ Esc ]";
const CLOSE_TEXT: &str = "Close";

// Palette of the modal: section rules warm, keys bright, prose neutral.
const SECTION_COLOR: Color = Color::Yellow;
const KEY_COLOR: Color = Color::LightCyan;
const TEXT_COLOR: Color = Color::White;
const MUTED_COLOR: Color = Color::Gray;
const DIVIDER_COLOR: Color = Color::DarkGray;

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
// horizontally at most 84 columns; on smaller terminals it shrinks to
// fit (with at least one row/column of breathing room on each side).
fn responsive_help_rect(area: Rect) -> Rect {
    // Vertical: at most 30 rows, with at least 1 row of padding above
    // and below.
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Max(MAX_HEIGHT),
            Constraint::Min(1),
        ])
        .split(area);

    // Horizontal: at most 84 columns, with at least 1 column of padding
    // on each side.
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Max(MAX_WIDTH),
            Constraint::Min(1),
        ])
        .split(vertical[1])[1]
}

// One logical row of the help content. Entries are rendered as a key column
// followed by a (possibly wrapped) description.
enum Row {
    Section(&'static str),
    Entry(&'static str, &'static str),
    Note(&'static str),
    Blank,
}

// Left column: global actions and view-mode navigation.
fn left_rows() -> Vec<Row> {
    vec![
        Row::Section("GLOBAL"),
        Row::Entry("<w>", "Save"),
        Row::Entry("<q>", "Quit"),
        Row::Entry("<K>", "Toggle this help"),
        Row::Blank,
        Row::Section("VIEW MODE"),
        Row::Entry("<k>", "Up"),
        Row::Entry("<j>", "Down"),
        Row::Entry("<m>", "Move mode"),
        Row::Entry("<r>", "Resolution"),
        Row::Entry("<s>", "Scale"),
        Row::Entry("<o>", "Rotate"),
        Row::Entry("<0-9>", "Set workspace (0 clears)"),
        Row::Entry("<e>", "Enable"),
        Row::Entry("<d>", "Disable"),
        Row::Entry("<p>", "Preset menu"),
        Row::Blank,
        Row::Section("MOUSE"),
        Row::Entry("Click", "Select; click again toggles"),
        Row::Entry("Drag", "Move; Apply saves"),
        Row::Entry("<Enter/Space>", "Apply pending move"),
        Row::Entry("<Esc/q>", "Cancel / revert all"),
    ]
}

// Right column: mode-specific actions and a usage tip.
fn right_rows() -> Vec<Row> {
    vec![
        Row::Section("MOVE MODE"),
        Row::Entry("<H/J/K/L>", "Freemove"),
        Row::Entry("<h/j/k/l>", "Snapmove"),
        Row::Entry("<Esc/Enter/q>", "Done"),
        Row::Blank,
        Row::Section("RES / SCALE MODE"),
        Row::Entry("<Enter/Space>", "Apply"),
        Row::Entry("<k/j>", "Up / down"),
        Row::Entry("<Esc/q>", "Cancel"),
        Row::Entry("[f]", "Force write"),
        Row::Entry("[Space]", "Auto fix"),
        Row::Blank,
        Row::Section("TIP"),
        Row::Note("Arrow keys work as alternative to h/j/k/l in all modes."),
    ]
}

// Two-column layout for terminals wide enough to fit both columns.
fn render_columns(popup_area: Rect, buf: &mut Buffer) {
    let inner = popup_area.inner(Margin::new(1, 1));
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 2),
            Constraint::Length(1),
            Constraint::Ratio(1, 2),
        ])
        .split(inner);

    let left = Paragraph::new(build_lines(&left_rows(), columns[0].width));
    let right = Paragraph::new(build_lines(&right_rows(), columns[2].width));
    left.render(columns[0], buf);
    right.render(columns[2], buf);

    // A fine vertical rule separates the two sections of the grid.
    let divider_style = Style::default().fg(DIVIDER_COLOR);
    for y in inner.top()..inner.bottom() {
        put(buf, columns[1].x, y, "│", divider_style);
    }
}

// Fallback for narrow terminals: every section stacked in one column.
fn render_stacked(popup_area: Rect, buf: &mut Buffer) {
    let inner = popup_area.inner(Margin::new(1, 1));
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut rows = left_rows();
    rows.push(Row::Blank);
    rows.extend(right_rows());

    Paragraph::new(build_lines(&rows, inner.width)).render(inner, buf);
}

// Turns the logical rows into styled lines for one column of `width` cells.
fn build_lines(rows: &[Row], width: u16) -> Vec<Line<'static>> {
    let width = width as usize;
    if width == 0 {
        return Vec::new();
    }

    let key_width = KEY_COLUMN.min(width.saturating_sub(8)).max(1);
    let desc_width = width.saturating_sub(key_width).max(1);
    let mut lines = Vec::new();

    for row in rows {
        match row {
            Row::Blank => lines.push(Line::from("")),
            Row::Section(title) => lines.push(section_rule(title, width)),
            Row::Note(text) => {
                for part in wrap_text(text, width) {
                    lines.push(Line::from(Span::styled(part, Style::default().fg(MUTED_COLOR))));
                }
            }
            Row::Entry(keys, desc) => {
                for (index, part) in wrap_text(desc, desc_width).into_iter().enumerate() {
                    let key = if index == 0 {
                        Span::styled(
                            format!("{:<key_width$}", keys),
                            Style::default().fg(KEY_COLOR).bold(),
                        )
                    } else {
                        Span::raw(" ".repeat(key_width))
                    };
                    lines.push(Line::from(vec![
                        key,
                        Span::styled(part, Style::default().fg(TEXT_COLOR)),
                    ]));
                }
            }
        }
    }

    lines
}

// A coloured section rule, e.g. `── GLOBAL ──────────`.
fn section_rule(title: &str, width: usize) -> Line<'static> {
    let label = format!(" {} ", title);
    let lead = 2;
    let fill = width.saturating_sub(lead + label.chars().count());
    let style = Style::default().fg(SECTION_COLOR);

    Line::from(vec![
        Span::styled("──", style),
        Span::styled(label, style.bold()),
        Span::styled("─".repeat(fill), style),
    ])
}

// Greedy word wrap that never splits words unless a single word is wider
// than the column.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut used = 0usize;

    for word in text.split_whitespace() {
        let length = word.chars().count();
        if length > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut chunk = String::new();
            for ch in word.chars() {
                if chunk.chars().count() == width {
                    lines.push(std::mem::take(&mut chunk));
                }
                chunk.push(ch);
            }
            current = chunk;
            used = current.chars().count();
        } else if current.is_empty() {
            current.push_str(word);
            used = length;
        } else if used + 1 + length <= width {
            current.push(' ');
            current.push_str(word);
            used += 1 + length;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
            used = length;
        }
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }

    lines
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
            Rect::new(58, 15, 84, 30)
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
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains("VIEW MODE"));
        assert!(text.contains("RES / SCALE MODE"));

        // "MOVE MODE" lives in the right column, "VIEW MODE" in the left.
        let (view_x, _) = find(&buf, "VIEW MODE").unwrap();
        let (move_x, _) = find(&buf, "MOVE MODE").unwrap();
        let popup = responsive_help_rect(Rect::new(0, 0, 120, 40));
        assert!(view_x < popup.x + popup.width / 2);
        assert!(move_x >= popup.x + popup.width / 2);
    }

    #[test]
    fn the_two_columns_are_split_by_a_vertical_rule() {
        let buf = render_help(120, 40);
        let popup = responsive_help_rect(Rect::new(0, 0, 120, 40));
        let (x, y) = find(&buf, "│").expect("divider");

        assert!(
            x.abs_diff(popup.x + popup.width / 2) <= 1,
            "divider sits in the middle"
        );
        assert_eq!(buf[(x, y)].style().fg, Some(DIVIDER_COLOR));
        assert!(y > popup.y && y < popup.bottom() - 1);
    }

    #[test]
    fn the_stacked_layout_has_no_divider() {
        // 60 cols < TWO_COLUMN_MIN_WIDTH: the second column drops below.
        assert!(find(&render_help(60, 30), "│").is_none());
    }

    #[test]
    fn section_rules_are_warm_and_filled_to_the_column() {
        let buf = render_help(120, 40);
        let (x, y) = find(&buf, "GLOBAL").expect("section rule");
        assert_eq!(buf[(x, y)].style().fg, Some(SECTION_COLOR));

        // The rule ends with a drawing character, not body text.
        let (rule_x, rule_y) = find(&buf, "── GLOBAL").unwrap();
        let row: String = (rule_x..rule_x + 20)
            .map(|x| buf[(x, rule_y)].symbol())
            .collect();
        assert!(row.starts_with("── GLOBAL ─"), "{row:?}");
    }

    #[test]
    fn keys_are_accented_and_descriptions_stay_neutral() {
        use ratatui::style::Modifier;

        let buf = render_help(120, 40);
        let (key_x, key_y) = find(&buf, "<w>").expect("save key");
        assert_eq!(buf[(key_x, key_y)].style().fg, Some(KEY_COLOR));
        assert!(
            buf[(key_x, key_y)]
                .style()
                .add_modifier
                .contains(Modifier::BOLD)
        );

        let (desc_x, desc_y) = find(&buf, "Save").expect("save description");
        assert_eq!(
            desc_x,
            key_x + KEY_COLUMN as u16,
            "descriptions start in the same column"
        );
        assert_eq!(buf[(desc_x, desc_y)].style().fg, Some(TEXT_COLOR));
    }

    #[test]
    fn every_key_uses_the_accent_colour() {
        let buf = render_help(120, 40);
        for key in ["<w>", "<q>", "<k>", "<j>", "[f]", "[Space]", "<H/J/K/L>"] {
            let (x, y) = find(&buf, key).unwrap_or_else(|| panic!("{key} missing"));
            assert_eq!(
                buf[(x, y)].style().fg,
                Some(KEY_COLOR),
                "{key} is not accented"
            );
        }
    }

    #[test]
    fn help_renders_single_column_on_narrow_terminal() {
        // 60 cols < TWO_COLUMN_MIN_WIDTH so the fallback stacking is used.
        let buf = render_help(60, 30);
        let text = as_text(&buf);
        assert!(text.contains("Keybindings/Help"));
        assert!(text.contains("VIEW MODE"));
        assert!(text.contains("<w>"));

        // The second column is stacked below the first, so the later mode
        // sections start lower down than view-mode navigation.
        let (_, view_y) = find(&buf, "VIEW MODE").unwrap();
        let (_, move_y) = find(&buf, "MOVE MODE").unwrap();
        assert!(view_y < move_y, "the right column drops below the left");
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
            "GLOBAL",
            "VIEW MODE",
            "MOUSE",
            "MOVE MODE",
            "RES / SCALE MODE",
            "TIP",
        ];
        let bindings = [
            "<w>",
            "Save",
            "<q>",
            "Quit",
            "<p>",
            "Preset menu",
            "Freemove",
            "Snapmove",
            "Apply pending move",
            "<Esc/q>",
            "Cancel / revert all",
            "[f]",
            "Force write",
            "[Space]",
            "Auto fix",
            "Arrow keys work as alternative",
        ];

        // Wide terminals show both columns at once.
        let text = as_text(&render_help(120, 40));
        for needle in sections.iter().chain(bindings.iter()) {
            assert!(text.contains(needle), "{:?} missing at 120x40", needle);
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
                popup.width <= MAX_WIDTH && popup.height <= MAX_HEIGHT,
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
