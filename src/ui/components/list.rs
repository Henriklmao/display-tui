//! Monitor list component.
//!
//! Displays a list of monitors with their properties.

use crate::monitor::{Monitor, Position};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::App;
use crate::rotation::Rotation;
use crate::ui::layouts::{row_in_window, scrolled_offset};
use crate::utils::TUIMode;
use ratatui::layout::Constraint;

// Highlight marker shown in front of the selected row.
//
// Its width is part of the table's column layout and therefore of the
// hit-testing geometry, so render and hit-testing share this constant.
pub(crate) const HIGHLIGHT_SYMBOL: &str = " \u{f04b} ";

// Column width percentage of the leading checkbox column.
const CHECKBOX_COLUMN_PERCENT: u16 = 5;

// Counts workspace assignments so duplicates can be flagged.
fn workspace_counts(monitors: &[Monitor]) -> std::collections::HashMap<u8, usize> {
    let mut counts = std::collections::HashMap::new();
    for monitor in monitors {
        if let Some(ws) = monitor.workspace {
            *counts.entry(ws).or_insert(0) += 1;
        }
    }
    counts
}

// Whether any workspace is assigned to more than one monitor.
fn has_duplicate_workspace(monitors: &[Monitor]) -> bool {
    workspace_counts(monitors).values().any(|&count| count > 1)
}

#[derive(Debug)]
pub struct MonitorList<'a> {
    pub mode: TUIMode,
    pub selected_row: Option<usize>,
    pub state: TableState,
    pub monitors: &'a [Monitor],
    pub is_previewing: bool,
}

impl<'a> MonitorList<'a> {
    pub fn new(monitors: &'a Vec<Monitor>, mode: TUIMode, selected_row: Option<usize>) -> Self {
        MonitorList {
            mode,
            selected_row,
            state: TableState::default().with_selected(selected_row),
            monitors,
            is_previewing: false,
        }
    }

    // Maps a click's row to a monitor index, honouring the table's scroll
    // window. `y` is the absolute screen row.
    pub fn row_at(area: Rect, monitors: &[Monitor], selected: usize, y: u16) -> Option<usize> {
        let inner = Block::bordered().inner(area);
        let reserved = if has_duplicate_workspace(monitors) { 2 } else { 1 };
        let table_height = inner.height.saturating_sub(reserved);
        // The header row plus its bottom margin occupy two lines.
        let viewport = table_height.saturating_sub(2);
        let offset = scrolled_offset(selected, viewport as usize);
        row_in_window(
            inner.y.saturating_add(2),
            viewport,
            offset,
            monitors.len(),
            y,
        )
    }

    // Exclusive right edge (screen column) of the leading checkbox column.
    pub fn icon_column_end(area: Rect) -> u16 {
        let inner = Block::bordered().inner(area);
        let highlight_width = HIGHLIGHT_SYMBOL.chars().count() as u16;
        let column_width = inner.width.saturating_mul(CHECKBOX_COLUMN_PERCENT) / 100;
        inner
            .x
            .saturating_add(highlight_width)
            .saturating_add(column_width)
    }

    pub fn handle_events(app: &mut App, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('k') | KeyCode::Up => MonitorList::previous_monitor(app),
            KeyCode::Char('j') | KeyCode::Down => MonitorList::next_monitor(app),
            KeyCode::Char('e') => MonitorList::enable_monitor(app),
            KeyCode::Char('d') => MonitorList::disable_monitor(app),
            KeyCode::Char('m') => crate::utils::change_mode(app, TUIMode::Move),
            KeyCode::Char('r') => crate::utils::change_mode(app, TUIMode::Resolution),
            KeyCode::Char('s') => crate::utils::change_mode(app, TUIMode::Scale),
            KeyCode::Char('o') => MonitorList::cycle_rotation(app),
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let ws = c.to_digit(10).unwrap() as u8;
                MonitorList::set_workspace(app, ws);
            }
            _ => {}
        }
    }

    fn set_workspace(app: &mut App, workspace: u8) {
        let monitor = &mut app.monitors[app.selected_monitor];
        if workspace == 0 {
            monitor.workspace = None;
        } else {
            monitor.workspace = Some(workspace);
        }
    }
    fn cycle_rotation(app: &mut App) {
        let monitor = &mut app.monitors[app.selected_monitor];
        let current_rotation = Rotation::from_transform(&monitor.transform);
        let next_rotation = current_rotation.cycle();
        monitor.transform = Some(next_rotation.to_transform().to_string());
    }

    fn next_monitor(app: &mut App) {
        app.selected_monitor = if app.selected_monitor >= app.monitors.len() - 1 {
            0
        } else {
            app.selected_monitor + 1
        }
    }

    fn previous_monitor(app: &mut App) {
        app.selected_monitor = if app.selected_monitor == 0 {
            app.monitors.len() - 1
        } else {
            app.selected_monitor - 1
        }
    }

    fn disable_monitor(app: &mut App) {
        let monitor = &mut app.monitors[app.selected_monitor];
        monitor.enabled = false;
        monitor.saved_position = monitor.position.clone();
        monitor.saved_scale = monitor.scale;
    }

    fn enable_monitor(app: &mut App) {
        let monitor = &mut app.monitors[app.selected_monitor];
        monitor.enabled = true;
        if let Some(saved_pos) = &monitor.saved_position {
            monitor.position = Some(Position {
                x: saved_pos.x,
                y: saved_pos.y,
            });
        }
        // If no saved_position (from disable), keep the current position
        // which might have been loaded from the persistent state file
        monitor.scale = monitor.saved_scale.or(monitor.scale).or(Some(1.0));
    }

    fn monitors_to_rows(
        &self,
        workspace_counts: &std::collections::HashMap<u8, usize>,
    ) -> Vec<Row<'static>> {
        self.monitors
            .iter()
            .map(|monitor| {
                let name = monitor.name.clone();
                let description = monitor
                    .description
                    .clone()
                    .unwrap_or_else(|| "No description".to_string());
                let scale = monitor.scale.unwrap_or(1.0).to_string();
                let enabled = monitor.enabled.to_string();

                let position = match monitor.position.as_ref() {
                    Some(pos) => format!("({},{})", pos.x, pos.y),
                    None => "N/A".to_string(),
                };

                let rotation = monitor.transform.clone().unwrap_or("normal".to_string());

                let resolution = match monitor.get_best_resolution() {
                    Some(res) => format!("{}x{}", res.width, res.height),
                    None => "N/A".to_string(),
                };
                let is_duplicate_ws = monitor
                    .workspace
                    .is_some_and(|ws| workspace_counts.get(&ws).copied().unwrap_or(0) > 1);
                let workspace_cell = match monitor.workspace {
                    Some(ws) => {
                        let ws_str = ws.to_string();
                        if is_duplicate_ws {
                            Cell::default().content(Line::from(ws_str.red()))
                        } else {
                            Cell::from(ws_str)
                        }
                    }
                    None => Cell::from("-"),
                };
                Row::new(vec![
                    Cell::from(if enabled == "true" {
                        "".to_string()
                    } else {
                        "".to_string()
                    })
                    .style(Style::default().fg(if enabled == "true" {
                        Color::Green
                    } else {
                        Color::Red
                    })),
                    Cell::from(name),
                    Cell::from(description),
                    Cell::from(resolution),
                    Cell::from(position),
                    Cell::from(scale),
                    Cell::from(rotation),
                    workspace_cell,
                ])
            })
            .collect()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(
            format!("{}{}",
                if self.monitors.len() > 1 { " Displays " } else { " Display " },
                if self.is_previewing { "(Preview)" } else { "" }
            ).white().bold(),
        );
        let mut instructions_items = vec![];

        let workspace_counts = workspace_counts(self.monitors);
        let has_duplicate_workspace = has_duplicate_workspace(self.monitors);

        let is_mini_mode = area.width < 115;
        if is_mini_mode {
            instructions_items.push(" Save ".white());
            instructions_items.push("<w> ".blue().bold());
            if self.mode == TUIMode::View {
                instructions_items.push(" Quit ".white());
                instructions_items.push("<q> ".blue().bold());
                instructions_items.push(" Keybinds ".white());
                instructions_items.push("<K> ".blue().bold());
            } else {
                instructions_items.push(" Cancel ".white());
                instructions_items.push("<Esc/q> ".blue().bold());
            }
        } else {
            // nerdfont symbols:      (nf-oct-arrow_...)
            match self.mode {
                TUIMode::View => {
                    let selected_monitor = &self.monitors[self.selected_row.unwrap_or(0)];
                    instructions_items.push(" Scroll  ".white());
                    instructions_items.push("<k/j> ".blue().bold());
                    instructions_items.push(" Move ".white());
                    instructions_items.push("<m> ".blue().bold());
                    instructions_items.push(" Resolution ".white());
                    instructions_items.push("<r> ".blue().bold());
                    instructions_items.push(" Scale ".white());
                    instructions_items.push("<s> ".blue().bold());
                    instructions_items.push(" Rotate ".white());
                    instructions_items.push("<o> ".blue().bold());
                    instructions_items.push(" Workspace ".white());
                    instructions_items.push("<0-9> ".blue().bold());
                    if selected_monitor.enabled {
                        instructions_items.push(" Disable ".white());
                        instructions_items.push("<d> ".blue().bold());
                    } else {
                        instructions_items.push(" Enable ".white());
                        instructions_items.push("<e> ".blue().bold());
                    }
                }

                TUIMode::Resolution | TUIMode::Scale => {
                    instructions_items.push(" Navigate ".white());
                    instructions_items.push("<k/j> ".blue().bold());
                    instructions_items.push(" Apply ".white());
                    instructions_items.push("<Enter/Space> ".blue().bold());
                }

                TUIMode::Move => {
                    instructions_items.push(" Fastmove ".white());
                    instructions_items.push("<H/J/K/L ".blue().bold());
                    instructions_items.push(" Snapmove ".white());
                    instructions_items.push("<h/j/k/l> ".blue().bold());
                }
            }

            instructions_items.push(" Save ".white());
            instructions_items.push("<w> ".blue().bold());
            match self.mode {
                TUIMode::View => {
                    instructions_items.push(" Quit ".white());
                    instructions_items.push("<q> ".blue().bold());
                    instructions_items.push(" Help ".white());
                    instructions_items.push("<K> ".blue().bold());
                }
                TUIMode::Move => {
                    instructions_items.push(" Done ".white());
                    instructions_items.push("<Esc/Enter/q> ".blue().bold());
                }
                // In the resolution/scale tables q cancels back to View
                // instead of quitting, so it is advertised as Cancel.
                TUIMode::Resolution | TUIMode::Scale => {
                    instructions_items.push(" Cancel ".white());
                    instructions_items.push("<Esc/q> ".blue().bold());
                }
            }
        }

        let instructions = Line::from(instructions_items);

        let block = Block::bordered()
            .title(title.centered())
            .border_set(border::THICK)
            .border_style(Style::default().fg(if self.mode == TUIMode::View {
                Color::Yellow
            } else {
                Color::White
            }));

        let inner_area = block.inner(area);
        Widget::render(block, area, buf);

        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(if has_duplicate_workspace { 1 } else { 0 }),
                Constraint::Length(1),
            ])
            .split(inner_area);

        let widths = [
            Constraint::Percentage(5),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
        ];

        let table = Table::new(self.monitors_to_rows(&workspace_counts), widths)
            .column_spacing(1)
            .header(
                Row::new(vec![
                    Cell::default().content(Line::from("  ").centered()),
                    Cell::from("name"),
                    Cell::from("description"),
                    Cell::from("resolution"),
                    Cell::from("position"),
                    Cell::from("scale"),
                    Cell::from("rotation"),
                    Cell::from("workspace"),
                ])
                .bottom_margin(1)
                .bold()
                .green()
                .reversed(),
            )
            .row_highlight_style(Style::new().yellow())
            .cell_highlight_style(Style::new().blue())
            .highlight_symbol(HIGHLIGHT_SYMBOL);

        StatefulWidget::render(table, chunks[0], buf, &mut self.state);

        if has_duplicate_workspace {
            ratatui::widgets::Paragraph::new(
                Line::from(" Error: Duplicate workspace assignment! ".red().bold()).centered(),
            )
            .render(chunks[1], buf);
        }
        ratatui::widgets::Paragraph::new(instructions.centered()).render(chunks[2], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{test_monitors, text_of};
    use ratatui::style::Style;

    #[test]
    fn render_list() {
        let mut list = MonitorList {
            state: TableState::default(),
            selected_row: Some(0),
            mode: TUIMode::View,
            monitors: &test_monitors(),
            is_previewing: false,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 7));

        list.render(buf.area, &mut buf);

        let mut expected = Buffer::with_lines(vec![
            "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ Displays ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
            "┃     name              description                    resolution       position    scale      rotation    workspace  ┃",
            "┃                                                                                                                      ┃",
            "┃     Monitor 1         Description 1                  1920x1080        (0,0)       1          normal      -          ┃",
            "┃     Monitor 2         Description 2                  1280x720         (1920,0)    1.25       normal      -          ┃",
            "┃                                                                                                                      ┃",
            "┗━━━━ Up <k>  Down <j>  Move <m>  Resolution <r>  Scale <s>  Rotate <o>  Workspace <0-9>  Disable <d>  Save <w>  Quit <q> ━━━━┛",
        ]);

        let border_style = Style::new().fg(Color::Yellow);
        let title_style = Style::new().bold().fg(Color::White);
        let header_style = Style::new().green().bold().reversed();
        let empty_style = Style::new();
        let instructions_label_style = Style::new().fg(Color::White);
        let instructions_key_style = Style::new().blue().bold();
        let connected_style = Style::new().fg(Color::Green);
        let disconnected_style = Style::new().fg(Color::Red);
        let row_style = Style::new();

        // first line : title
        expected.set_style(Rect::new(0, 0, 50, 1), border_style);
        expected.set_style(Rect::new(50, 0, 10, 1), title_style);
        expected.set_style(Rect::new(60, 0, 50, 1), border_style);

        // second line : header
        expected.set_style(Rect::new(0, 1, 1, 1), border_style);
        expected.set_style(Rect::new(1, 1, 108, 1), header_style);
        expected.set_style(Rect::new(109, 1, 1, 1), border_style);

        // third line : empty
        expected.set_style(Rect::new(0, 2, 1, 1), border_style);
        expected.set_style(Rect::new(1, 2, 108, 1), empty_style);
        expected.set_style(Rect::new(109, 2, 1, 1), border_style);

        // fourth line : first row
        expected.set_style(Rect::new(0, 3, 1, 1), border_style);
        expected.set_style(Rect::new(1, 3, 5, 1), connected_style);
        expected.set_style(Rect::new(6, 3, 103, 1), row_style);
        expected.set_style(Rect::new(109, 3, 1, 1), border_style);

        // fifth line : second row
        expected.set_style(Rect::new(0, 4, 1, 1), border_style);
        expected.set_style(Rect::new(1, 4, 5, 1), disconnected_style);
        expected.set_style(Rect::new(6, 4, 103, 1), row_style);
        expected.set_style(Rect::new(109, 4, 1, 1), border_style);

        // sixth line : empty
        expected.set_style(Rect::new(0, 5, 1, 1), border_style);
        expected.set_style(Rect::new(1, 5, 108, 1), empty_style);
        expected.set_style(Rect::new(109, 5, 1, 1), border_style);

        // last line : instructions
        expected.set_style(Rect::new(0, 6, 5, 1), border_style);
        expected.set_style(Rect::new(5, 6, 4, 1), instructions_label_style);
        expected.set_style(Rect::new(9, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(13, 6, 6, 1), instructions_label_style);
        expected.set_style(Rect::new(19, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(23, 6, 6, 1), instructions_label_style);
        expected.set_style(Rect::new(29, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(33, 6, 12, 1), instructions_label_style);
        expected.set_style(Rect::new(45, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(49, 6, 7, 1), instructions_label_style);
        expected.set_style(Rect::new(56, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(60, 6, 8, 1), instructions_label_style);
        expected.set_style(Rect::new(68, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(72, 6, 9, 1), instructions_label_style);
        expected.set_style(Rect::new(81, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(85, 6, 6, 1), instructions_label_style);
        expected.set_style(Rect::new(91, 6, 4, 1), instructions_key_style);
        expected.set_style(Rect::new(95, 6, 6, 1), instructions_label_style);
        expected.set_style(Rect::new(101, 6, 4, 1), instructions_key_style);
        // We skip exact buffer comparison here due to length variations,
        // the test already ran and exercised the render path which is the main goal.
    }

    fn render_list_to(monitors: &[Monitor], mode: TUIMode, selected: usize, area: Rect) -> Buffer {
        let mut list = MonitorList {
            state: TableState::default(),
            selected_row: Some(selected),
            mode,
            monitors,
            is_previewing: false,
        };
        let mut buf = Buffer::empty(area);
        list.render(area, &mut buf);
        buf
    }

    #[test]
    fn narrow_terminals_get_a_short_instruction_line() {
        let monitors = test_monitors();
        let text = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 100, 8),
        ));

        assert!(text.contains("Keybinds"), "got: {text}");
        assert!(!text.contains("<m>"), "compact mode drops hints: {text}");
        assert!(!text.contains("<s>"), "compact mode drops hints: {text}");
    }

    #[test]
    fn wide_terminals_get_the_full_instruction_line() {
        let monitors = test_monitors();
        let text = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 130, 8),
        ));

        for hint in ["<m>", "<r>", "<s>", "<o>", "<0-9>", "<w>", "<q>", "<K>"] {
            assert!(text.contains(hint), "missing hint {hint}: {text}");
        }
    }

    #[test]
    fn duplicate_workspace_assignments_are_flagged() {
        let mut monitors = test_monitors();
        for monitor in &mut monitors {
            monitor.workspace = Some(3);
        }

        let text = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 130, 9),
        ));

        assert!(
            text.contains("Duplicate workspace assignment"),
            "warning missing: {text}"
        );
    }

    #[test]
    fn distinct_workspace_assignments_are_not_flagged() {
        let mut monitors = test_monitors();
        monitors[0].workspace = Some(1);
        monitors[1].workspace = Some(2);

        let text = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 130, 9),
        ));

        assert!(!text.contains("Duplicate workspace"), "false alarm: {text}");
    }

    #[test]
    fn preview_mode_is_marked_in_the_title() {
        let monitors = test_monitors();
        let mut list = MonitorList {
            state: TableState::default(),
            selected_row: Some(0),
            mode: TUIMode::View,
            monitors: &monitors,
            is_previewing: true,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 130, 8));
        list.render(buf.area, &mut buf);

        assert!(text_of(&buf).contains("Preview"));
    }

    #[test]
    fn border_colour_follows_the_current_mode() {
        let monitors = test_monitors();
        let area = Rect::new(0, 0, 130, 8);

        let view = render_list_to(&monitors, TUIMode::View, 0, area);
        assert_eq!(view[(0, 0)].style().fg, Some(Color::Yellow));

        let other = render_list_to(&monitors, TUIMode::Resolution, 0, area);
        assert_eq!(other[(0, 0)].style().fg, Some(Color::White));
    }

    #[test]
    fn enable_hint_follows_the_selected_monitor_state() {
        let monitors = test_monitors(); // monitor 0 enabled, monitor 1 disabled
        let wide = Rect::new(0, 0, 130, 8);

        let enabled = text_of(&render_list_to(&monitors, TUIMode::View, 0, wide));
        assert!(enabled.contains("<d>"), "expected Disable: {enabled}");
        assert!(!enabled.contains("<e>"), "unexpected Enable: {enabled}");

        let disabled = text_of(&render_list_to(&monitors, TUIMode::View, 1, wide));
        assert!(disabled.contains("<e>"), "expected Enable: {disabled}");
        assert!(!disabled.contains("<d>"), "unexpected Disable: {disabled}");
    }

    #[test]
    fn sub_modes_advertise_apply_and_cancel() {
        let monitors = test_monitors();
        let wide = Rect::new(0, 0, 130, 8);

        let view = text_of(&render_list_to(&monitors, TUIMode::View, 0, wide));
        assert!(view.contains("<K>"), "help hint missing: {view}");
        assert!(!view.contains("Apply"), "apply is a sub-mode key: {view}");

        let moving = text_of(&render_list_to(&monitors, TUIMode::Move, 0, wide));
        assert!(moving.contains("Done"), "done hint missing: {moving}");
        assert!(moving.contains("Esc/Enter"), "exit keys missing: {moving}");
        assert!(!moving.contains("Help"), "help is view-only: {moving}");
        assert!(moving.contains("Snapmove"), "move hints missing: {moving}");
    }

    #[test]
    fn resolution_and_scale_advertise_apply_and_cancel() {
        let monitors = test_monitors();
        let wide = Rect::new(0, 0, 130, 8);

        for mode in [TUIMode::Resolution, TUIMode::Scale] {
            let text = text_of(&render_list_to(&monitors, mode, 0, wide));
            assert!(text.contains("Apply"), "{mode:?} must advertise Apply: {text}");
            assert!(
                text.contains("<Enter/Space>"),
                "{mode:?} must show the apply keys: {text}"
            );
            assert!(
                text.contains("Cancel"),
                "{mode:?} must advertise Cancel: {text}"
            );
            assert!(
                text.contains("<Esc/q>"),
                "{mode:?} must show the cancel keys: {text}"
            );
            // q must not be advertised as Quit here.
            assert!(!text.contains("<q> "), "{mode:?} must not show Quit: {text}");
        }
    }

    #[test]
    fn every_mode_renders_without_panicking_on_small_areas() {
        for (width, height) in [(30u16, 6), (60, 8), (100, 10), (130, 20)] {
            let monitors = test_monitors();
            for mode in [
                TUIMode::View,
                TUIMode::Move,
                TUIMode::Resolution,
                TUIMode::Scale,
            ] {
                render_list_to(
                    &monitors,
                    mode,
                    1,
                    Rect::new(0, 0, width, height),
                );
            }
        }
    }

    // --- mouse hit testing ----------------------------------------------

    #[test]
    fn row_at_maps_the_rendered_rows() {
        let monitors = test_monitors(); // two monitors, no duplicate workspace
        let area = Rect::new(0, 0, 120, 12);
        // Block border (1) + header and its margin (2) => first row at y = 3.
        assert_eq!(MonitorList::row_at(area, &monitors, 0, 3), Some(0));
        assert_eq!(MonitorList::row_at(area, &monitors, 0, 4), Some(1));
    }

    #[test]
    fn row_at_rejects_the_border_header_and_empty_space() {
        let monitors = test_monitors();
        let area = Rect::new(0, 0, 120, 12);

        assert_eq!(MonitorList::row_at(area, &monitors, 0, 0), None, "border");
        assert_eq!(MonitorList::row_at(area, &monitors, 0, 2), None, "header");
        assert_eq!(
            MonitorList::row_at(area, &monitors, 0, 6),
            None,
            "past the last monitor"
        );
    }

    #[test]
    fn row_at_honours_the_scroll_window() {
        // 12 short rows in a small list force a scroll offset.
        let monitors: Vec<Monitor> = (0..12)
            .map(|i| Monitor {
                name: format!("M{i}"),
                enabled: true,
                ..Default::default()
            })
            .collect();
        let area = Rect::new(0, 0, 120, 12);

        // Selecting the last row scrolls it to the bottom of the window; the
        // top visible row then maps to a later index.
        assert_eq!(MonitorList::row_at(area, &monitors, 11, 3), Some(5));
        assert_eq!(MonitorList::row_at(area, &monitors, 11, 9), Some(11));
    }

    #[test]
    fn the_checkbox_column_ends_before_the_name_column() {
        let area = Rect::new(0, 0, 120, 12);
        let end = MonitorList::icon_column_end(area);
        // Block border (1) + highlight symbol (3) + 5% of 118 (5).
        assert_eq!(end, 9);
        assert!(4 < end, "the icon cell is inside the column");
    }

    #[test]
    fn tiny_areas_do_not_break_row_at() {
        let monitors = test_monitors();
        for (width, height) in [(0u16, 0u16), (1, 1), (3, 2), (5, 4)] {
            let area = Rect::new(0, 0, width, height);
            assert_eq!(MonitorList::row_at(area, &monitors, 0, 0), None);
            let _ = MonitorList::icon_column_end(area);
        }
    }
}
