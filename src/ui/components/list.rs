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
    text::Line,
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::App;
use crate::app::state::HoverTarget;
use crate::rotation::Rotation;
use crate::ui::components::action_bar::{ActionBar, ActionBarContext};
use crate::ui::layouts::{row_in_window, scrolled_offset};
use crate::ui::styles;
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

// The columns shown at a given terminal width: trailing columns are dropped
// instead of being squeezed into unreadable slivers. The checkbox and name
// columns always survive, so hit-testing stays meaningful.
fn visible_columns(width: u16) -> Vec<Constraint> {
    let count = if width < 60 {
        2 // icon, name
    } else if width < 90 {
        4 // icon, name, description, resolution
    } else if width < 110 {
        6 // ... position, scale
    } else {
        COLUMN_WIDTHS.len()
    };
    COLUMN_WIDTHS[..count].to_vec()
}

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

// Whether any workspace is assigned to more than one monitor. This stays on
// the stack because the footer hit-test calls it on every mouse move.
fn has_duplicate_workspace(monitors: &[Monitor]) -> bool {
    let mut seen = [false; 256];
    for monitor in monitors {
        if let Some(ws) = monitor.workspace {
            if seen[ws as usize] {
                return true;
            }
            seen[ws as usize] = true;
        }
    }
    false
}

// The vertical split of the list's inner area: table, optional duplicate
// warning, footer. Rendering and hit-testing both use this.
fn list_chunks(inner: Rect, monitors: &[Monitor]) -> [Rect; 3] {
    Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(if has_duplicate_workspace(monitors) {
                1
            } else {
                0
            }),
            Constraint::Length(1),
        ])
        .areas::<3>(inner)
}

// The visible data-row window: first screen row, viewport height and scroll
// offset. Shared by row and cell hit-testing.
fn row_window(area: Rect, monitors: &[Monitor], selected: usize) -> (u16, u16, usize) {
    let inner = Block::bordered().inner(area);
    let warning = u16::from(has_duplicate_workspace(monitors));
    let footer = u16::from(inner.height > warning);
    let table_height = inner.height.saturating_sub(warning).saturating_sub(footer);
    // The header row plus its bottom margin occupy two lines.
    let viewport = table_height.saturating_sub(2);
    let offset = scrolled_offset(selected, viewport as usize);
    (inner.y.saturating_add(2), viewport, offset)
}

// The columns of the display table, in draw order. Shared by rendering and
// hit-testing so a click on a cell can never drift from the drawn column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListColumn {
    Icon,
    Name,
    Description,
    Resolution,
    Position,
    Scale,
    Rotation,
    Workspace,
}

fn column_for_index(index: usize) -> Option<ListColumn> {
    match index {
        0 => Some(ListColumn::Icon),
        1 => Some(ListColumn::Name),
        2 => Some(ListColumn::Description),
        3 => Some(ListColumn::Resolution),
        4 => Some(ListColumn::Position),
        5 => Some(ListColumn::Scale),
        6 => Some(ListColumn::Rotation),
        7 => Some(ListColumn::Workspace),
        _ => None,
    }
}

// Whether the pointer rests on one specific action cell.
fn cell_hover(hover: Option<HoverTarget>, index: usize, column: ListColumn) -> bool {
    match (hover, column) {
        (Some(HoverTarget::ListIcon(row)), ListColumn::Icon) => row == index,
        (Some(HoverTarget::ListResolution(row)), ListColumn::Resolution) => row == index,
        (Some(HoverTarget::ListScale(row)), ListColumn::Scale) => row == index,
        (Some(HoverTarget::ListRotation(row)), ListColumn::Rotation) => row == index,
        (Some(HoverTarget::ListWorkspace(row)), ListColumn::Workspace) => row == index,
        _ => false,
    }
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
    // The element currently under the pointer, for the hover glow.
    pub hover: Option<HoverTarget>,
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
            hover: None,
        }
    }

    // Maps a click's row to a monitor index, honouring the table's scroll
    // window. `y` is the absolute screen row.
    pub fn row_at(area: Rect, monitors: &[Monitor], selected: usize, y: u16) -> Option<usize> {
        let (top, viewport, offset) = row_window(area, monitors, selected);
        row_in_window(top, viewport, offset, monitors.len(), y)
    }

    // The drawn columns as (column, screen rect) pairs. Shared by rendering and
    // hit-testing so a click on a cell can never drift from the drawn column.
    fn column_rects(area: Rect) -> Vec<(ListColumn, Rect)> {
        let inner = Block::bordered().inner(area);
        if inner.width == 0 {
            return Vec::new();
        }
        let highlight_width = HIGHLIGHT_SYMBOL.chars().count() as u16;
        let columns_area =
            Layout::horizontal([Constraint::Length(highlight_width), Constraint::Fill(0)])
                .split(Rect::new(0, 0, inner.width, 1))[1];
        let rects = Layout::horizontal(visible_columns(area.width))
            .flex(Flex::Start)
            .spacing(COLUMN_SPACING)
            .split(columns_area);

        rects
            .iter()
            .enumerate()
            .filter_map(|(index, rect)| {
                column_for_index(index).map(|column| {
                    (
                        column,
                        Rect::new(inner.x.saturating_add(rect.x), 0, rect.width, 1),
                    )
                })
            })
            .collect()
    }

    // The action-bar row the renderer actually reserves. `None` when the list
    // is too short, so the mouse cannot aim at a row that was not drawn.
    pub fn footer_rect(area: Rect, monitors: &[Monitor]) -> Option<Rect> {
        // Arithmetic fast path: this runs on every mouse move, so it must not
        // build a ratatui Layout (whose constraint list allocates).
        let inner = Block::bordered().inner(area);
        if inner.height == 0 {
            return None;
        }
        let warning = u16::from(has_duplicate_workspace(monitors));
        if inner.height <= warning {
            return None;
        }
        Some(Rect {
            x: inner.x,
            y: inner.bottom() - 1,
            width: inner.width,
            height: 1,
        })
    }

    // The table column under a screen column, if any.
    pub fn column_at(area: Rect, x: u16) -> Option<ListColumn> {
        MonitorList::column_rects(area)
            .into_iter()
            .find(|(_, rect)| x >= rect.x && x < rect.right())
            .map(|(column, _)| column)
    }

    // The screen rectangle of one column, with `y` left at zero. Callers that
    // need a cell set the row themselves.
    pub fn column_rect(area: Rect, column: ListColumn) -> Option<Rect> {
        MonitorList::column_rects(area)
            .into_iter()
            .find(|(candidate, _)| *candidate == column)
            .map(|(_, rect)| rect)
    }

    // The screen rectangle of one data cell, honouring the table's scroll
    // window. Returns None when that monitor row is not drawn.
    pub fn cell_rect(
        area: Rect,
        monitors: &[Monitor],
        selected: usize,
        row: usize,
        column: ListColumn,
    ) -> Option<Rect> {
        let (top, viewport, offset) = row_window(area, monitors, selected);
        if row >= monitors.len() || row < offset || row >= offset.saturating_add(viewport as usize)
        {
            return None;
        }

        let y = top.saturating_add((row - offset) as u16);
        let column = MonitorList::column_rect(area, column)?;
        Some(Rect {
            x: column.x,
            y,
            width: column.width,
            height: 1,
        })
    }

    // Exclusive right edge (screen column) of the leading checkbox column.
    // Kept as a test helper; production hit-testing uses `column_at`.
    #[cfg(test)]
    pub fn icon_column_end(area: Rect) -> u16 {
        MonitorList::column_rect(area, ListColumn::Icon)
            .map(|rect| rect.right())
            .unwrap_or(area.x)
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

    pub(crate) fn set_workspace(app: &mut App, workspace: u8) {
        if let Some(monitor) = app.monitors.get_mut(app.selected_monitor) {
            if workspace == 0 {
                monitor.workspace = None;
            } else {
                monitor.workspace = Some(workspace);
            }
        }
    }
    pub(crate) fn cycle_rotation(app: &mut App) {
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
                let mut workspace_cell = match monitor.workspace {
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
                if cell_hover(self.hover, index, ListColumn::Workspace) {
                    workspace_cell = workspace_cell.style(styles::hover_style());
                }

                let mut icon_cell = Cell::from(if enabled == "true" {
                    "".to_string()
                } else {
                    "".to_string()
                })
                .style(Style::default().fg(if enabled == "true" {
                    Color::Green
                } else {
                    Color::Red
                }));
                if cell_hover(self.hover, index, ListColumn::Icon) {
                    icon_cell = icon_cell.style(styles::hover_style());
                }

                let mut resolution_cell = Cell::from(resolution);
                if cell_hover(self.hover, index, ListColumn::Resolution) {
                    resolution_cell = resolution_cell.style(styles::hover_style());
                }

                let mut scale_cell = Cell::from(scale);
                if cell_hover(self.hover, index, ListColumn::Scale) {
                    scale_cell = scale_cell.style(styles::hover_style());
                }

                let mut rotation_cell = Cell::from(rotation);
                if cell_hover(self.hover, index, ListColumn::Rotation) {
                    rotation_cell = rotation_cell.style(styles::hover_style());
                }

                let row = Row::new(vec![
                    icon_cell,
                    Cell::from(name),
                    Cell::from(description),
                    resolution_cell,
                    Cell::from(position),
                    scale_cell,
                    rotation_cell,
                    workspace_cell,
                ]);

                // A live caution background on any offending monitor, derived
                // from the preview state at render time. The per-cell styles
                // (icon colour, red workspace number) still win because cells
                // are painted over the row style.
                //
                // The selected row is deliberately exempt: ratatui patches the
                // selection style (reversed) over the whole row afterwards, so
                // a caution background would turn into a black block and make a
                // warned row look different from a clean selected row. The red
                // workspace number still marks the problem on the selected row.
                let selected = self.state.selected() == Some(index);
                let mut style = Style::default();
                if caution.flags(index) && !selected {
                    style = style.bg(Color::Yellow).fg(Color::Black);
                }
                if self.hover == Some(HoverTarget::ListRow(index)) {
                    style = style.fg(styles::hover_accent()).bold();
                }
                if style == Style::default() {
                    row
                } else {
                    row.style(style)
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

        let action_context = ActionBarContext {
            mode: self.mode,
            pending: self.moved_pending,
            has_monitors: !self.monitors.is_empty(),
            selected_enabled: self
                .selected_row
                .and_then(|index| self.monitors.get(index))
                .is_some_and(|monitor| monitor.enabled),
        };

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

        let chunks = list_chunks(inner_area, self.monitors);

        let widths = visible_columns(area.width);

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
        ActionBar::render_in(chunks[2], buf, action_context, self.hover, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{find_cell, find_text, test_monitors, text_of};

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
            hover: None,
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
            hover: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 7));

        list.render(buf.area, &mut buf);
        let text = text_of(&buf);

        assert!(text.contains("Displays"), "title missing: {text}");
        assert!(text.contains("Monitor 1"), "first row missing: {text}");
        assert!(text.contains("Monitor 2"), "second row missing: {text}");
        assert!(text.contains("[ Save  w ]"), "footer missing: {text}");
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
            hover: None,
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
    fn wide_terminals_get_the_full_button_line() {
        let monitors = test_monitors();
        let text = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 160, 8),
        ));

        for button in [
            "[ Move  m ]",
            "[ Resolution  r ]",
            "[ Scale  s ]",
            "[ Rotate  o ]",
            "Workspace <0-9>",
            "[ Disable  d ]",
            "[ Presets  p ]",
            "[ Save  w ]",
            "[ Quit  q ]",
            "[ Help  K ]",
        ] {
            assert!(text.contains(button), "missing button {button}: {text}");
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
            hover: None,
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
        assert!(
            enabled.contains("[ Disable  d ]"),
            "expected Disable: {enabled}"
        );
        assert!(
            !enabled.contains("[ Enable  e ]"),
            "unexpected Enable: {enabled}"
        );

        let disabled = text_of(&render_list_to(&monitors, TUIMode::View, 1, wide));
        assert!(
            disabled.contains("[ Enable  e ]"),
            "expected Enable: {disabled}"
        );
        assert!(
            !disabled.contains("[ Disable  d ]"),
            "unexpected Disable: {disabled}"
        );
    }

    #[test]
    fn sub_modes_advertise_apply_and_cancel() {
        let monitors = test_monitors();
        let wide = Rect::new(0, 0, 130, 8);

        let view = text_of(&render_list_to(&monitors, TUIMode::View, 0, wide));
        assert!(view.contains("[ Help  K ]"), "help button missing: {view}");
        assert!(!view.contains("Apply"), "apply is a sub-mode key: {view}");

        let moving = text_of(&render_list_to(&monitors, TUIMode::Move, 0, wide));
        assert!(
            moving.contains("[ Done  Esc ]"),
            "done button missing: {moving}"
        );
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
                text.contains("[ Apply  Enter ]"),
                "{mode:?} must show the apply button: {text}"
            );
            assert!(
                text.contains("[ Cancel  Esc ]"),
                "{mode:?} must show the cancel button: {text}"
            );
            // q must not be advertised as Quit here.
            assert!(
                !text.contains("[ Quit  q ]"),
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
            hover: None,
        };
        let mut buf = Buffer::empty(wide);
        list.render(wide, &mut buf);
        let text = text_of(&buf);
        assert!(text.contains("Apply"), "apply hint missing: {text}");
        assert!(
            text.contains("[ Apply  Enter ]"),
            "apply button missing: {text}"
        );

        // In a sub-mode the pending hint is suppressed (Enter/Space apply the
        // highlighted value there), so "Apply" must appear exactly once.
        let mut sub = MonitorList {
            state: TableState::default().with_selected(Some(0)),
            selected_row: Some(0),
            mode: TUIMode::Resolution,
            monitors: &monitors,
            is_previewing: false,
            moved_pending: true,
            hover: None,
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
                hover: None,
            };
            let mut buf = Buffer::empty(area);
            list.render(area, &mut buf);
            let text = text_of(&buf);

            assert!(
                text.contains("[ Apply  Enter ]"),
                "apply button missing: {text}"
            );
            assert!(
                text.contains("[ Cancel all  Esc ]"),
                "revert button missing: {text}"
            );
            for hidden in [
                "[ Move  m ]",
                "[ Resolution  r ]",
                "[ Scale  s ]",
                "[ Rotate  o ]",
                "Workspace <0-9>",
                "[ Save  w ]",
                "[ Quit  q ]",
                "[ Help  K ]",
            ] {
                assert!(!text.contains(hidden), "{hidden} must be hidden: {text}");
            }
        }
    }

    #[test]
    fn the_footer_drops_whole_buttons_instead_of_clipping_them() {
        // A clipped chip would leave an unmatched bracket. Every drawn '[' must
        // have a closing ']' after it, whatever the width and mode.
        for mode in [
            TUIMode::View,
            TUIMode::Move,
            TUIMode::Resolution,
            TUIMode::Scale,
        ] {
            for width in [16u16, 24, 30, 40, 46, 60, 90, 114, 115, 120, 130, 160] {
                for pending in [false, true] {
                    let line = footer(width, mode, pending);
                    assert_eq!(
                        line.matches('[').count(),
                        line.matches(']').count(),
                        "{mode:?} at {width} left a partial chip: {line}"
                    );
                    for (index, _) in line.match_indices('[') {
                        let rest = &line[index + 1..];
                        let end = rest.find(']').unwrap_or_else(|| {
                            panic!("{mode:?} at {width} has an unclosed chip: {line}")
                        });
                        assert!(
                            !rest[..end].contains('['),
                            "{mode:?} at {width} nested a chip: {line}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_pending_move_footer_keeps_both_buttons_or_neither() {
        // The help modal is gated while a move is pending, so the footer is the
        // only documentation: it must not offer Apply without its Cancel.
        for width in 8u16..170 {
            let line = footer(width, TUIMode::View, true);
            let has_apply = line.contains("Apply") || line.contains("[\u{21b5}]");
            let has_cancel = line.contains("Cancel") || line.contains("[Esc]");
            assert_eq!(has_apply, has_cancel, "at {width}: {line}");
        }

        // At 46 columns both full buttons still fit.
        let line = footer(46, TUIMode::View, true);
        assert!(
            line.contains("[ Apply  Enter ]") && line.contains("[ Cancel all  Esc ]"),
            "{line}"
        );
    }

    #[test]
    fn the_pending_footer_leaves_right_click_to_the_tooltip() {
        // The Apply button's tooltip mentions the right-click alternative; the
        // limited footer width belongs to the two real buttons.
        for width in [46u16, 130] {
            let line = footer(width, TUIMode::View, true);
            assert!(
                !line.contains("Right-click"),
                "the footer must not spend width on the mouse hint: {line}"
            );
        }
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
            hover: None,
        };
        let mut buf = Buffer::empty(area);
        list.render(area, &mut buf);
        let text = text_of(&buf);

        assert!(text.contains("<k/j>"), "navigation hint missing: {text}");
        assert!(
            text.contains("[ Apply  Enter ]"),
            "row apply missing: {text}"
        );
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
    fn a_too_short_list_reserves_no_footer_row() {
        let mut monitors = test_monitors();
        for monitor in &mut monitors {
            monitor.workspace = Some(3); // forces the duplicate-warning row
        }
        assert!(
            MonitorList::footer_rect(Rect::new(0, 0, 120, 3), &monitors).is_none(),
            "the warning line wins when there is only one inner row"
        );
        assert!(
            MonitorList::footer_rect(Rect::new(0, 0, 120, 8), &monitors).is_some(),
            "a normal list reserves its footer"
        );
    }

    #[test]
    fn the_fast_footer_rect_matches_the_layout_split() {
        for height in 0..16 {
            for duplicate in [false, true] {
                let mut monitors = test_monitors();
                if duplicate {
                    for monitor in &mut monitors {
                        monitor.workspace = Some(3);
                    }
                }
                let area = Rect::new(0, 0, 120, height);
                let inner = Block::bordered().inner(area);
                let expected = if inner.height == 0 {
                    None
                } else {
                    let footer = list_chunks(inner, &monitors)[2];
                    (footer.height > 0).then_some(footer)
                };

                assert_eq!(
                    MonitorList::footer_rect(area, &monitors),
                    expected,
                    "height {height}, duplicate {duplicate}"
                );
            }
        }
    }

    #[test]
    fn the_fast_row_window_matches_the_layout_split() {
        for height in 0..20 {
            for duplicate in [false, true] {
                let mut monitors = test_monitors();
                if duplicate {
                    for monitor in &mut monitors {
                        monitor.workspace = Some(3);
                    }
                }
                let area = Rect::new(0, 0, 120, height);
                let inner = Block::bordered().inner(area);
                let table = list_chunks(inner, &monitors)[0];
                let viewport = table.height.saturating_sub(2);
                let offset = scrolled_offset(0, viewport as usize);

                assert_eq!(
                    row_window(area, &monitors, 0),
                    (table.y.saturating_add(2), viewport, offset),
                    "height {height}, duplicate {duplicate}"
                );
            }
        }
    }

    #[test]
    fn column_at_maps_every_drawn_column() {
        let area = Rect::new(0, 0, 160, 12);

        for column in [
            ListColumn::Icon,
            ListColumn::Name,
            ListColumn::Description,
            ListColumn::Resolution,
            ListColumn::Position,
            ListColumn::Scale,
            ListColumn::Rotation,
            ListColumn::Workspace,
        ] {
            let rect = MonitorList::column_rect(area, column).expect("visible column");
            assert_eq!(MonitorList::column_at(area, rect.x), Some(column));
            assert_eq!(
                MonitorList::column_at(area, rect.right() - 1),
                Some(column),
                "the last cell of {column:?}"
            );
        }
    }

    #[test]
    fn columns_disappear_on_narrow_terminals() {
        let area = Rect::new(0, 0, 80, 12);

        assert!(MonitorList::column_rect(area, ListColumn::Resolution).is_some());
        assert!(
            MonitorList::column_rect(area, ListColumn::Scale).is_none(),
            "scale is dropped before the resolution column"
        );
        assert!(
            MonitorList::column_rect(area, ListColumn::Workspace).is_none(),
            "workspace is dropped on a narrow terminal"
        );
    }

    #[test]
    fn cell_rect_maps_the_drawn_row_and_column() {
        let monitors = test_monitors();
        let area = Rect::new(0, 0, 160, 12);

        let first = MonitorList::cell_rect(area, &monitors, 0, 0, ListColumn::Workspace)
            .expect("first row");
        let second = MonitorList::cell_rect(area, &monitors, 0, 1, ListColumn::Workspace)
            .expect("second row");
        let column =
            MonitorList::column_rect(area, ListColumn::Workspace).expect("workspace column");

        assert_eq!(first.y, 3);
        assert_eq!(second.y, 4);
        assert_eq!(first.x, column.x);
        assert_eq!(first.width, column.width);
        assert_eq!(
            MonitorList::cell_rect(area, &monitors, 0, 2, ListColumn::Workspace),
            None,
            "past the last monitor"
        );
    }

    #[test]
    fn a_hovered_action_cell_glows() {
        let monitors = test_monitors();
        let area = Rect::new(0, 0, 160, 8);
        let mut list = MonitorList {
            state: TableState::default(),
            selected_row: Some(0),
            mode: TUIMode::View,
            monitors: &monitors,
            is_previewing: false,
            moved_pending: false,
            hover: Some(HoverTarget::ListResolution(0)),
        };
        let mut buf = Buffer::empty(area);
        list.render(area, &mut buf);

        let (x, y) = find_text(&buf, "1920x1080").expect("resolution cell");
        assert_eq!(
            buf[(x, y)].style().fg,
            Some(styles::hover_accent()),
            "the hovered cell must use the accent colour"
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
                // Reset/Reset is the terminal's own default pair, which is
                // always contrasting; only explicit colours can collide.
                if fg == Color::Reset && bg == Color::Reset {
                    continue;
                }
                assert_ne!(fg, bg, "cell ({x}, {row}) would be invisible when reversed");
            }
        }
    }

    // The selected row must look the same whether or not it carries a warning:
    // the reversed selection patch turns a yellow caution background into a
    // solid block, so a warned selected row looked nothing like a clean one.
    #[test]
    fn a_selected_warned_row_looks_like_any_other_selected_row() {
        let clean = vec![
            {
                let mut m = placed("Alpha", 0);
                m.workspace = Some(1);
                m
            },
            {
                let mut m = placed("Beta", 1920);
                m.workspace = Some(2);
                m
            },
        ];
        // Same layout, but both monitors share a workspace (duplicate warning).
        let warned = vec![
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

        let clean_buf = render_list_to(&clean, TUIMode::View, 0, area);
        let warned_buf = render_list_to(&warned, TUIMode::View, 0, area);

        let (cx, cy) = find_text(&clean_buf, "Alpha").expect("clean row");
        let (wx, wy) = find_text(&warned_buf, "Alpha").expect("warned row");
        assert_eq!(cy, wy, "both render on the first data row");
        assert_eq!(
            clean_buf[(cx, cy)].style(),
            warned_buf[(wx, wy)].style(),
            "the selected warned row must have the same background/style"
        );
        assert_ne!(
            warned_buf[(wx, wy)].style().bg,
            Some(Color::Yellow),
            "the selection must not add a caution background"
        );
    }

    #[test]
    fn an_unselected_warned_row_keeps_its_caution_background() {
        let warned = vec![
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
        // Select the second row; the first (warned) row stays unselected.
        let buf = render_list_to(&warned, TUIMode::View, 1, area);

        let (x, y) = find_text(&buf, "Alpha").expect("warned row");
        assert_eq!(
            buf[(x, y)].style().bg,
            Some(Color::Yellow),
            "an unselected warned row still carries the caution background"
        );
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

    #[test]
    fn narrow_lists_drop_trailing_columns_instead_of_squeezing() {
        let monitors = test_monitors();

        let wide = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 140, 9),
        ));
        assert!(
            wide.contains("workspace"),
            "wide keeps every column: {wide}"
        );

        let medium = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 70, 9),
        ));
        assert!(medium.contains("description"), "medium keeps descriptions");
        assert!(
            !medium.contains("rotation"),
            "medium drops rotation: {medium}"
        );

        let narrow = text_of(&render_list_to(
            &monitors,
            TUIMode::View,
            0,
            Rect::new(0, 0, 50, 9),
        ));
        assert!(narrow.contains("name"), "narrow keeps names: {narrow}");
        assert!(
            !narrow.contains("description"),
            "narrow drops descriptions: {narrow}"
        );
    }

    #[test]
    fn a_hovered_row_glows_in_the_accent_colour() {
        let monitors = vec![placed("Alpha", 0), placed("Beta", 1920)];
        let mut list = MonitorList {
            state: TableState::default().with_selected(Some(2)),
            selected_row: Some(2),
            mode: TUIMode::View,
            monitors: &monitors,
            is_previewing: false,
            moved_pending: false,
            hover: Some(crate::app::state::HoverTarget::ListRow(1)),
        };
        let area = Rect::new(0, 0, 130, 9);
        let mut buf = Buffer::empty(area);
        list.render(area, &mut buf);

        let (x, y) = find_text(&buf, "Beta").expect("Beta row");
        assert_eq!(
            buf[(x, y)].style().fg,
            Some(crate::ui::styles::hover_accent()),
            "the hovered row carries the steady accent"
        );
    }
}
