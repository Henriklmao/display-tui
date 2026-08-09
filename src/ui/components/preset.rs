//! Preset menu component for managing saved monitor configurations.

use crate::config::count_enabled_monitors_in_preset;
use crate::ui::layouts::centered_rect;
use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

pub enum PresetAction {
    Create(String),
    Delete(String),
    Rename(String, String), // (old_name, new_name)
    Apply(String),
    Override(String),
}

// Result of handling a key event in the preset menu.
pub enum MenuEvent {
    // A concrete action should be performed by the caller.
    Action(PresetAction),
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

// Preset entry with name and enabled monitor count.
#[derive(Debug, Clone)]
pub struct PresetEntry {
    pub name: String,
    pub enabled_count: usize,
}

/// PresetMenu component.
pub struct PresetMenu {
    pub state: MenuState,
    pub presets: Vec<PresetEntry>,
    pub selected_index: usize,
    pub error_message: Option<String>,
}

impl PresetMenu {
    pub fn new(preset_names: Vec<String>) -> Self {
        let presets = preset_names
            .into_iter()
            .map(|name| {
                let enabled_count = count_enabled_monitors_in_preset(&name).unwrap_or(0);
                PresetEntry { name, enabled_count }
            })
            .collect();
        Self {
            state: MenuState::List,
            presets,
            selected_index: 0,
            error_message: None,
        }
    }

    // Sets the error message displayed at the bottom of the menu.
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    // Handles key events for the preset menu and returns how the event was consumed.
    pub fn handle_event(&mut self, key: KeyCode) -> MenuEvent {
        // Clear any stae error message from a previous key press.
        self.error_message = None;

        match &mut self.state {
            MenuState::List => match key {
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
                    MenuEvent::Handled
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected_index < self.presets.len().saturating_sub(1) {
                        self.selected_index += 1;
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
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
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
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
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

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let popup_area = centered_rect(50, 40, area);
        Clear.render(popup_area, buf);

        let mut text: Vec<Line> = match &self.state {
            MenuState::List => {
                let mut lines = vec![Line::from("")];
                if self.presets.is_empty() {
                    lines.push(Line::from("No presets found").dim());
                } else {
                    for (i, entry) in self.presets.iter().enumerate() {
                        let is_last = crate::config::is_last_preset(&entry.name);
                        let display = if is_last {
                            format!("{} [read-only]", entry.name)
                        } else {
                            format!("{} ({} monitors)", entry.name, entry.enabled_count)
                        };
                        if i == self.selected_index {
                            if is_last {
                                lines.push(Line::from(format!(" > {} ", display)).cyan().dim());
                            } else {
                                lines.push(Line::from(format!(" > {} ", display).cyan()));
                            }
                        } else {
                            if is_last {
                                lines.push(Line::from(format!("   {} ", display)).dim());
                            } else {
                                lines.push(Line::from(format!("   {} ", display)));
                            }
                        }
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "[Enter/Space] Apply  [o] Override  [n] New  [d] Delete  [r] Rename  [Esc] Close".dim(),
                ));
                lines
            }
            MenuState::CreateName(name) => {
                vec![
                    Line::from(" Create New Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Name: {} ", name).yellow()),
                    Line::from(""),
                    Line::from("[Enter] Save  [Esc] Cancel".dim()),
                ]
            }
            MenuState::DeleteConfirm(name) => {
                vec![
                    Line::from(" Delete Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Are you sure you want to delete '{}'? ", name).yellow()),
                    Line::from(""),
                    Line::from("[y] Yes  [n] No  [Esc] Cancel".dim()),
                ]
            }
            MenuState::RenameName(old_name, new_name) => {
                vec![
                    Line::from(" Rename Preset ".bold().white()),
                    Line::from(""),
                    Line::from(format!(" Rename '{}' to: {} ", old_name, new_name).yellow()),
                    Line::from(""),
                    Line::from("[Enter] Save  [Esc] Cancel".dim()),
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
