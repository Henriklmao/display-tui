//! Application state and core logic.
//!
//! Contains the main App struct, its methods, and the Popup type.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use std::io;
use crate::config::{Configuration, save_monitor_state, load_monitor_state};
use crate::monitor::Monitor;
use crate::ui::components::{MonitorList, Map, Resolutions, Scale, PresetMenu};
use crate::utils::TUIMode;

// Main application state.
#[derive(Default)]
pub struct App {
    pub exit: bool,
    pub config: Configuration,
    pub monitors: Vec<Monitor>,
    pub selected_monitor: usize,
    pub selected_resolution: usize,
    pub selected_scale: usize,
    pub mode: TUIMode,
    pub show_help: bool,
    pub show_popup: Option<Popup>,
    pub show_preset_menu: Option<PresetMenu>,
}

// Popup message displayed to the user.
pub struct Popup {
    pub title: String,
    pub lines: Vec<String>,
    pub is_error: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.monitors = Monitor::get_monitors();

        // Load saved monitor positions/scales
        if let Some(saved_states) = load_monitor_state() {
            for monitor in &mut self.monitors {
                if let Some(saved_state) = saved_states.iter().find(|s| s.name == monitor.name) {
                    if let Some(pos) = &saved_state.position {
                        monitor.position = Some(pos.clone());
                    }
                    if let Some(scale) = saved_state.scale {
                        monitor.scale = Some(scale);
                    }
                    if let Some(workspace) = saved_state.workspace {
                        monitor.workspace = Some(workspace);
                    }
                }
            }
        }

        self.selected_resolution = 0;
        self.selected_monitor = 0;
        self.config = Configuration::get();

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.show_help {
            match key_event.code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('K') | KeyCode::Char('k') => {
                    self.show_help = false
                }
                _ => {}
            }
            return;
        }

        if let Some(ref _popup) = self.show_popup {
            match key_event.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char(' ') => {
                    self.show_popup = None
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.show_popup = None;
                    self.write();
                }
                _ => {}
            }
            return;
        }

        if let Some(ref mut preset_menu) = self.show_preset_menu {
            let event = preset_menu.handle_event(key_event.code);
            match event {
                crate::ui::components::MenuEvent::Action(action) => {
                    match action {
                        crate::ui::components::PresetAction::Create(name) => {
                            match self.create_preset(&name) {
                                Ok(()) => self.show_preset_menu = None,
                                Err(err_text) => {
                                    if let Some(menu) = self.show_preset_menu.as_mut() {
                                        menu.set_error(err_text);
                                    }
                                }
                            }
                        }
                        crate::ui::components::PresetAction::Delete(name) => {
                            match self.delete_preset(&name) {
                                Ok(()) => self.show_preset_menu = None,
                                Err(err_text) => {
                                    if let Some(menu) = self.show_preset_menu.as_mut() {
                                        menu.set_error(err_text);
                                    }
                                }
                            }
                        }
                        crate::ui::components::PresetAction::Rename(old, new) => {
                            match self.rename_preset(&old, &new) {
                                Ok(()) => self.show_preset_menu = None,
                                Err(err_text) => {
                                    if let Some(menu) = self.show_preset_menu.as_mut() {
                                        menu.set_error(err_text);
                                    }
                                }
                            }
                        }
                    }
                }
                crate::ui::components::MenuEvent::Handled => {}
                crate::ui::components::MenuEvent::Ignored => {
                    if key_event.code == KeyCode::Char('q')
                        || key_event.code == KeyCode::Char('Q')
                        || key_event.code == KeyCode::Esc
                    {
                        self.show_preset_menu = None;
                    }
                }
            }
            return;
        }

        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('w') => match self.validate() {
                Ok(_) => self.write(),
                Err(errs) => {
                    self.show_popup = Some(Popup {
                        title: " Error ".to_string(),
                        lines: errs,
                        is_error: true,
                    })
                }
            },
            KeyCode::Char('K') if self.mode != TUIMode::Move => self.show_help = true,
            _ => match self.mode {
                TUIMode::View => {
                    if key_event.code == KeyCode::Char('p') {
                        self.show_preset_menu = Some(crate::ui::components::PresetMenu::new(crate::config::list_presets()));
                    } else {
                        MonitorList::handle_events(self, key_event)
                    }
                },
                TUIMode::Move => Map::handle_events(self, key_event),
                TUIMode::Resolution => Resolutions::handle_events(self, key_event),
                TUIMode::Scale => Scale::handle_events(self, key_event),
            },
        }
    }

    fn exit(&mut self) {
        if let Err(e) = save_monitor_state(&self.monitors) {
            eprintln!("Warning: Failed to save monitor state on exit: {}", e);
        }
        self.exit = true;
    }

    fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut ws_counts = std::collections::HashMap::new();
        for m in &self.monitors {
            if let Some(ws) = m.workspace {
                *ws_counts.entry(ws).or_insert(0) += 1;
            }
        }

        let mut duplicated_ws = Vec::new();
        for (ws, count) in ws_counts {
            if count > 1 {
                duplicated_ws.push(ws.to_string());
            }
        }
        if !duplicated_ws.is_empty() {
            errors.push(format!("Duplicate workspace assignment detected: {}", duplicated_ws.join(", ")));
        }

        let enabled_indices: Vec<usize> = self
            .monitors
            .iter()
            .enumerate()
            .filter(|(_, m)| m.enabled)
            .map(|(i, _)| i)
            .collect();

        if enabled_indices.len() > 1 {
            let mut adj = vec![vec![]; enabled_indices.len()];
            let mut geoms = Vec::new();
            for &idx in &enabled_indices {
                geoms.push(self.monitors[idx].get_geometry());
            }

            let eps = 2.0;
            for i in 0..geoms.len() {
                for j in (i + 1)..geoms.len() {
                    let (x1, y1, w1, h1) = geoms[i];
                    let (x2, y2, w2, h2) = geoms[j];

                    if x1 < x2 + w2 && x2 < x1 + w1 && y1 < y2 + h2 && y2 < y1 + h1 {
                        let name1 = &self.monitors[enabled_indices[i]].name;
                        let name2 = &self.monitors[enabled_indices[j]].name;
                        errors.push(format!("Monitors overlap: {} and {}", name1, name2));
                    }

                    let touches_x = x1 <= x2 + w2 + eps && x2 <= x1 + w1 + eps;
                    let touches_y = y1 <= y2 + h2 + eps && y2 <= y1 + h1 + eps;

                    if touches_x && touches_y {
                        adj[i].push(j);
                        adj[j].push(i);
                    }
                }
            }

            let mut components = Vec::new();
            let mut global_visited = vec![false; enabled_indices.len()];

            for i in 0..enabled_indices.len() {
                if !global_visited[i] {
                    let mut comp = Vec::new();
                    let mut q = vec![i];
                    global_visited[i] = true;

                    while let Some(node) = q.pop() {
                        comp.push(node);
                        for &neighbor in &adj[node] {
                            if !global_visited[neighbor] {
                                global_visited[neighbor] = true;
                                q.push(neighbor);
                            }
                        }
                    }
                    components.push(comp);
                }
            }

            if components.len() > 1 {
                components.sort_by_key(|a| std::cmp::Reverse(a.len()));
                let mut disconnected = Vec::new();
                for comp in components.iter().skip(1) {
                    for &idx in comp {
                        disconnected.push(self.monitors[enabled_indices[idx]].name.clone());
                    }
                }
                errors.push(format!("Monitors not contiguous. Disconnected: {}", disconnected.join(", ")));
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    fn write(&mut self) {
        let path = self
            .config
            .monitors_config_path
            .as_deref()
            .unwrap_or("~/.config/hypr/monitors.conf");
        let lua_config = self.config.lua_monitor_config.as_deref();

        if Monitor::save_hyprland_config(path, &self.monitors, lua_config).is_err() {
            let lines = vec!["Failed to save Hyprland config.".to_string()];
            self.show_popup = Some(Popup {
                title: " Error ".to_string(),
                lines,
                is_error: true,
            });
        } else {
            let _ = std::process::Command::new("hyprctl")
                .arg("reload")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }

        if let Err(e) = save_monitor_state(&self.monitors) {
            eprintln!("✗ Failed to save monitor state: {}", e);
        }
    }

    pub fn create_preset(&mut self, name: &str) -> Result<(), String> {
        crate::config::save_preset(name, &self.monitors)
            .map_err(|e| format!("Failed to save preset '{}': {}", name, e))
    }

    pub fn delete_preset(&mut self, name: &str) -> Result<(), String> {
        crate::config::delete_preset(name)
            .map_err(|e| format!("Failed to delete preset '{}': {}", name, e))
    }

    pub fn rename_preset(&mut self, old_name: &str, new_name: &str) -> Result<(), String> {
        crate::config::rename_preset(old_name, new_name)
            .map_err(|e| format!("Failed to rename preset: {}", e))
    }
}
