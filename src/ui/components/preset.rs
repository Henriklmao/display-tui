//! Preset menu component for managing saved monitor configurations.

use crate::app::state::HoverTarget;
use crate::config::{count_enabled_monitors_in_preset, load_preset};
use crate::ui::styles;
use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

#[derive(Debug, PartialEq, Eq)]
pub enum PresetAction {
    Create(String),
    Delete(String),
    Rename(String, String), // (old_name, new_name)
    Apply(String),
    Override(String),
}

// Result of handling a key event in the preset menu.
#[derive(Debug, PartialEq, Eq)]
pub enum MenuEvent {
    // A concrete action should be performed by the caller.
    Action(PresetAction),
    // The preset under the cursor should be previewed (live preview).
    Preview(String),
    // The key was consumed by the menu (navigation, typing, cancel-to-list).
    Handled,
    // The key was not handled by the menu (caller may act, e.g. close on Esc).
    Ignored,
}

#[derive(Debug, PartialEq)]
pub enum MenuState {
    List,
    CreateName(String),
    DeleteConfirm(String),
    RenameName(String, String), // (old_name, new_name)
}

// An action a clickable preset-menu button performs. Its key is dispatched
// through the regular menu handler, so a click and the key stay in sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetButtonAction {
    Apply,
    Override,
    New,
    Delete,
    Rename,
    Close,
    Save,
    Cancel,
    Yes,
    No,
}

impl PresetButtonAction {
    pub fn key(self) -> KeyCode {
        match self {
            PresetButtonAction::Apply | PresetButtonAction::Save => KeyCode::Enter,
            PresetButtonAction::Override => KeyCode::Char('o'),
            PresetButtonAction::New => KeyCode::Char('n'),
            PresetButtonAction::Delete => KeyCode::Char('d'),
            PresetButtonAction::Rename => KeyCode::Char('r'),
            PresetButtonAction::Close
            | PresetButtonAction::Cancel
            | PresetButtonAction::No => KeyCode::Esc,
            PresetButtonAction::Yes => KeyCode::Char('y'),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PresetButtonAction::Apply => "Apply",
            PresetButtonAction::Override => "Override",
            PresetButtonAction::New => "New",
            PresetButtonAction::Delete => "Delete",
            PresetButtonAction::Rename => "Rename",
            PresetButtonAction::Close => "Close",
            PresetButtonAction::Save => "Save",
            PresetButtonAction::Cancel => "Cancel",
            PresetButtonAction::Yes => "Yes",
            PresetButtonAction::No => "No",
        }
    }

    fn key_label(self) -> &'static str {
        match self {
            PresetButtonAction::Apply | PresetButtonAction::Save => "Enter",
            PresetButtonAction::Override => "o",
            PresetButtonAction::New => "n",
            PresetButtonAction::Delete => "d",
            PresetButtonAction::Rename => "r",
            PresetButtonAction::Close | PresetButtonAction::Cancel => "Esc",
            PresetButtonAction::Yes => "y",
            // The delete prompt's "No" returns to the list; Esc is its wider
            // cancel key, but the button advertises the dedicated `n`.
            PresetButtonAction::No => "n",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            PresetButtonAction::Apply => "Apply the selected preset",
            PresetButtonAction::Override => "Overwrite the preset with the current layout",
            PresetButtonAction::New => "Create a new preset",
            PresetButtonAction::Delete => "Delete the selected preset",
            PresetButtonAction::Rename => "Rename the selected preset",
            PresetButtonAction::Close => "Close the preset menu",
            PresetButtonAction::Save => "Save the entered name",
            PresetButtonAction::Cancel => "Cancel without saving",
            PresetButtonAction::Yes => "Delete the preset",
            PresetButtonAction::No => "Keep the preset",
        }
    }
}

// A clickable button placed in the menu footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetButton {
    pub action: PresetButtonAction,
    pub rect: Rect,
}

// The menu's frame, content area and footer buttons for one screen area.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PresetFrame {
    pub frame: Rect,
    pub content: Rect,
    pub buttons: Vec<PresetButton>,
}

// Preset entry with name, enabled monitor count, and hardware-match status.
#[derive(Debug, Clone)]
pub struct PresetEntry {
    pub name: String,
    pub enabled_count: usize,
    pub has_mismatch: bool,
}

// PresetMenu component.
pub struct PresetMenu {
    pub state: MenuState,
    pub presets: Vec<PresetEntry>,
    pub selected_index: usize,
    pub error_message: Option<String>,
    pub active_preset: Option<String>,
}

impl PresetMenu {
    pub fn new(
        preset_names: Vec<String>,
        connected_names: &[String],
        active_preset: Option<String>,
    ) -> Self {
        let presets = preset_names
            .into_iter()
            .map(|name| {
                let enabled_count = count_enabled_monitors_in_preset(&name).unwrap_or(0);
                let has_mismatch = match load_preset(&name) {
                    Some(state) => {
                        let enabled_in_preset: Vec<&str> = state
                            .iter()
                            .filter(|m| m.enabled)
                            .map(|m| m.name.as_str())
                            .collect();
                        !enabled_in_preset.is_empty()
                            && !enabled_in_preset
                                .iter()
                                .all(|n| connected_names.iter().any(|c| c == *n))
                    }
                    None => false,
                };
                PresetEntry {
                    name,
                    enabled_count,
                    has_mismatch,
                }
            })
            .collect();
        Self {
            state: MenuState::List,
            presets,
            selected_index: 0,
            error_message: None,
            active_preset,
        }
    }

    // Sets the error message displayed at the bottom of the menu.
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    // The popup rectangle this menu occupies for a given screen area.
    pub fn rect(area: Rect) -> Rect {
        preset_rect(area)
    }

    // The footer buttons offered by the current menu state.
    fn actions(&self) -> Vec<PresetButtonAction> {
        match self.state {
            MenuState::List => vec![
                PresetButtonAction::Apply,
                PresetButtonAction::Override,
                PresetButtonAction::New,
                PresetButtonAction::Delete,
                PresetButtonAction::Rename,
                PresetButtonAction::Close,
            ],
            MenuState::CreateName(_) | MenuState::RenameName(_, _) => vec![
                PresetButtonAction::Save,
                PresetButtonAction::Cancel,
            ],
            MenuState::DeleteConfirm(_) => vec![
                PresetButtonAction::Yes,
                PresetButtonAction::No,
                PresetButtonAction::Cancel,
            ],
        }
    }

    // The frame, content area and clickable footer buttons for one screen area.
    pub fn layout(&self, area: Rect) -> PresetFrame {
        let frame = preset_rect(area);
        let inner = Block::bordered().inner(frame);
        if inner.width == 0 || inner.height == 0 {
            return PresetFrame {
                frame,
                content: Rect::default(),
                buttons: Vec::new(),
            };
        }

        let labels: Vec<(String, PresetButtonAction)> = self
            .actions()
            .into_iter()
            .map(|action| (button_label(action), action))
            .collect();
        let rows = wrap_buttons(&labels, inner.width);
        let footer_height = (rows.len() as u16).min(inner.height);
        let content_height = inner.height.saturating_sub(footer_height);
        let content = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: content_height,
        };
        let footer_top = inner.y.saturating_add(content_height);

        let mut buttons = Vec::new();
        for (row, row_buttons) in rows.iter().enumerate() {
            let y = footer_top.saturating_add(row as u16);
            if y >= inner.bottom() {
                break;
            }
            let row_width: u16 = row_buttons
                .iter()
                .map(|(label, _)| label.chars().count() as u16)
                .sum::<u16>()
                .saturating_add(row_buttons.len().saturating_sub(1) as u16);
            let mut x = inner.x + inner.width.saturating_sub(row_width) / 2;
            for (label, action) in row_buttons {
                let width = (label.chars().count() as u16)
                    .min(inner.right().saturating_sub(x));
                if width == 0 {
                    break;
                }
                buttons.push(PresetButton {
                    action: *action,
                    rect: Rect {
                        x,
                        y,
                        width,
                        height: 1,
                    },
                });
                x = x.saturating_add(width).saturating_add(1);
            }
        }

        PresetFrame {
            frame,
            content,
            buttons,
        }
    }

    // The button under a screen point, if any.
    pub fn button_at(&self, area: Rect, point: (u16, u16)) -> Option<PresetButtonAction> {
        self.layout(area)
            .buttons
            .into_iter()
            .find(|button| inside(button.rect, point))
            .map(|button| button.action)
    }

    // Maps a click's row to a preset index; only the list state is clickable.
    pub fn row_at(&self, area: Rect, y: u16) -> Option<usize> {
        if self.state != MenuState::List {
            return None;
        }
        let layout = self.layout(area);
        // One border line plus the leading blank line before the entries.
        let top = layout.content.y.saturating_add(1);
        if y < top || y >= layout.content.bottom() {
            return None;
        }
        let index = (y - top) as usize;
        if index < self.presets.len() {
            Some(index)
        } else {
            None
        }
    }

    // Selects a preset by index (list state only) and returns its name.
    pub fn select_index(&mut self, index: usize) -> Option<String> {
        if self.state != MenuState::List {
            return None;
        }
        let name = self.presets.get(index)?.name.clone();
        self.selected_index = index;
        self.error_message = None;
        Some(name)
    }

    // Moves the selection by a wheel notch (clamped, no wrap) and returns the
    // newly selected preset name.
    pub fn scroll(&mut self, delta: i32) -> Option<String> {
        if self.state != MenuState::List || self.presets.is_empty() {
            return None;
        }
        let last = self.presets.len() - 1;
        self.selected_index = if delta < 0 {
            self.selected_index.saturating_sub(1)
        } else {
            (self.selected_index + 1).min(last)
        };
        self.error_message = None;
        self.presets
            .get(self.selected_index)
            .map(|e| e.name.clone())
    }

    // Handles key events for the preset menu and returns how the event was consumed.
    pub fn handle_event(&mut self, key: KeyCode) -> MenuEvent {
        // Clear any stale error message from a previous key press.
        self.error_message = None;

        match &mut self.state {
            MenuState::List => match key {
                KeyCode::Char(c @ '1'..='9') => {
                    let idx = c.to_digit(10).unwrap() as usize - 1;
                    if idx < self.presets.len() {
                        self.selected_index = idx;
                    }
                    MenuEvent::Handled
                }
                KeyCode::Char('n') => {
                    self.state = MenuState::CreateName(String::new());
                    MenuEvent::Handled
                }
                KeyCode::Char('d') => match self.presets.get(self.selected_index) {
                    Some(entry) => {
                        if crate::config::is_last_preset(&entry.name) {
                            self.set_error("Cannot delete read-only 'last' preset".to_string());
                        } else {
                            self.state = MenuState::DeleteConfirm(entry.name.clone());
                        }
                        MenuEvent::Handled
                    }
                    None => MenuEvent::Handled,
                },
                KeyCode::Char('r') => match self.presets.get(self.selected_index) {
                    Some(entry) => {
                        if crate::config::is_last_preset(&entry.name) {
                            self.set_error("Cannot rename read-only 'last' preset".to_string());
                        } else {
                            self.state =
                                MenuState::RenameName(entry.name.clone(), entry.name.clone());
                        }
                        MenuEvent::Handled
                    }
                    None => MenuEvent::Handled,
                },
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                    // Return preview of the newly selected preset.
                    if let Some(entry) = self.presets.get(self.selected_index) {
                        return MenuEvent::Preview(entry.name.clone());
                    }
                    MenuEvent::Handled
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected_index < self.presets.len().saturating_sub(1) {
                        self.selected_index += 1;
                    }
                    if let Some(entry) = self.presets.get(self.selected_index) {
                        return MenuEvent::Preview(entry.name.clone());
                    }
                    MenuEvent::Handled
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(entry) = self.presets.get(self.selected_index) {
                        return MenuEvent::Action(PresetAction::Apply(entry.name.clone()));
                    }
                    MenuEvent::Handled
                }
                KeyCode::Char('o') => {
                    if let Some(entry) = self.presets.get(self.selected_index) {
                        return MenuEvent::Action(PresetAction::Override(entry.name.clone()));
                    }
                    MenuEvent::Handled
                }
                // Esc/q in the list is left for the caller to close the menu.
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => MenuEvent::Ignored,
                _ => MenuEvent::Ignored,
            },
            MenuState::CreateName(name) => match key {
                KeyCode::Char(c) if is_valid_name_char(c) => {
                    name.push(c);
                    MenuEvent::Handled
                }
                KeyCode::Backspace => {
                    name.pop();
                    MenuEvent::Handled
                }
                KeyCode::Enter => {
                    if name.is_empty() {
                        self.set_error("Preset name cannot be empty".to_string());
                    } else {
                        return MenuEvent::Action(PresetAction::Create(name.clone()));
                    }
                    MenuEvent::Handled
                }
                // 'q'/'Q' are valid name characters and are consumed by the
                // typing arm above, so only Esc can cancel.
                KeyCode::Esc => {
                    self.state = MenuState::List;
                    MenuEvent::Handled
                }
                _ => MenuEvent::Ignored,
            },
            MenuState::RenameName(old_name, new_name) => match key {
                KeyCode::Char(c) if is_valid_name_char(c) => {
                    new_name.push(c);
                    MenuEvent::Handled
                }
                KeyCode::Backspace => {
                    new_name.pop();
                    MenuEvent::Handled
                }
                KeyCode::Enter => {
                    if new_name.is_empty() {
                        self.set_error("Preset name cannot be empty".to_string());
                    } else if new_name == old_name {
                        self.set_error("New name must differ from the old name".to_string());
                    } else {
                        return MenuEvent::Action(PresetAction::Rename(
                            old_name.clone(),
                            new_name.clone(),
                        ));
                    }
                    MenuEvent::Handled
                }
                // 'q'/'Q' are valid name characters and are consumed by the
                // typing arm above, so only Esc can cancel.
                KeyCode::Esc => {
                    self.state = MenuState::List;
                    MenuEvent::Handled
                }
                _ => MenuEvent::Ignored,
            },
            MenuState::DeleteConfirm(name) => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    MenuEvent::Action(PresetAction::Delete(name.clone()))
                }
                KeyCode::Esc
                | KeyCode::Char('q')
                | KeyCode::Char('Q')
                | KeyCode::Char('n')
                | KeyCode::Char('N') => {
                    self.state = MenuState::List;
                    MenuEvent::Handled
                }
                _ => MenuEvent::Ignored,
            },
        }
    }
}
// The full chip text for a menu button.
fn button_label(action: PresetButtonAction) -> String {
    format!("[ {}  {} ]", action.label(), action.key_label())
}

// The styled spans of a menu button.
fn button_spans(action: PresetButtonAction) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!("[ {}  ", action.label()),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            action.key_label().to_string(),
            Style::default().fg(Color::Blue).bold(),
        ),
        Span::styled(" ]", Style::default().fg(Color::Gray)),
    ]
}

// Half-open containment so adjacent buttons never both claim a cell.
fn inside(rect: Rect, point: (u16, u16)) -> bool {
    point.0 >= rect.x && point.0 < rect.right() && point.1 >= rect.y && point.1 < rect.bottom()
}

// Greedily wraps button labels into rows that fit `width`.
fn wrap_buttons(
    labels: &[(String, PresetButtonAction)],
    width: u16,
) -> Vec<Vec<(String, PresetButtonAction)>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut row_width: u16 = 0;

    for (label, action) in labels {
        let label_width = label.chars().count() as u16;
        let needed = if row.is_empty() {
            label_width
        } else {
            row_width + 1 + label_width
        };
        if !row.is_empty() && needed > width {
            rows.push(std::mem::take(&mut row));
            row_width = 0;
        }
        row_width = if row.is_empty() {
            label_width
        } else {
            row_width + 1 + label_width
        };
        row.push((label.clone(), *action));
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

// Responsive popup rect for preset menu
fn preset_rect(area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Max(30), Constraint::Min(1)])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Max(60), Constraint::Min(1)])
        .split(vertical[1])[1]
}
impl PresetMenu {
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_with(area, buf, None);
    }

    pub fn render_with(
        &self,
        area: Rect,
        buf: &mut Buffer,
        hover: Option<crate::app::state::HoverTarget>,
    ) {
        let layout = self.layout(area);
        let frame = layout.frame;
        if frame.width == 0 || frame.height == 0 {
            return;
        }
        Clear.render(frame, buf);

        let mut text: Vec<Line> = match &self.state {
            MenuState::List => {
                let mut lines = vec![Line::from("")];
                if self.presets.is_empty() {
                    lines.push(Line::from("No presets found").dim());
                } else {
                    for (i, entry) in self.presets.iter().enumerate() {
                        let is_last = crate::config::is_last_preset(&entry.name);
                        let is_active = self.active_preset.as_deref() == Some(entry.name.as_str());
                        let display = if is_last {
                            format!("{} [read-only]", entry.name)
                        } else {
                            format!("{} ({} monitors)", entry.name, entry.enabled_count)
                        };
                        let marker = if i == self.selected_index {
                            " > "
                        } else {
                            "   "
                        };
                        let number = format!("{:>2}. ", i + 1);
                        // Presets with 0 enabled monitors get a dimmed indicator.
                        if entry.enabled_count == 0 {
                            lines.push(
                                Line::from(format!("{}{}{}", marker, number, display)).dark_gray(),
                            );
                            continue;
                        }
                        let mut line = Line::from(format!("{}{}{}", marker, number, display));
                        if entry.has_mismatch {
                            line = line.red();
                            if i == self.selected_index {
                                line = line.bold();
                            }
                        } else if i == self.selected_index {
                            line = line.cyan();
                            if is_last {
                                line = line.dim();
                            }
                        } else if is_last {
                            line = line.dim();
                        }
                        // Highlight the currently active preset in bold.
                        if is_active {
                            line = line.bold();
                        }
                        if hover == Some(HoverTarget::PresetRow(i)) {
                            line = line.style(styles::hover_style());
                        }
                        lines.push(line);
                    }
                }
                lines.push(Line::from(""));
                lines
            }
            MenuState::CreateName(name) => {
                vec![
                    Line::from(" Create New Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Name: {} ", name).yellow()),
                    Line::from(""),
                ]
            }
            MenuState::DeleteConfirm(name) => {
                vec![
                    Line::from(" Delete Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Are you sure you want to delete '{}'? ", name).yellow()),
                    Line::from(""),
                ]
            }
            MenuState::RenameName(old_name, new_name) => {
                vec![
                    Line::from(" Rename Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Rename '{}' to: {} ", old_name, new_name).yellow()),
                    Line::from(""),
                ]
            }
        };

        if let Some(err) = &self.error_message {
            text.push(Line::from(""));
            text.push(Line::from(err.as_str()).red());
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Presets ".white());
        Widget::render(block, frame, buf);

        if layout.content.width > 0 && layout.content.height > 0 {
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .render(layout.content, buf);
        }

        for button in &layout.buttons {
            let hovered = hover == Some(HoverTarget::PresetButton(button.action));
            if hovered {
                Paragraph::new(
                    Line::from(button_label(button.action))
                        .style(styles::hover_style().reversed()),
                )
                .render(button.rect, buf);
            } else {
                Paragraph::new(Line::from(button_spans(button.action))).render(button.rect, buf);
            }
        }
    }
}

fn is_valid_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{find_cell, text_of};
    use ratatui::style::Color;

    fn entry(name: &str, enabled_count: usize) -> PresetEntry {
        PresetEntry {
            name: name.to_string(),
            enabled_count,
            has_mismatch: false,
        }
    }

    fn menu(presets: Vec<PresetEntry>) -> PresetMenu {
        PresetMenu {
            state: MenuState::List,
            presets,
            selected_index: 0,
            error_message: None,
            active_preset: None,
        }
    }

    fn two_presets() -> PresetMenu {
        menu(vec![entry("desk", 2), entry("gaming", 1)])
    }

    // Builds a menu that is already in the create-name sub state.
    fn creating(initial: &str) -> PresetMenu {
        let mut m = two_presets();
        m.state = MenuState::CreateName(initial.to_string());
        m
    }

    fn renaming(old: &str, new: &str) -> PresetMenu {
        let mut m = two_presets();
        m.state = MenuState::RenameName(old.to_string(), new.to_string());
        m
    }

    fn confirming(name: &str) -> PresetMenu {
        let mut m = two_presets();
        m.state = MenuState::DeleteConfirm(name.to_string());
        m
    }

    fn render(m: &PresetMenu, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        m.render(area, &mut buf);
        buf
    }

    // --- construction -------------------------------------------------

    #[test]
    fn new_starts_in_the_list_state() {
        let m = PresetMenu::new(vec!["a".to_string(), "b".to_string()], &[], None);

        assert_eq!(m.state, MenuState::List);
        assert_eq!(m.selected_index, 0);
        assert_eq!(m.presets.len(), 2);
        assert_eq!(m.error_message, None);
        assert_eq!(m.active_preset, None);
    }

    #[test]
    fn new_maps_names_it_cannot_load_to_empty_entries() {
        // An invalid name can never exist on disk, so this is deterministic.
        let m = PresetMenu::new(vec!["___".to_string()], &["DP-1".to_string()], None);

        assert_eq!(m.presets.len(), 1);
        assert_eq!(m.presets[0].enabled_count, 0);
        assert!(!m.presets[0].has_mismatch);
    }

    // --- list navigation ----------------------------------------------

    #[test]
    fn digit_keys_jump_directly_to_a_preset() {
        let mut m = two_presets();

        assert_eq!(m.handle_event(KeyCode::Char('2')), MenuEvent::Handled);
        assert_eq!(m.selected_index, 1);
    }

    #[test]
    fn digit_keys_out_of_range_are_ignored() {
        let mut m = two_presets();

        assert_eq!(m.handle_event(KeyCode::Char('9')), MenuEvent::Handled);
        assert_eq!(m.selected_index, 0, "selection must not move");
        assert_eq!(m.handle_event(KeyCode::Char('0')), MenuEvent::Ignored);
    }

    #[test]
    fn arrow_and_vi_keys_move_the_selection_within_bounds() {
        let mut m = two_presets();

        assert!(matches!(
            m.handle_event(KeyCode::Down),
            MenuEvent::Preview(_)
        ));
        assert_eq!(m.selected_index, 1);

        // Already at the bottom: stays put but still previews.
        assert_eq!(
            m.handle_event(KeyCode::Char('j')),
            MenuEvent::Preview("gaming".to_string())
        );
        assert_eq!(m.selected_index, 1);

        assert_eq!(
            m.handle_event(KeyCode::Char('k')),
            MenuEvent::Preview("desk".to_string())
        );
        assert_eq!(m.selected_index, 0);

        assert_eq!(
            m.handle_event(KeyCode::Up),
            MenuEvent::Preview("desk".to_string())
        );
        assert_eq!(m.selected_index, 0, "must not wrap around");
    }

    #[test]
    fn navigation_reports_the_newly_selected_preset() {
        let mut m = two_presets();
        m.selected_index = 1;

        assert_eq!(
            m.handle_event(KeyCode::Up),
            MenuEvent::Preview("desk".to_string())
        );
        assert_eq!(
            m.handle_event(KeyCode::Down),
            MenuEvent::Preview("gaming".to_string())
        );
    }

    #[test]
    fn navigation_on_an_empty_list_does_not_panic() {
        let mut m = menu(vec![]);

        assert_eq!(m.handle_event(KeyCode::Down), MenuEvent::Handled);
        assert_eq!(m.handle_event(KeyCode::Up), MenuEvent::Handled);
        assert_eq!(m.handle_event(KeyCode::Enter), MenuEvent::Handled);
        assert_eq!(m.handle_event(KeyCode::Char('o')), MenuEvent::Handled);
        assert_eq!(m.handle_event(KeyCode::Char('d')), MenuEvent::Handled);
        assert_eq!(m.handle_event(KeyCode::Char('r')), MenuEvent::Handled);
    }

    #[test]
    fn enter_and_space_apply_the_selection() {
        let mut m = two_presets();

        assert_eq!(
            m.handle_event(KeyCode::Enter),
            MenuEvent::Action(PresetAction::Apply("desk".to_string()))
        );
        assert_eq!(
            m.handle_event(KeyCode::Char(' ')),
            MenuEvent::Action(PresetAction::Apply("desk".to_string()))
        );
    }

    #[test]
    fn o_overrides_the_selection() {
        let mut m = two_presets();
        m.handle_event(KeyCode::Char('2'));

        assert_eq!(
            m.handle_event(KeyCode::Char('o')),
            MenuEvent::Action(PresetAction::Override("gaming".to_string()))
        );
    }

    #[test]
    fn close_keys_are_left_to_the_caller() {
        let mut m = two_presets();

        assert_eq!(m.handle_event(KeyCode::Esc), MenuEvent::Ignored);
        assert_eq!(m.handle_event(KeyCode::Char('q')), MenuEvent::Ignored);
        assert_eq!(m.handle_event(KeyCode::Char('Q')), MenuEvent::Ignored);
        assert_eq!(m.handle_event(KeyCode::Char('x')), MenuEvent::Ignored);
        assert_eq!(m.state, MenuState::List);
    }

    // --- delete / rename guards ---------------------------------------

    #[test]
    fn delete_enters_a_confirmation_state() {
        let mut m = two_presets();

        assert_eq!(m.handle_event(KeyCode::Char('d')), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::DeleteConfirm("desk".to_string()));
    }

    #[test]
    fn the_reserved_preset_cannot_be_deleted_or_renamed() {
        let mut m = menu(vec![entry("last", 2)]);
        m.selected_index = 0;

        assert_eq!(m.handle_event(KeyCode::Char('d')), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::List, "no confirm dialog");
        assert_eq!(
            m.error_message.as_deref(),
            Some("Cannot delete read-only 'last' preset")
        );

        assert_eq!(m.handle_event(KeyCode::Char('r')), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::List, "no rename dialog");
        assert_eq!(
            m.error_message.as_deref(),
            Some("Cannot rename read-only 'last' preset")
        );
    }

    #[test]
    fn rename_prefills_the_current_name() {
        let mut m = two_presets();

        assert_eq!(m.handle_event(KeyCode::Char('r')), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::RenameName("desk".into(), "desk".into()));
    }

    #[test]
    fn every_event_clears_a_stale_error_message() {
        let mut m = two_presets();
        m.set_error("boom".to_string());

        m.handle_event(KeyCode::Char('x'));

        assert_eq!(m.error_message, None);
    }

    // --- create-name sub state ----------------------------------------

    #[test]
    fn create_stores_typed_characters() {
        let mut m = creating("");

        for c in ['m', 'y', '-', '1'] {
            assert_eq!(m.handle_event(KeyCode::Char(c)), MenuEvent::Handled);
        }
        assert_eq!(m.state, MenuState::CreateName("my-1".to_string()));
    }

    #[test]
    fn create_rejects_characters_a_preset_name_cannot_contain() {
        let mut m = creating("");

        for c in [' ', '_', '.', '/', 'é', '!'] {
            assert_eq!(
                m.handle_event(KeyCode::Char(c)),
                MenuEvent::Ignored,
                "{:?} must be rejected",
                c
            );
        }
        assert_eq!(m.state, MenuState::CreateName(String::new()));
    }

    #[test]
    fn create_backspace_erases_and_keeps_the_state() {
        let mut m = creating("ab");

        assert_eq!(m.handle_event(KeyCode::Backspace), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::CreateName("a".to_string()));

        // Backspacing an empty name is harmless.
        m.handle_event(KeyCode::Backspace);
        m.handle_event(KeyCode::Backspace);
        assert_eq!(m.state, MenuState::CreateName(String::new()));
    }

    #[test]
    fn create_refuses_an_empty_name() {
        let mut m = creating("");

        assert_eq!(m.handle_event(KeyCode::Enter), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::CreateName(String::new()));
        assert_eq!(
            m.error_message.as_deref(),
            Some("Preset name cannot be empty")
        );
    }

    #[test]
    fn create_emits_the_action_once_a_name_is_typed() {
        let mut m = creating("");
        m.handle_event(KeyCode::Char('d'));
        m.handle_event(KeyCode::Char('e'));

        assert_eq!(
            m.handle_event(KeyCode::Enter),
            MenuEvent::Action(PresetAction::Create("de".to_string()))
        );
        assert_eq!(m.error_message, None);
    }

    #[test]
    fn create_can_be_cancelled_with_escape() {
        let mut m = creating("x");

        assert_eq!(m.handle_event(KeyCode::Esc), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::List);
    }

    #[test]
    fn letters_are_always_typed_in_the_name_states() {
        // 'q'/'Q' are valid name characters, so they are never treated as a
        // cancel key here — only Esc leaves the sub state. A cancel arm for
        // them would be dead code behind the typing arm.
        let mut m = creating("x");
        assert_eq!(m.handle_event(KeyCode::Char('q')), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::CreateName("xq".to_string()));

        let mut m = creating("x");
        assert_eq!(m.handle_event(KeyCode::Char('Q')), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::CreateName("xQ".to_string()));

        let mut m = renaming("desk", "desk");
        assert_eq!(m.handle_event(KeyCode::Char('q')), MenuEvent::Handled);
        assert_eq!(
            m.state,
            MenuState::RenameName("desk".into(), "deskq".into())
        );
        assert_eq!(m.error_message, None);

        let mut m = renaming("desk", "desk");
        assert_eq!(m.handle_event(KeyCode::Char('Q')), MenuEvent::Handled);
        assert_eq!(
            m.state,
            MenuState::RenameName("desk".into(), "deskQ".into())
        );
    }

    // --- rename sub state ---------------------------------------------

    #[test]
    fn rename_appends_to_the_prefilled_name() {
        let mut m = renaming("desk", "desk");

        assert_eq!(m.handle_event(KeyCode::Backspace), MenuEvent::Handled);
        assert_eq!(m.handle_event(KeyCode::Char('2')), MenuEvent::Handled);

        assert_eq!(m.state, MenuState::RenameName("desk".into(), "des2".into()));
    }

    #[test]
    fn rename_refuses_an_empty_or_unchanged_name() {
        let mut m = renaming("desk", "desk");
        assert_eq!(m.handle_event(KeyCode::Enter), MenuEvent::Handled);
        assert_eq!(
            m.error_message.as_deref(),
            Some("New name must differ from the old name")
        );

        let mut m = renaming("desk", "");
        assert_eq!(m.handle_event(KeyCode::Enter), MenuEvent::Handled);
        assert_eq!(
            m.error_message.as_deref(),
            Some("Preset name cannot be empty")
        );
        assert!(matches!(m.state, MenuState::RenameName(..)));
    }

    #[test]
    fn rename_emits_both_names_on_success() {
        let mut m = renaming("desk", "desk");
        // "desk" -> backspace -> "des" -> 'x' -> "desx"
        m.handle_event(KeyCode::Backspace);
        m.handle_event(KeyCode::Char('x'));

        assert_eq!(
            m.handle_event(KeyCode::Enter),
            MenuEvent::Action(PresetAction::Rename("desk".to_string(), "desx".to_string()))
        );
    }

    #[test]
    fn rename_can_be_cancelled() {
        let mut m = renaming("desk", "desk");

        assert_eq!(m.handle_event(KeyCode::Esc), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::List);
    }

    #[test]
    fn rename_rejects_invalid_characters() {
        let mut m = renaming("desk", "desk");

        assert_eq!(m.handle_event(KeyCode::Char(' ')), MenuEvent::Ignored);
        assert_eq!(m.state, MenuState::RenameName("desk".into(), "desk".into()));
    }

    // --- delete confirmation ------------------------------------------

    #[test]
    fn delete_is_confirmed_with_y_or_upper_case_y() {
        for key in [KeyCode::Char('y'), KeyCode::Char('Y')] {
            let mut m = confirming("desk");
            assert_eq!(
                m.handle_event(key),
                MenuEvent::Action(PresetAction::Delete("desk".to_string()))
            );
        }
    }

    #[test]
    fn delete_can_be_declined_with_several_keys() {
        for key in [
            KeyCode::Char('n'),
            KeyCode::Char('N'),
            KeyCode::Esc,
            KeyCode::Char('q'),
            KeyCode::Char('Q'),
        ] {
            let mut m = confirming("desk");
            assert_eq!(m.handle_event(key), MenuEvent::Handled, "key: {:?}", key);
            assert_eq!(m.state, MenuState::List, "key: {:?}", key);
        }
    }

    #[test]
    fn unexpected_keys_in_the_confirmation_are_ignored() {
        let mut m = confirming("desk");

        assert_eq!(m.handle_event(KeyCode::Char('z')), MenuEvent::Ignored);
        assert_eq!(m.state, MenuState::DeleteConfirm("desk".to_string()));
    }

    // --- name character filter ----------------------------------------

    #[test]
    fn only_ascii_alphanumerics_and_hyphens_are_allowed_in_names() {
        for c in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-".chars() {
            assert!(is_valid_name_char(c), "{:?} should be allowed", c);
        }
        for c in " _.+/!@#é".chars() {
            assert!(!is_valid_name_char(c), "{:?} should be rejected", c);
        }
    }

    // --- rendering -----------------------------------------------------

    #[test]
    fn list_renders_names_and_monitor_counts() {
        let m = two_presets();
        let text = text_of(&render(&m, Rect::new(0, 0, 80, 30)));

        assert!(text.contains("desk (2 monitors)"), "got: {text}");
        assert!(text.contains("gaming (1 monitors)"), "got: {text}");
        assert!(text.contains("Presets"));
    }

    #[test]
    fn a_long_preset_name_wraps_instead_of_clipping() {
        let long = "a-preset-name-that-is-quite-long-indeed-and-keeps-going";
        let m = menu(vec![entry(long, 2)]);

        let text = text_of(&render(&m, Rect::new(0, 0, 30, 12)));

        assert!(text.contains("Presets"), "got: {text}");
        assert!(
            text.contains("a-preset"),
            "the start of the name renders: {text}"
        );
    }

    #[test]
    fn empty_list_shows_a_placeholder() {
        let m = menu(vec![]);
        let text = text_of(&render(&m, Rect::new(0, 0, 80, 30)));

        assert!(text.contains("No presets found"), "got: {text}");
        assert!(!text.contains("monitors)"), "no entries are listed: {text}");
    }

    #[test]
    fn the_reserved_preset_is_labelled_read_only() {
        let m = menu(vec![entry("last", 2)]);
        let text = text_of(&render(&m, Rect::new(0, 0, 80, 30)));

        assert!(text.contains("last [read-only]"), "got: {text}");
        assert!(
            !text.contains("monitors)"),
            "count is replaced by the label"
        );
    }

    #[test]
    fn the_error_message_is_appended_to_the_render() {
        let mut m = two_presets();
        m.set_error("something went wrong".to_string());

        let text = text_of(&render(&m, Rect::new(0, 0, 80, 30)));

        assert!(text.contains("something went wrong"), "got: {text}");
        assert!(text.contains("[ Apply  Enter ]"), "got: {text}");
    }

    #[test]
    fn sub_state_prompts_are_rendered() {
        let area = Rect::new(0, 0, 80, 30);

        let text = text_of(&render(&creating("new"), area));
        assert!(text.contains("Create New Preset"), "got: {text}");
        assert!(text.contains("Name: new"), "got: {text}");

        let text = text_of(&render(&confirming("desk"), area));
        assert!(text.contains("Delete Preset"), "got: {text}");
        assert!(text.contains("delete 'desk'?"), "got: {text}");

        let text = text_of(&render(&renaming("desk", "desk2"), area));
        assert!(text.contains("Rename Preset"), "got: {text}");
        assert!(text.contains("Rename 'desk' to: desk2"), "got: {text}");
    }

    #[test]
    fn the_selected_row_is_highlighted() {
        let mut m = two_presets();
        m.selected_index = 1;

        let buf = render(&m, Rect::new(0, 0, 80, 30));
        let (x, y) = find_cell(&buf, ">").expect("selection marker");

        assert_eq!(
            buf[(x, y)].style().fg,
            Some(Color::Cyan),
            "the selected row must stand out"
        );
        assert!(text_of(&buf).contains(" >  2. gaming"));
    }

    #[test]
    fn render_stays_inside_the_popup_rectangle() {
        let area = Rect::new(0, 0, 80, 30);
        let m = two_presets();
        let buf = render(&m, area);
        let popup = preset_rect(area);

        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let inside = x >= popup.x
                    && x < popup.x + popup.width
                    && y >= popup.y
                    && y < popup.y + popup.height;
                if inside {
                    continue;
                }
                assert_eq!(buf[(x, y)].symbol(), " ", "spill at ({}, {})", x, y);
            }
        }
    }

    #[test]
    fn the_list_state_offers_all_six_buttons() {
        let actions: Vec<PresetButtonAction> = two_presets()
            .layout(Rect::new(0, 0, 80, 30))
            .buttons
            .iter()
            .map(|button| button.action)
            .collect();

        assert_eq!(
            actions,
            vec![
                PresetButtonAction::Apply,
                PresetButtonAction::Override,
                PresetButtonAction::New,
                PresetButtonAction::Delete,
                PresetButtonAction::Rename,
                PresetButtonAction::Close,
            ]
        );
    }

    #[test]
    fn sub_states_offer_their_own_buttons() {
        let area = Rect::new(0, 0, 80, 30);

        let actions = |menu: &PresetMenu| -> Vec<PresetButtonAction> {
            menu.layout(area)
                .buttons
                .iter()
                .map(|button| button.action)
                .collect()
        };

        assert_eq!(
            actions(&creating("x")),
            vec![PresetButtonAction::Save, PresetButtonAction::Cancel]
        );
        assert_eq!(
            actions(&confirming("desk")),
            vec![
                PresetButtonAction::Yes,
                PresetButtonAction::No,
                PresetButtonAction::Cancel
            ]
        );
        assert_eq!(
            actions(&renaming("desk", "desk2")),
            vec![PresetButtonAction::Save, PresetButtonAction::Cancel]
        );
    }

    #[test]
    fn button_at_maps_every_drawn_chip() {
        let area = Rect::new(0, 0, 80, 30);
        let m = two_presets();
        let frame = m.layout(area);

        for button in &frame.buttons {
            assert_eq!(
                m.button_at(area, (button.rect.x, button.rect.y)),
                Some(button.action)
            );
        }
        assert_eq!(m.button_at(area, (0, 0)), None);
    }

    #[test]
    fn the_hovered_button_is_inverted() {
        let area = Rect::new(0, 0, 80, 30);
        let m = two_presets();
        let button = m.layout(area).buttons[0];
        let mut buf = Buffer::empty(area);

        m.render_with(
            area,
            &mut buf,
            Some(HoverTarget::PresetButton(button.action)),
        );

        assert!(
            buf[(button.rect.x, button.rect.y)]
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::REVERSED),
            "the hovered chip must be inverted"
        );
    }

    #[test]
    fn the_footer_is_not_part_of_the_preset_list() {
        // Enough entries to make a footer row a plausible list index.
        let entries = (0..100).map(|i| entry(&format!("p{i}"), 1)).collect();
        let m = menu(entries);
        let area = Rect::new(0, 0, 80, 30);
        let button = m.layout(area).buttons[0];

        assert_eq!(
            m.row_at(area, button.rect.y),
            None,
            "a footer click must not select a clipped preset"
        );
    }

    #[test]
    fn preset_rect_is_capped_and_centred_on_large_areas() {
        let popup = preset_rect(Rect::new(0, 0, 120, 60));

        assert_eq!(popup.width, 60);
        assert_eq!(popup.height, 30);
        assert_eq!(popup.x, 30, "horizontally centred");
        assert_eq!(popup.y, 15, "vertically centred");
    }

    #[test]
    fn preset_rect_shrinks_to_the_area_when_there_is_no_room() {
        let area = Rect::new(0, 0, 30, 12);
        let popup = preset_rect(area);

        assert!(popup.width <= area.width);
        assert!(popup.height <= area.height);
        assert!(popup.x >= area.x && popup.y >= area.y);
        assert!(popup.width >= 1 && popup.height >= 1);
    }

    // --- mouse hit testing ----------------------------------------------

    #[test]
    fn row_at_maps_the_list_entries() {
        let m = two_presets();
        let area = Rect::new(0, 0, 80, 30);
        // Border line + leading blank line, then the entries.
        let top = PresetMenu::rect(area).y + 2;

        assert_eq!(m.row_at(area, top), Some(0));
        assert_eq!(m.row_at(area, top + 1), Some(1));
        assert_eq!(m.row_at(area, top - 1), None, "blank line");
        assert_eq!(m.row_at(area, top + 2), None, "past the list");
    }

    #[test]
    fn row_at_is_ignored_outside_the_list_state() {
        let mut m = two_presets();
        m.state = MenuState::CreateName("x".to_string());

        assert_eq!(m.row_at(Rect::new(0, 0, 80, 30), 10), None);
    }

    #[test]
    fn select_index_picks_a_preset_and_rejects_out_of_range() {
        let mut m = two_presets();

        assert_eq!(m.select_index(1), Some("gaming".to_string()));
        assert_eq!(m.selected_index, 1);
        assert_eq!(m.select_index(9), None);
        assert_eq!(m.selected_index, 1, "selection must survive");
    }

    #[test]
    fn scroll_clamps_without_wrapping() {
        let mut m = two_presets();

        // Already at the top: scrolling up stays put but still previews.
        assert_eq!(m.scroll(-1), Some("desk".to_string()));
        assert_eq!(m.selected_index, 0);
        assert_eq!(m.scroll(1), Some("gaming".to_string()));
        assert_eq!(m.scroll(1), Some("gaming".to_string()));
        assert_eq!(m.selected_index, 1);
    }

    #[test]
    fn select_and_scroll_do_nothing_while_typing() {
        let mut m = creating("x");

        assert_eq!(m.select_index(0), None);
        assert_eq!(m.scroll(1), None);
        assert_eq!(m.state, MenuState::CreateName("x".to_string()));

        let mut empty = menu(vec![]);
        assert_eq!(empty.scroll(1), None);
        assert_eq!(empty.selected_index, 0);
    }
}
