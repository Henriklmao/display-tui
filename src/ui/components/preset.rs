//! Preset menu component for managing saved monitor configurations.

use crate::config::{count_enabled_monitors_in_preset, load_preset};
use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
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
                            self.state = MenuState::RenameName(entry.name.clone(), entry.name.clone());
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
// Responsive popup rect for preset menu
fn preset_rect(area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Max(30),
            Constraint::Min(1),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Max(60),
            Constraint::Min(1),
        ])
        .split(vertical[1])[1]
}
impl PresetMenu {
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let popup_area = preset_rect(area);
        Clear.render(popup_area, buf);

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
                        let marker = if i == self.selected_index { " > " } else { "   " };
                        let number = format!("{:>2}. ", i + 1);
                        // Presets with 0 enabled monitors get a dimmed indicator.
                        if entry.enabled_count == 0 {
                            lines.push(Line::from(format!("{}{}{}", marker, number, display)).dark_gray());
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
                        lines.push(line);
                    }
                }
                lines.push(Line::from(""));
                // Responsive hints
                if popup_area.width >= 60 {
                    lines.push(Line::from("[Enter/Space] Apply  [o] Override  [n] New").dim());
                    lines.push(Line::from("[d] Delete  [r] Rename  [Esc] Close").dim());
                } else {
                    lines.push(Line::from("[Enter] Apply [o] Override [n] New").dim());
                    lines.push(Line::from("[d] Delete [r] Rename [Esc] Close").dim());
                }
                lines
            }
            MenuState::CreateName(name) => {
                vec![
                    Line::from(" Create New Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Name: {} ", name).yellow()),
                    Line::from(""),
                    if popup_area.width >= 60 {
                        Line::from("[Enter] Save  [Esc] Cancel").dim()
                    } else {
                        Line::from("[Enter] Save [Esc] Cancel").dim()
                    },
                ]
            }
            MenuState::DeleteConfirm(name) => {
                vec![
                    Line::from(" Delete Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Are you sure you want to delete '{}'? ", name).yellow()),
                    Line::from(""),
                    if popup_area.width >= 60 {
                        Line::from("[y] Yes  [n] No  [Esc] Cancel").dim()
                    } else {
                        Line::from("[y] Yes [n] No [Esc] Cancel").dim()
                    },
                ]
            }
            MenuState::RenameName(old_name, new_name) => {
                vec![
                    Line::from(" Rename Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Rename '{}' to: {} ", old_name, new_name).yellow()),
                    Line::from(""),
                    if popup_area.width >= 60 {
                        Line::from("[Enter] Save  [Esc] Cancel").dim()
                    } else {
                        Line::from("[Enter] Save [Esc] Cancel").dim()
                    },
                ]
            }
        };

        if let Some(err) = &self.error_message {
            text.push(Line::from(""));
            text.push(Line::from(err.as_str()).red());
        }

        let p = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Presets ".white()),
        );
        p.render(popup_area, buf);
    }
}

fn is_valid_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn text_of(buf: &Buffer) -> String {
        let mut out = String::new();
        for y in buf.area.top()..buf.area.bottom() {
            for x in buf.area.left()..buf.area.right() {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn find_cell(buf: &Buffer, symbol: &str) -> Option<(u16, u16)> {
        for y in buf.area.top()..buf.area.bottom() {
            for x in buf.area.left()..buf.area.right() {
                if buf[(x, y)].symbol() == symbol {
                    return Some((x, y));
                }
            }
        }
        None
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
        assert_eq!(m.handle_event(KeyCode::Char('j')), MenuEvent::Preview("gaming".to_string()));
        assert_eq!(m.selected_index, 1);

        assert_eq!(m.handle_event(KeyCode::Char('k')), MenuEvent::Preview("desk".to_string()));
        assert_eq!(m.selected_index, 0);

        assert_eq!(m.handle_event(KeyCode::Up), MenuEvent::Preview("desk".to_string()));
        assert_eq!(m.selected_index, 0, "must not wrap around");
    }

    #[test]
    fn navigation_reports_the_newly_selected_preset() {
        let mut m = two_presets();
        m.selected_index = 1;

        assert_eq!(m.handle_event(KeyCode::Up), MenuEvent::Preview("desk".to_string()));
        assert_eq!(m.handle_event(KeyCode::Down), MenuEvent::Preview("gaming".to_string()));
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
        assert_eq!(m.state, MenuState::RenameName("desk".into(), "deskq".into()));
        assert_eq!(m.error_message, None);

        let mut m = renaming("desk", "desk");
        assert_eq!(m.handle_event(KeyCode::Char('Q')), MenuEvent::Handled);
        assert_eq!(m.state, MenuState::RenameName("desk".into(), "deskQ".into()));
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
            MenuEvent::Action(PresetAction::Rename(
                "desk".to_string(),
                "desx".to_string()
            ))
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
        assert!(!text.contains("monitors)"), "count is replaced by the label");
    }

    #[test]
    fn the_error_message_is_appended_to_the_render() {
        let mut m = two_presets();
        m.set_error("something went wrong".to_string());

        let text = text_of(&render(&m, Rect::new(0, 0, 80, 30)));

        assert!(text.contains("something went wrong"), "got: {text}");
        assert!(text.contains("[Enter/Space] Apply"), "got: {text}");
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
}
