//! Application state and core logic.
//!
//! Contains the main App struct, its methods, and the Popup type.

use crate::CliAction;
use crate::config::{Configuration, load_monitor_state, save_monitor_state};
use crate::monitor::Monitor;
use crate::ui::components::{Map, MonitorList, PresetMenu, Resolutions, Scale};
use crate::utils::TUIMode;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use std::io;

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
    pub preset_backup: Option<Vec<crate::config::MonitorState>>,
    pub active_preset: Option<String>,
    pub cli_action: Option<CliAction>,
}

// Popup message displayed to the user.
pub struct Popup {
    pub title: String,
    pub lines: Vec<String>,
    pub is_error: bool,
    pub apply_preset: Option<String>,
    pub is_forceable: bool,
}

impl App {
    // Create a new App with an optional CLI action.
    pub fn new(action: Option<CliAction>) -> Self {
        App {
            cli_action: action,
            ..Default::default()
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.monitors = Monitor::get_monitors();

        // Load saved monitor positions/scales
        if let Some(saved_states) = load_monitor_state() {
            for monitor in &mut self.monitors {
                if let Some(saved_state) = saved_states.iter().find(|s| s.name == monitor.name) {
                    monitor.enabled = saved_state.enabled;
                    if let Some(pos) = &saved_state.position {
                        monitor.position = Some(pos.clone());
                    }
                    if let Some(scale) = saved_state.scale {
                        monitor.scale = Some(scale);
                    }
                    if let Some(workspace) = saved_state.workspace {
                        monitor.workspace = Some(workspace);
                    }
                    monitor.transform = saved_state.rotation.clone();
                    if let Some(ref res) = saved_state.resolution
                        && let Some(idx) = monitor.modes.iter().position(|m| {
                            m.width == res.width
                                && m.height == res.height
                                && (m.refresh - res.refresh_rate).abs() < 0.1
                        })
                    {
                        monitor.set_current_resolution(idx);
                    }
                }
            }
        }

        self.selected_resolution = 0;
        self.selected_monitor = 0;
        self.config = Configuration::get();

        // Handle CLI actions before entering the main loop.
        let cli_action = self.cli_action.take();
        match cli_action {
            Some(CliAction::LoadPreset(ref name)) => {
                self.handle_cli_preset_load(name);
            }
            Some(CliAction::OpenPresetMenu) => {
                self.handle_cli_open_preset_menu();
            }
            _ => {}
        }

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    // CLI: load, validate, and apply a preset. On failure, show an error
    // popup and fall through to the normal TUI so the user can fix things.
    fn handle_cli_preset_load(&mut self, name: &str) {
        // Validate preset exists and name is valid
        match crate::config::validate_preset_monitors_match(name, &self.monitors) {
            Err(missing) => {
                // Apply the preset anyway but show mismatch popup
                let _ = crate::config::save_state_as_last(&self.monitors);
                let _ = crate::config::apply_preset(name, &mut self.monitors);
                self.active_preset = Some(name.to_string());

                let mut lines = vec![
                    "Preset does not match connected monitors.".to_string(),
                    "".to_string(),
                    "Missing monitors:".to_string(),
                ];
                for m in &missing {
                    lines.push(format!("  • {}", m));
                }
                lines.push("".to_string());
                lines.push("<Enter> Accept anyway, or <Esc> Cancel".to_string());
                self.show_popup = Some(Popup {
                    title: " Preset Mismatch ".to_string(),
                    lines,
                    is_error: false,
                    apply_preset: None,
                    is_forceable: true,
                });
            }
            Ok(()) => {
                // Check for zero-monitor preset
                if crate::config::count_enabled_monitors_in_preset(name) == Some(0) {
                    let _ = crate::config::apply_preset(name, &mut self.monitors);
                    self.active_preset = Some(name.to_string());
                    self.show_popup = Some(Popup {
                        title: " Preset Warning ".to_string(),
                        lines: vec![
                            "This preset has 0 enabled monitors.".to_string(),
                            "".to_string(),
                            "<Enter> Accept".to_string(),
                        ],
                        is_error: false,
                        apply_preset: None,
                        is_forceable: false,
                    });
                } else {
                    // Apply and write
                    let _ = crate::config::save_state_as_last(&self.monitors);
                    match crate::config::apply_preset(name, &mut self.monitors) {
                        Ok(()) => {
                            self.active_preset = Some(name.to_string());
                            self.write();
                            // If write succeeded (no popup shown), exit.
                            if self.show_popup.is_none() {
                                let _ = save_monitor_state(&self.monitors);
                                self.exit = true;
                            }
                        }
                        Err(err_text) => {
                            self.show_popup = Some(Popup {
                                title: " Error ".to_string(),
                                lines: vec![err_text],
                                is_error: true,
                                apply_preset: None,
                                is_forceable: false,
                            });
                        }
                    }
                }
            }
        }
    }

    // CLI: open the preset menu immediately on startup.
    fn handle_cli_open_preset_menu(&mut self) {
        self.save_preset_backup();
        self.show_preset_menu = Some(crate::ui::components::PresetMenu::new(
            crate::config::list_presets(),
            &self
                .monitors
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<String>>(),
            self.active_preset.clone(),
        ));
        // Preview the first preset.
        let first_preset = self
            .show_preset_menu
            .as_ref()
            .and_then(|menu| menu.presets.first().map(|entry| entry.name.clone()));
        if let Some(name) = first_preset {
            self.preview_preset(&name);
        }
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

        if let Some(popup) = self.show_popup.take() {
            match key_event.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char(' ') => {
                    // Close popup without action
                }
                KeyCode::Enter => {
                    if let Some(preset_name) = popup.apply_preset {
                        // Apply preset to monitor_state only (don't write)
                        let _ = crate::config::apply_preset(&preset_name, &mut self.monitors);
                    }
                }
                KeyCode::Char('f') | KeyCode::Char('F') if popup.is_error => {
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
                    // Drop the borrow by extracting what we need, then act
                    let action_to_take = Some(action);
                    let _ = preset_menu;

                    if let Some(action) = action_to_take {
                        match action {
                            crate::ui::components::PresetAction::Create(name) => {
                                // Save the pre-menu state, not the live-preview values
                                self.restore_preset_backup();
                                let result = self.create_preset(&name);
                                if let Some(menu) = self.show_preset_menu.as_mut() {
                                    match result {
                                        Ok(()) => {
                                            *menu = crate::ui::components::PresetMenu::new(
                                                crate::config::list_presets(),
                                                &self
                                                    .monitors
                                                    .iter()
                                                    .map(|m| m.name.clone())
                                                    .collect::<Vec<String>>(),
                                                self.active_preset.clone(),
                                            );
                                        }
                                        Err(err_text) => menu.set_error(err_text),
                                    }
                                }
                                // Re-save backup and preview first preset
                                self.save_preset_backup();
                                if let Some(entry) = self
                                    .show_preset_menu
                                    .as_ref()
                                    .and_then(|menu| menu.presets.first())
                                {
                                    let name = entry.name.clone();
                                    self.preview_preset(&name);
                                }
                            }
                            crate::ui::components::PresetAction::Delete(name) => {
                                let result = self.delete_preset(&name);
                                if let Some(menu) = self.show_preset_menu.as_mut() {
                                    match result {
                                        Ok(()) => {
                                            *menu = crate::ui::components::PresetMenu::new(
                                                crate::config::list_presets(),
                                                &self
                                                    .monitors
                                                    .iter()
                                                    .map(|m| m.name.clone())
                                                    .collect::<Vec<String>>(),
                                                self.active_preset.clone(),
                                            )
                                        }
                                        Err(err_text) => menu.set_error(err_text),
                                    }
                                }
                            }
                            crate::ui::components::PresetAction::Rename(old, new) => {
                                let result = self.rename_preset(&old, &new);
                                if let Some(menu) = self.show_preset_menu.as_mut() {
                                    match result {
                                        Ok(()) => {
                                            *menu = crate::ui::components::PresetMenu::new(
                                                crate::config::list_presets(),
                                                &self
                                                    .monitors
                                                    .iter()
                                                    .map(|m| m.name.clone())
                                                    .collect::<Vec<String>>(),
                                                self.active_preset.clone(),
                                            )
                                        }
                                        Err(err_text) => menu.set_error(err_text),
                                    }
                                }
                            }
                            crate::ui::components::PresetAction::Apply(name) => {
                                // Validate hardware match first
                                match crate::config::validate_preset_monitors_match(
                                    &name,
                                    &self.monitors,
                                ) {
                                    Err(missing) => {
                                        // Mismatch: apply into monitor_state, but NO write
                                        let _ = crate::config::save_state_as_last(&self.monitors);
                                        let _ =
                                            crate::config::apply_preset(&name, &mut self.monitors);
                                        // State change is intentional, so no restore.
                                        self.preset_backup = None;
                                        self.show_preset_menu = None;
                                        let mut popup_lines = vec![
                                            "Preset does not match connected monitors.".to_string(),
                                            "".to_string(),
                                            "Missing monitors:".to_string(),
                                        ];
                                        for m in &missing {
                                            popup_lines.push(format!("  \u{2022} {}", m));
                                        }
                                        popup_lines.push("".to_string());
                                        popup_lines.push(
                                            "<Enter> Accept anyway, or <Esc> Cancel".to_string(),
                                        );

                                        self.show_popup = Some(Popup {
                                            title: " Preset Mismatch ".to_string(),
                                            lines: popup_lines,
                                            is_error: false, // Not a hard error
                                            apply_preset: Some(name),
                                            is_forceable: true,
                                        });
                                    }
                                    Ok(()) => {
                                        // Warn for presets with 0 enabled monitors but allow applying (state only, no write)
                                        if crate::config::count_enabled_monitors_in_preset(&name)
                                            == Some(0)
                                        {
                                            // Apply preset state but don't write
                                            let _ = crate::config::apply_preset(
                                                &name,
                                                &mut self.monitors,
                                            );
                                            self.active_preset = Some(name.clone());
                                            self.preset_backup = None;
                                            self.show_preset_menu = None;
                                            // Show warning popup with only <Enter> Accept
                                            self.show_popup = Some(Popup {
                                                title: " Preset Warning ".to_string(),
                                                lines: vec![
                                                    "This preset has 0 enabled monitors."
                                                        .to_string(),
                                                    "".to_string(),
                                                    "<Enter> Accept".to_string(),
                                                ],
                                                is_error: false,
                                                apply_preset: None,
                                                is_forceable: false,
                                            });
                                        } else {
                                            let _ =
                                                crate::config::save_state_as_last(&self.monitors);
                                            match crate::config::apply_preset(
                                                &name,
                                                &mut self.monitors,
                                            ) {
                                                Ok(()) => {
                                                    // Successful apply: don't restore the backup.
                                                    self.active_preset = Some(name.clone());
                                                    self.preset_backup = None;
                                                    self.show_preset_menu = None;
                                                    self.write();
                                                }
                                                Err(err_text) => {
                                                    if let Some(menu) =
                                                        self.show_preset_menu.as_mut()
                                                    {
                                                        menu.set_error(err_text);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            crate::ui::components::PresetAction::Override(name) => {
                                // Restore pre-menu state so override saves the real
                                // monitor_state, not the live-preview values.
                                self.restore_preset_backup();
                                let result = crate::config::override_preset(&name, &self.monitors);
                                if let Some(menu) = self.show_preset_menu.as_mut() {
                                    match result {
                                        Ok(()) => {
                                            *menu = crate::ui::components::PresetMenu::new(
                                                crate::config::list_presets(),
                                                &self
                                                    .monitors
                                                    .iter()
                                                    .map(|m| m.name.clone())
                                                    .collect::<Vec<String>>(),
                                                self.active_preset.clone(),
                                            );
                                        }
                                        Err(err_text) => menu.set_error(err_text),
                                    }
                                }
                                // Re-save backup and preview first preset (the
                                // menu borrow above has ended, so self methods
                                // are safe to call here).
                                self.save_preset_backup();
                                if let Some(entry) = self
                                    .show_preset_menu
                                    .as_ref()
                                    .and_then(|menu| menu.presets.first())
                                {
                                    let name = entry.name.clone();
                                    self.preview_preset(&name);
                                }
                            }
                        }
                    }
                }
                crate::ui::components::MenuEvent::Preview(name) => {
                    // Live preview: render the preset in display/map in real time.
                    self.preview_preset(&name);
                }
                crate::ui::components::MenuEvent::Handled => {}
                crate::ui::components::MenuEvent::Ignored => {
                    if key_event.code == KeyCode::Char('q')
                        || key_event.code == KeyCode::Char('Q')
                        || key_event.code == KeyCode::Esc
                    {
                        // Closing without apply: restore the original state.
                        self.restore_preset_backup();
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
                        apply_preset: None,
                        is_forceable: false,
                    })
                }
            },
            KeyCode::Char('K') if self.mode != TUIMode::Move => self.show_help = true,
            _ => match self.mode {
                TUIMode::View => {
                    if key_event.code == KeyCode::Char('p') || key_event.code == KeyCode::Char('P')
                    {
                        self.save_preset_backup();
                        self.show_preset_menu = Some(crate::ui::components::PresetMenu::new(
                            crate::config::list_presets(),
                            &self
                                .monitors
                                .iter()
                                .map(|m| m.name.clone())
                                .collect::<Vec<String>>(),
                            self.active_preset.clone(),
                        ));
                        // Preview the first preset immediately.
                        let first_preset = self
                            .show_preset_menu
                            .as_ref()
                            .and_then(|menu| menu.presets.first().map(|entry| entry.name.clone()));
                        if let Some(name) = first_preset {
                            self.preview_preset(&name);
                        }
                    } else {
                        MonitorList::handle_events(self, key_event);
                        self.clear_active_preset_if_changed();
                    }
                }
                TUIMode::Move => {
                    Map::handle_events(self, key_event);
                    self.clear_active_preset_if_changed();
                }
                TUIMode::Resolution => {
                    Resolutions::handle_events(self, key_event);
                    self.clear_active_preset_if_changed();
                }
                TUIMode::Scale => {
                    Scale::handle_events(self, key_event);
                    self.clear_active_preset_if_changed();
                }
            },
        }
    }

    // Snapshot the current monitor state so it can be restored if the
    // preset menu is closed without applying.
    fn save_preset_backup(&mut self) {
        self.preset_backup = Some(
            self.monitors
                .iter()
                .map(|m| {
                    let resolution =
                        m.get_current_resolution()
                            .map(|r| crate::config::ResolutionState {
                                width: r.width,
                                height: r.height,
                                refresh_rate: r.refresh,
                            });
                    crate::config::MonitorState {
                        name: m.name.clone(),
                        enabled: m.enabled,
                        position: m.position.clone(),
                        scale: m.scale,
                        workspace: m.workspace,
                        rotation: m.transform.clone(),
                        resolution,
                    }
                })
                .collect(),
        );
    }

    // Restore the monitor state captured when the preset menu was opened.
    fn restore_preset_backup(&mut self) {
        if let Some(backup) = self.preset_backup.take() {
            for state in backup {
                if let Some(monitor) = self.monitors.iter_mut().find(|m| m.name == state.name) {
                    monitor.enabled = state.enabled;
                    monitor.position = state.position;
                    monitor.scale = state.scale;
                    monitor.workspace = state.workspace;
                    monitor.transform = state.rotation;
                    if let Some(ref res) = state.resolution
                        && let Some(idx) = monitor.modes.iter().position(|m| {
                            m.width == res.width
                                && m.height == res.height
                                && (m.refresh - res.refresh_rate).abs() < 0.1
                        })
                    {
                        monitor.set_current_resolution(idx);
                    }
                }
            }
        }
    }

    // Clear the active preset marker as soon as the current monitor state
    // diverges from what the active preset defines (or the preset file is gone).
    fn clear_active_preset_if_changed(&mut self) {
        if let Some(ref active) = self.active_preset.clone() {
            if let Some(state) = crate::config::load_preset(active) {
                for monitor_state in &state {
                    if let Some(monitor) =
                        self.monitors.iter().find(|m| m.name == monitor_state.name)
                    {
                        let current_res = monitor.get_current_resolution();
                        let resolution_changed = match (&monitor_state.resolution, current_res) {
                            (Some(pr), Some(cr)) => {
                                pr.width != cr.width
                                    || pr.height != cr.height
                                    || (pr.refresh_rate - cr.refresh).abs() >= 0.1
                            }
                            (None, None) => false,
                            _ => true,
                        };
                        let changed = monitor.position != monitor_state.position
                            || monitor.scale != monitor_state.scale
                            || monitor.workspace != monitor_state.workspace
                            || monitor.enabled != monitor_state.enabled
                            || monitor.transform != monitor_state.rotation
                            || resolution_changed;
                        if changed {
                            self.active_preset = None;
                            return;
                        }
                    }
                }
            } else {
                // Preset file gone
                self.active_preset = None;
            }
        }
    }

    // Apply a preset as a live preview only; no save_state_as_last (that is
    // only done on the final apply).
    fn preview_preset(&mut self, name: &str) {
        let _ = crate::config::apply_preset(name, &mut self.monitors);
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
            errors.push(format!(
                "Duplicate workspace assignment detected: {}",
                duplicated_ws.join(", ")
            ));
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
                errors.push(format!(
                    "Monitors not contiguous. Disconnected: {}",
                    disconnected.join(", ")
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn write(&mut self) {
        // Check that at least one monitor is enabled (not forceable)
        if !self.monitors.iter().any(|m| m.enabled) {
            self.show_popup = Some(Popup {
                title: " Error ".to_string(),
                lines: vec!["At least one monitor must be enabled.".to_string()],
                is_error: true,
                apply_preset: None,
                is_forceable: false,
            });
            return;
        }

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
                apply_preset: None,
                is_forceable: true,
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
