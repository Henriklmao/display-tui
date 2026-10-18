//! Monitor list component.
//!
//! Displays a list of monitors with their properties.

use crate::monitor::{Monitor, Position};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::App;
use crate::rotation::Rotation;
use crate::ui::layouts::{row_in_window, scrolled_offset};
use crate::utils::TUIMode;
use crate::validation::diagnostics::{CautionFlags, caution_flags};

// Highlight marker shown in front of the selected row.
//
// Its width is part of the table's column layout and therefore of the
// hit-testing geometry, so render and hit-testing share this constant.
pub(crate) const HIGHLIGHT_SYMBOL: &str = " \u{f04b} ";

// Column widths of the display table, shared by rendering and hit-testing so
// the clickable checkbox column can never drift from what is drawn.
const COLUMN_WIDTHS: [Constraint; 8] = [
    Constraint::Percentage(5),
    Constraint::Percentage(15),
    Constraint::Percentage(25),
    Constraint::Percentage(15),
    Constraint::Percentage(10),
    Constraint::Percentage(10),
    Constraint::Percentage(10),
    Constraint::Percentage(10),
];

// Gap between table columns.
const COLUMN_SPACING: u16 = 1;

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

// A footer hint and how soon it is dropped when the line does not fit: lower
// ranks go first, and hints sharing a rank go together.
type Hint = (usize, Vec<Span<'static>>);

// Ranks hints in display order, so the last one is dropped first. The hints are
// listed most important first, which is also how they read.
fn ranked(groups: Vec<Vec<Span<'static>>>) -> Vec<Hint> {
    let last = groups.len().saturating_sub(1);
    groups
        .into_iter()
        .enumerate()
        .map(|(index, spans)| (last - index, spans))
        .collect()
}

// Drops the least important hints until the line fits `inner_width`, so a narrow
// terminal loses whole hints instead of clipping half a key. Everything goes
// before the line is allowed to overflow.
fn fit_footer(hints: &mut Vec<Hint>, inner_width: usize) {
    let mut ranks: Vec<usize> = hints.iter().map(|(rank, _)| *rank).collect();
    ranks.sort_unstable();
    ranks.dedup();

    for rank in ranks {
        if footer_width(hints) <= inner_width {
            return;
        }
        hints.retain(|(hint_rank, _)| *hint_rank != rank);
    }
}

// The drawn width of the footer hints in cells, ignoring the separator space
// after the last hint: it is invisible, so it must not cost a hint at the
// boundary.
fn footer_width(hints: &[Hint]) -> usize {
    let width: usize = hints
        .iter()
        .flat_map(|(_, spans)| spans)
        .map(Span::width)
        .sum();
    let hanging_space = hints
        .last()
        .and_then(|(_, spans)| spans.last())
        .is_some_and(|span| span.content.ends_with(' '));

    width.saturating_sub(usize::from(hanging_space))
}

#[derive(Debug)]
pub struct MonitorList<'a> {
    pub mode: TUIMode,
    pub selected_row: Option<usize>,
    pub state: TableState,
    pub monitors: &'a [Monitor],
    pub is_previewing: bool,
    // Whether a mouse move is waiting to be applied.
    pub moved_pending: bool,
}

impl<'a> MonitorList<'a> {
    pub fn new(monitors: &'a Vec<Monitor>, mode: TUIMode, selected_row: Option<usize>) -> Self {
        MonitorList {
            mode,
            selected_row,
            state: TableState::default().with_selected(selected_row),
            monitors,
            is_previewing: false,
            moved_pending: false,
        }
    }

    // Maps a click's row to a monitor index, honouring the table's scroll
    // window. `y` is the absolute screen row.
    pub fn row_at(area: Rect, monitors: &[Monitor], selected: usize, y: u16) -> Option<usize> {
        let inner = Block::bordered().inner(area);
        let reserved = if has_duplicate_workspace(monitors) {
            2
        } else {
            1
        };
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
    //
    // Computed with the same ratatui layout the table renders with, so the
    // clickable toggle region always matches the drawn checkbox cell.
    pub fn icon_column_end(area: Rect) -> u16 {
        let inner = Block::bordered().inner(area);
        let highlight_width = HIGHLIGHT_SYMBOL.chars().count() as u16;
        let columns_area =
            Layout::horizontal([Constraint::Length(highlight_width), Constraint::Fill(0)])
                .split(Rect::new(0, 0, inner.width, 1))[1];
        let first = Layout::horizontal(COLUMN_WIDTHS)
            .flex(Flex::Start)
            .spacing(COLUMN_SPACING)
            .split(columns_area)[0];
        inner.x.saturating_add(first.right())
    }

    pub fn handle_events(app: &mut App, key_event: KeyEvent) {
        // With no monitors there is nothing to act on.
        if app.monitors.is_empty() {
            return;
        }
        match key_event.code {
            // A pending mouse move is resolved by the confirm gate in
            // `App::handle_key_event` (Enter/Space apply, Esc/q revert) before
            // the View dispatcher runs, so it is not handled here.
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
        {
            let monitor = &mut app.monitors[app.selected_monitor];
            let current_rotation = Rotation::from_transform(&monitor.transform);
            let next_rotation = current_rotation.cycle();
            monitor.transform = Some(next_rotation.to_transform().to_string());
        }
        // A quarter turn swaps the logical width/height, so validate now.
        app.check_layout();
    }

    fn next_monitor(app: &mut App) {
        if app.monitors.is_empty() {
            return;
        }
        app.selected_monitor = if app.selected_monitor >= app.monitors.len() - 1 {
            0
        } else {
            app.selected_monitor + 1
        }
    }

    fn previous_monitor(app: &mut App) {
        if app.monitors.is_empty() {
            return;
        }
        app.selected_monitor = if app.selected_monitor == 0 {
            app.monitors.len() - 1
        } else {
            app.selected_monitor - 1
        }
    }

    fn disable_monitor(app: &mut App) {
        let selected = app.selected_monitor;
        let monitor = &mut app.monitors[selected];
        monitor.enabled = false;
        monitor.saved_position = monitor.position.clone();
        monitor.saved_scale = monitor.scale;
        // A disabled monitor has no Apply button, so drop any pending move. The
        // confirm gate normally resolves the move before `d` is reached; this
        // keeps the invariant if the monitor is disabled some other way.
        app.discard_move(selected);
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
        caution: &CautionFlags,
    ) -> Vec<Row<'static>> {
        self.monitors
            .iter()
            .enumerate()
            .map(|(index, monitor)| {
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
                            // `Cell::default()` carries a zero column span and
                            // would never be painted; build it from the content.
                            Cell::from(Line::from(ws_str.red()))
                        } else {
                            Cell::from(ws_str)
                        }
                    }
                    None => Cell::from("-"),
                };
                let row = Row::new(vec![
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
                ]);

                // A live caution background on any offending monitor, derived
                // from the preview state at render time. The per-cell styles
                // (icon colour, red workspace number) still win because cells
                // are painted over the row style, and the reversed selection
                // patch only swaps fg/bg without clobbering them.
                if caution.flags(index) {
                    row.style(Style::default().bg(Color::Yellow).fg(Color::Black))
                } else {
                    row
                }
            })
            .collect()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(
            format!(
                "{}{}",
                if self.monitors.len() > 1 {
                    " Displays "
                } else {
                    " Display "
                },
                if self.is_previewing { "(Preview)" } else { "" }
            )
            .white()
            .bold(),
        );
        let workspace_counts = workspace_counts(self.monitors);
        let has_duplicate_workspace = has_duplicate_workspace(self.monitors);
        // Derived from the live preview every frame, so every drag pixel, snap,
        // toggle and preset preview updates the caution background instantly.
        let caution = caution_flags(self.monitors);

        let is_mini_mode = area.width < 115;
        // Footer hints as (drop rank, spans), drawn in the order given: when the
        // line does not fit, `fit_footer` drops the lowest ranks until it does,
        // so a narrow terminal loses whole hints instead of half a key.
        let mut hints: Vec<Hint> = Vec::new();
        if self.moved_pending && self.mode == TUIMode::View {
            // A pending mouse move is a confirm state: only Apply or
            // Cancel/Revert are offered, so the usual actions stay hidden until
            // the move is resolved. The help modal is gated while the move is
            // pending, so these hints are the only documentation: the mouse
            // alternative goes first, then the longer verb, and the two keys
            // last and together. "Apply" outlives "Cancel all" so the surviving
            // verb stays with the key it names.
            hints.extend([
                (2, vec![" Apply ".white()]),
                (3, vec!["<Enter/Space> ".blue().bold()]),
                (0, vec![" Right-click ".white()]),
                (1, vec![" Cancel all ".white()]),
                (3, vec!["<Esc/q> ".blue().bold()]),
            ]);
        } else {
            let mut groups: Vec<Vec<Span<'static>>> = Vec::new();
            if is_mini_mode {
                groups.push(vec![" Save ".white(), "<w> ".blue().bold()]);
                if self.mode == TUIMode::View {
                    groups.push(vec![" Quit ".white(), "<q> ".blue().bold()]);
                    groups.push(vec![" Keybinds ".white(), "<K> ".blue().bold()]);
                } else {
                    groups.push(vec![" Cancel ".white(), "<Esc/q> ".blue().bold()]);
                }
            } else {
                // nerdfont symbols:      (nf-oct-arrow_...)
                match self.mode {
                    TUIMode::View => {
                        groups.push(vec![" Scroll  ".white(), "<k/j> ".blue().bold()]);
                        groups.push(vec![" Move ".white(), "<m> ".blue().bold()]);
                        groups.push(vec![" Resolution ".white(), "<r> ".blue().bold()]);
                        groups.push(vec![" Scale ".white(), "<s> ".blue().bold()]);
                        groups.push(vec![" Rotate ".white(), "<o> ".blue().bold()]);
                        groups.push(vec![" Workspace ".white(), "<0-9> ".blue().bold()]);
                        match self.monitors.get(self.selected_row.unwrap_or(0)) {
                            Some(monitor) if monitor.enabled => {
                                groups.push(vec![" Disable ".white(), "<d> ".blue().bold()]);
                            }
                            Some(_) => {
                                groups.push(vec![" Enable ".white(), "<e> ".blue().bold()]);
                            }
                            None => {}
                        }
                    }

                    TUIMode::Resolution | TUIMode::Scale => {
                        groups.push(vec![" Navigate ".white(), "<k/j> ".blue().bold()]);
                        groups.push(vec![" Apply ".white(), "<Enter/Space> ".blue().bold()]);
                    }

                    TUIMode::Move => {
                        groups.push(vec![" Fastmove ".white(), "<H/J/K/L> ".blue().bold()]);
                        groups.push(vec![" Snapmove ".white(), "<h/j/k/l> ".blue().bold()]);
                    }
                }

                groups.push(vec![" Save ".white(), "<w> ".blue().bold()]);
                match self.mode {
                    TUIMode::View => {
                        groups.push(vec![" Quit ".white(), "<q> ".blue().bold()]);
                        groups.push(vec![" Help ".white(), "<K> ".blue().bold()]);
                    }
                    TUIMode::Move => {
                        groups.push(vec![" Done ".white(), "<Esc/Enter/q> ".blue().bold()]);
                    }
                    // In the resolution/scale tables q cancels back to View
                    // instead of quitting, so it is advertised as Cancel.
                    TUIMode::Resolution | TUIMode::Scale => {
                        groups.push(vec![" Cancel ".white(), "<Esc/q> ".blue().bold()]);
                    }
                }
            }
            hints = ranked(groups);
        }

        fit_footer(&mut hints, area.width.saturating_sub(2) as usize);

        let instructions = Line::from(
            hints
                .into_iter()
                .flat_map(|(_, spans)| spans)
                .collect::<Vec<Span<'static>>>(),
        );

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

        let widths = COLUMN_WIDTHS;

        let table = Table::new(self.monitors_to_rows(&workspace_counts, &caution), widths)
            .column_spacing(COLUMN_SPACING)
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
            .row_highlight_style(Style::new().reversed().bold())
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
    use crate::test_utils::tests::{find_cell, find_text, test_monitors, text_of};
    use ratatui::style::Style;

    // The instruction footer of a rendered list, without the border column and
    // the centring spaces (so it is the drawn hint text, not the padding).
    fn footer(width: u16, mode: TUIMode, pending: bool) -> String {
        let monitors = test_monitors();
        let area = Rect::new(0, 0, width, 8);
        let mut list = MonitorList {
            state: TableState::default().with_selected(Some(0)),
            selected_row: Some(0),
            mode,
            monitors: &monitors,
            is_previewing: false,
            moved_pending: pending,
        };
        let mut buf = Buffer::empty(area);
        list.render(area, &mut buf);

        // The footer is the last inner row, above the bottom border.
        let row = area.height - 2;
        (0..width)
            .map(|x| buf[(x, row)].symbol())
            .collect::<String>()
            .trim_matches(|symbol| symbol == ' ' || symbol == '┃')
            .to_string()
    }

    #[test]
    fn render_list() {
        let mut list = MonitorList {
            state: TableState::default(),
            selected_row: Some(0),
            mode: TUIMode::View,
            monitors: &test_monitors(),
            is_previewing: false,
            moved_pending: false,
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
            // Match MonitorList::new: a selection makes the table reserve the
            // highlight column, exactly like production.
            state: TableState::default().with_selected(Some(selected)),
            selected_row: Some(selected),
            mode,
            monitors,
            is_previewing: false,
            moved_pending: false,
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
            moved_pending: false,
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
            assert!(
                text.contains("Apply"),
                "{mode:?} must advertise Apply: {text}"
            );
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
            assert!(
                !text.contains("<q> "),
                "{mode:?} must not show Quit: {text}"
            );
        }
    }

    #[test]
    fn a_pending_move_advertises_apply_in_view_mode() {
        let monitors = test_monitors();
        let wide = Rect::new(0, 0, 130, 8);

        let plain = text_of(&render_list_to(&monitors, TUIMode::View, 0, wide));
        assert!(
            !plain.contains("Apply"),
            "no apply hint without a move: {plain}"
        );

        let mut list = MonitorList {
            state: TableState::default(),
            selected_row: Some(0),
            mode: TUIMode::View,
            monitors: &monitors,
            is_previewing: false,
            moved_pending: true,
        };
        let mut buf = Buffer::empty(wide);
        list.render(wide, &mut buf);
        let text = text_of(&buf);
        assert!(text.contains("Apply"), "apply hint missing: {text}");
        assert!(text.contains("<Enter/Space>"), "apply keys missing: {text}");

        // In a sub-mode the pending hint is suppressed (Enter/Space apply the
        // highlighted value there), so "Apply" must appear exactly once.
        let mut sub = MonitorList {
            state: TableState::default().with_selected(Some(0)),
            selected_row: Some(0),
            mode: TUIMode::Resolution,
            monitors: &monitors,
            is_previewing: false,
            moved_pending: true,
        };
        let mut buf = Buffer::empty(wide);
        sub.render(wide, &mut buf);
        let text = text_of(&buf);
        assert_eq!(
            text.matches("Apply").count(),
            1,
            "no duplicate Apply hint in a sub-mode: {text}"
        );
    }

    #[test]
    fn a_pending_move_offers_only_apply_and_cancel() {
        let monitors = test_monitors();
        // Both the wide and the compact instruction line collapse to the confirm
        // hints, so the check runs across the mini-mode threshold too.
        for area in [Rect::new(0, 0, 130, 8), Rect::new(0, 0, 100, 8)] {
            let mut list = MonitorList {
                state: TableState::default().with_selected(Some(0)),
                selected_row: Some(0),
                mode: TUIMode::View,
                monitors: &monitors,
                is_previewing: false,
                moved_pending: true,
            };
            let mut buf = Buffer::empty(area);
            list.render(area, &mut buf);
            let text = text_of(&buf);

            assert!(text.contains("Apply"), "apply hint missing: {text}");
            assert!(text.contains("Cancel all"), "revert hint missing: {text}");
            assert!(text.contains("<Esc/q>"), "revert keys missing: {text}");
            for hidden in ["<m>", "<r>", "<s>", "<o>", "<0-9>", "<w>", "<q>", "<K>"] {
                assert!(!text.contains(hidden), "{hidden} must be hidden: {text}");
            }
        }
    }

    #[test]
    fn the_footer_drops_whole_hints_instead_of_clipping_them() {
        // The footer is the only place several hints appear, and ratatui clips a
        // centred line silently, so assert on the drawn cells: no word and no key
        // may be cut in half, whatever the width.
        const KEYS: [&str; 16] = [
            "<k/j>",
            "<m>",
            "<r>",
            "<s>",
            "<o>",
            "<0-9>",
            "<d>",
            "<e>",
            "<w>",
            "<q>",
            "<K>",
            "<Enter/Space>",
            "<Esc/q>",
            "<H/J/K/L>",
            "<h/j/k/l>",
            "<Esc/Enter/q>",
        ];
        const WORDS: [&str; 21] = [
            "Apply",
            "Cancel",
            "all",
            "Right-click",
            "Save",
            "Quit",
            "Keybinds",
            "Scroll",
            "Move",
            "Resolution",
            "Scale",
            "Rotate",
            "Workspace",
            "Disable",
            "Enable",
            "Navigate",
            "Fastmove",
            "Snapmove",
            "Done",
            "Help",
            // The scroll arrows of the View hint.
            "\u{f431}\u{f433}",
        ];

        for mode in [
            TUIMode::View,
            TUIMode::Move,
            TUIMode::Resolution,
            TUIMode::Scale,
        ] {
            for width in [16u16, 24, 30, 40, 46, 60, 90, 114, 115, 120, 130, 160] {
                for pending in [false, true] {
                    let line = footer(width, mode, pending);
                    let cut: Vec<&str> = line
                        .split_whitespace()
                        .filter(|token| !KEYS.contains(token) && !WORDS.contains(token))
                        .collect();

                    assert!(
                        cut.is_empty(),
                        "{mode:?} at {width} clipped {cut:?}: {line}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_pending_move_footer_keeps_both_keys_or_neither() {
        // The help modal is gated while a move is pending, so the footer is the
        // only documentation: it must not offer Apply without its Cancel.
        for width in 8u16..170 {
            let line = footer(width, TUIMode::View, true);
            assert_eq!(
                line.contains("<Enter/Space>"),
                line.contains("<Esc/q>"),
                "at {width}: {line}"
            );
        }

        // At 46 columns - the width that once clipped it - both survive.
        let line = footer(46, TUIMode::View, true);
        assert!(
            line.contains("<Enter/Space>") && line.contains("<Esc/q>"),
            "{line}"
        );
    }

    #[test]
    fn the_right_click_hint_is_only_shown_when_the_confirm_line_fits() {
        // Wide enough: the mouse alternative is advertised next to the keys.
        let wide = footer(130, TUIMode::View, true);
        assert!(wide.contains("Right-click"), "right-click hint: {wide}");

        // Too narrow for the longer line: the hint is dropped instead of
        // clipping the keys (covered by the overflow test above).
        let narrow = footer(46, TUIMode::View, true);
        assert!(
            !narrow.contains("Right-click"),
            "hint must be dropped: {narrow}"
        );
    }

    #[test]
    fn a_pending_move_does_not_change_the_sub_mode_footer() {
        // In a sub-mode Enter/Space apply the highlighted row, so the reduced
        // View hint must not leak in there.
        let monitors = test_monitors();
        let area = Rect::new(0, 0, 130, 8);
        let mut list = MonitorList {
            state: TableState::default().with_selected(Some(0)),
            selected_row: Some(0),
            mode: TUIMode::Resolution,
            monitors: &monitors,
            is_previewing: false,
            moved_pending: true,
        };
        let mut buf = Buffer::empty(area);
        list.render(area, &mut buf);
        let text = text_of(&buf);

        assert!(text.contains("<k/j>"), "navigation hint missing: {text}");
        assert!(text.contains("<Enter/Space>"), "row apply missing: {text}");
        assert!(
            !text.contains("Cancel all"),
            "the View confirm hint must stay in View: {text}"
        );
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
                render_list_to(&monitors, mode, 1, Rect::new(0, 0, width, height));
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
    fn the_checkbox_column_matches_the_drawn_icon_across_widths() {
        // The toggle region must contain the drawn checkbox cell and stop
        // before the name column, at every width (not just the one geometry
        // the old formula happened to agree on).
        let monitors = vec![Monitor {
            name: "X".to_string(),
            enabled: true,
            ..Default::default()
        }];
        for width in [40u16, 60, 80, 100, 120, 140, 200] {
            let area = Rect::new(0, 0, width, 12);
            let buf = render_list_to(&monitors, TUIMode::View, 0, area);
            let (icon_x, _) = find_cell(&buf, "\u{f00c}").expect("enabled icon");
            let end = MonitorList::icon_column_end(area);

            assert!(
                icon_x < end,
                "width {width}: icon at {icon_x} must be inside the toggle region (< {end})"
            );
            let (name_x, _) = find_text(&buf, "X").expect("name column");
            assert!(
                name_x >= end,
                "width {width}: name at {name_x} must be outside the toggle region (< {end})"
            );
        }
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

    #[test]
    fn key_events_with_no_monitors_are_safe() {
        let mut a = App::new(None);
        assert!(a.monitors.is_empty());

        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Char('e'),
            KeyCode::Char('d'),
            KeyCode::Char('o'),
            KeyCode::Char('1'),
        ] {
            MonitorList::handle_events(
                &mut a,
                KeyEvent::new(code, crossterm::event::KeyModifiers::NONE),
            );
        }

        assert_eq!(a.selected_monitor, 0);
        assert_eq!(a.mode, TUIMode::View, "sub-modes need a monitor");
    }

    // --- live caution background ----------------------------------------

    use crate::monitor::{Position, Resolution};

    // An enabled 1920x1080 monitor at (x, 0) with the given name.
    fn placed(name: &str, x: i32) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
            position: Some(Position { x, y: 0 }),
            scale: Some(1.0),
            modes: vec![Resolution {
                width: 1920,
                height: 1080,
                refresh: 60.0,
                preferred: true,
                current: true,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn an_overlapping_row_carries_a_yellow_caution_background() {
        // B pokes into A: both are offending partners and both are flagged.
        let monitors = vec![placed("Alpha", 0), placed("Beta", 100)];
        let area = Rect::new(0, 0, 130, 9);
        let buf = render_list_to(&monitors, TUIMode::View, 2, area);

        for name in ["Alpha", "Beta"] {
            let (x, y) = find_text(&buf, name).expect("monitor row");
            assert_eq!(
                buf[(x, y)].style().bg,
                Some(Color::Yellow),
                "{name} must carry the caution background"
            );
            assert_eq!(buf[(x, y)].style().fg, Some(Color::Black));
        }
    }

    #[test]
    fn a_clean_row_has_no_caution_background() {
        let monitors = vec![placed("Alpha", 0), placed("Beta", 1920)];
        let area = Rect::new(0, 0, 130, 9);
        let buf = render_list_to(&monitors, TUIMode::View, 0, area);

        let (x, y) = find_text(&buf, "Alpha").expect("monitor row");
        assert_ne!(
            buf[(x, y)].style().bg,
            Some(Color::Yellow),
            "a valid layout must not be cautioned"
        );
    }

    #[test]
    fn a_detached_row_is_cautioned_but_its_anchor_is_not() {
        let monitors = vec![placed("Alpha", 0), placed("Far", 50_000)];
        let area = Rect::new(0, 0, 130, 10);
        let buf = render_list_to(&monitors, TUIMode::View, 0, area);

        let (fx, fy) = find_text(&buf, "Far").expect("detached row");
        assert_eq!(buf[(fx, fy)].style().bg, Some(Color::Yellow));

        let (ax, ay) = find_text(&buf, "Alpha").expect("anchor row");
        assert_ne!(
            buf[(ax, ay)].style().bg,
            Some(Color::Yellow),
            "only the offender is flagged, never the anchor"
        );
    }

    // Regression: ratatui patches the highlight style over the whole selected
    // row after the cells are painted, so a fg-only highlight used to clobber
    // the red workspace number and hide it behind yellow-on-yellow.
    #[test]
    fn the_red_workspace_number_survives_on_selected_and_unselected_rows() {
        let monitors = vec![
            {
                let mut m = placed("Alpha", 0);
                m.workspace = Some(3);
                m
            },
            {
                let mut m = placed("Beta", 1920);
                m.workspace = Some(3);
                m
            },
        ];
        let area = Rect::new(0, 0, 130, 10);

        for selected in [0, 1] {
            let buf = render_list_to(&monitors, TUIMode::View, selected, area);
            let (x, y) = find_text(&buf, "3").expect("workspace cell");
            assert_eq!(
                buf[(x, y)].style().fg,
                Some(Color::Red),
                "duplicate workspace must stay red on row {selected}"
            );
        }
    }

    #[test]
    fn a_selected_caution_row_keeps_readable_contrast() {
        let monitors = vec![placed("Alpha", 0), placed("Beta", 100)];
        let area = Rect::new(0, 0, 130, 9);
        let buf = render_list_to(&monitors, TUIMode::View, 0, area);
        let (_, row) = find_text(&buf, "Alpha").expect("monitor row");
        let inner = Block::bordered().inner(area);

        for x in inner.left()..inner.right() {
            let style = buf[(x, row)].style();
            if let (Some(fg), Some(bg)) = (style.fg, style.bg) {
                assert_ne!(
                    fg, bg,
                    "cell ({x}, {row}) would be invisible when reversed"
                );
            }
        }
    }

    #[test]
    fn moving_a_monitor_clear_un_flags_its_row_without_any_apply() {
        let monitors = vec![placed("Alpha", 0), placed("Beta", 100)];
        let area = Rect::new(0, 0, 130, 9);

        let flagged = render_list_to(&monitors, TUIMode::View, 2, area);
        let (x, y) = find_text(&flagged, "Beta").expect("Beta row");
        assert_eq!(flagged[(x, y)].style().bg, Some(Color::Yellow));

        // Move Beta clear of Alpha: re-rendering alone must clear the caution,
        // with no popup, apply or write in between.
        let mut cleared = monitors;
        cleared[1].position = Some(Position { x: 1920, y: 0 });
        let buf = render_list_to(&cleared, TUIMode::View, 2, area);
        let (x, y) = find_text(&buf, "Beta").expect("Beta row");
        assert_ne!(
            buf[(x, y)].style().bg,
            Some(Color::Yellow),
            "the caution background must disappear live"
        );
    }
}
