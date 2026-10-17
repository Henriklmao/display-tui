//! Application state and core logic.
//!
//! Contains the main App struct, its methods, and the Popup type.

use crate::CliAction;
use crate::actions::movement::SnapGuides;
use crate::config::{Configuration, load_monitor_state, save_monitor_state_in};
use crate::monitor::Monitor;
use crate::ui::components::{Map, MonitorList, PresetMenu, Resolutions, Scale};
use crate::ui::layouts::ScreenLayout;
use crate::utils::TUIMode;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, MouseEvent};
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
    // Rectangles of the last rendered frame, used to hit-test mouse events.
    pub layout: Option<ScreenLayout>,
    // An in-progress mouse drag of a monitor on the map, if any.
    pub drag: Option<DragState>,
    // Monitor whose position changed and still needs an explicit apply. It is
    // the monitor that shows the on-map Apply button.
    pub moved_monitor: Option<usize>,
    // Pre-move position of every monitor with an unapplied move, keyed by
    // index. Cancel/Revert restores each of them; Apply drops the record.
    pub move_origins: std::collections::HashMap<usize, (i32, i32)>,
    // Last left-click cell and time, for double-click detection.
    pub last_click: Option<(u16, u16, std::time::Instant)>,
    // Overrides where presets are read/written: production uses the real
    // config directory; tests point it at a temp dir.
    pub presets_dir: Option<std::path::PathBuf>,
    // Overrides where the monitor state is persisted: mode changes, exit and
    // preset apply all honour it. None (production) uses the real config path;
    // tests point it at a temp dir so those paths can be exercised.
    pub state_path: Option<std::path::PathBuf>,
}

// An in-progress drag of a monitor on the map.
pub struct DragState {
    // Index of the dragged monitor.
    pub monitor: usize,
    // World-space point where the drag started.
    pub grab: (f64, f64),
    // The monitor's position when the drag started.
    pub origin: (i32, i32),
    // Alignment guides the current snap locked onto.
    pub guides: SnapGuides,
    // Canvas bounds frozen when the drag started. Using these for the whole
    // drag keeps the cursor-to-world mapping linear instead of feeding the
    // moving monitor back into its own coordinate space.
    pub canvas: crate::monitor::MonitorCanvas,
    // Whether the dragged monitor already had a pending move before this drag.
    pub was_pending: bool,
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

    // Resolves the monitor state file, honouring the injectable override.
    pub fn state_path(&self) -> std::path::PathBuf {
        self.state_path
            .clone()
            .unwrap_or_else(crate::config::get_state_path)
    }

    // Resolves the presets directory, honouring the injectable override.
    pub(crate) fn presets_dir(&self) -> std::path::PathBuf {
        self.presets_dir
            .clone()
            .unwrap_or_else(crate::config::get_presets_dir)
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
                                if let Err(e) =
                                    save_monitor_state_in(&self.state_path(), &self.monitors)
                                {
                                    eprintln!("✗ Failed to save monitor state: {}", e);
                                }
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

    fn draw(&mut self, frame: &mut Frame) {
        // Record the interactive regions so mouse events can be hit-tested
        // against exactly what was drawn.
        self.layout = Some(crate::ui::layouts::screen_layout(frame.area(), self.mode));
        frame.render_widget(&*self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            Event::Mouse(mouse_event) => self.handle_mouse_event(mouse_event),
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        crate::actions::mouse::handle_mouse(self, mouse_event);
    }

    // Dispatches one key press. `pub(crate)` so the mouse tests can drive the
    // real key path (e.g. Escape after a drag) instead of the components alone.
    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.show_help {
            match key_event.code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('K') | KeyCode::Char('k') => {
                    self.show_help = false
                }
                _ => {}
            }
            return;
        }

        if let Some(popup) = &self.show_popup {
            let is_error = popup.is_error;
            let is_forceable = popup.is_forceable;
            match key_event.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char(' ') => {
                    // Close popup without action
                    self.show_popup = None;
                }
                KeyCode::Enter => {
                    if !is_error {
                        // For non-error popups (mismatch/warning), Enter accepts and writes
                        self.write();
                    }
                    // Close the popup
                    self.show_popup = None;
                }
                KeyCode::Char('f') | KeyCode::Char('F') if is_forceable => {
                    // Force write for forceable popups
                    self.write();
                    self.show_popup = None;
                }
                _ => {
                    // For unrelated keys, keep the popup visible
                }
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

        // An in-progress drag: Escape always aborts it, so a gesture can be
        // cancelled in any mode.
        if self.drag.is_some() && key_event.code == KeyCode::Esc {
            self.cancel_drag();
            return;
        }

        // A displaced monitor (dragging or released) is a confirm state in View:
        // only Apply and Cancel/Revert act, so nothing else can change or persist
        // the layout under an unconfirmed move. Enter/Space apply even while the
        // button is still held; in the sub-modes the move keeps its record but
        // those keys apply the highlighted value instead.
        if self.mode == TUIMode::View && self.moved_monitor.is_some() {
            match key_event.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.apply_move();
                    self.drag = None;
                }
                KeyCode::Esc | KeyCode::Char('q') => self.cancel_move(),
                _ => {}
            }
            return;
        }

        match key_event.code {
            // `q` only quits in View mode; inside a sub-mode it cancels back
            // to View (handled by the mode dispatchers below).
            KeyCode::Char('q') if self.mode == TUIMode::View => self.exit(),
            KeyCode::Char('w') => match self.validate() {
                Ok(_) => self.write(),
                Err(errs) => {
                    self.show_popup = Some(Popup {
                        title: " Error ".to_string(),
                        lines: errs,
                        is_error: true,
                        apply_preset: None,
                        is_forceable: true,
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
    pub(crate) fn restore_preset_backup(&mut self) {
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
    pub(crate) fn preview_preset(&mut self, name: &str) {
        let _ = crate::config::apply_preset_in(&self.presets_dir(), name, &mut self.monitors);
    }

    // Commit the pending monitor move: persist the layout and drop the marker.
    pub(crate) fn apply_move(&mut self) {
        if let Err(e) = save_monitor_state_in(&self.state_path(), &self.monitors) {
            eprintln!("✗ Failed to save monitor state: {}", e);
        }
        self.move_origins.clear();
        self.moved_monitor = None;
        self.clear_active_preset_if_changed();
    }

    // Records that `index` has an unapplied move from `origin` and makes it the
    // monitor that shows the Apply button. A monitor already on record keeps its
    // original position, so re-dragging does not lose the revert target.
    pub(crate) fn note_move(&mut self, index: usize, origin: (i32, i32)) {
        self.move_origins.entry(index).or_insert(origin);
        self.moved_monitor = Some(index);
    }

    // Drops a monitor's move record without touching its position. Used when the
    // move can no longer be applied (e.g. the monitor was disabled).
    pub(crate) fn discard_move(&mut self, index: usize) {
        self.move_origins.remove(&index);
        self.refresh_pending();
    }

    // Keeps `moved_monitor` in sync with the recorded moves after one is
    // dropped: it just has to report whether anything is still pending, so any
    // remaining monitor does. Highest index is a stable pick, not an order.
    fn refresh_pending(&mut self) {
        self.moved_monitor = self.move_origins.keys().copied().max();
    }

    // Cancel/Revert: put every unapplied move back where it started and drop the
    // pending state, aborting an in-progress drag as well.
    pub(crate) fn cancel_move(&mut self) {
        for (index, (x, y)) in std::mem::take(&mut self.move_origins) {
            if let Some(position) = self
                .monitors
                .get_mut(index)
                .and_then(|monitor| monitor.position.as_mut())
            {
                position.x = x;
                position.y = y;
            }
        }
        self.moved_monitor = None;
        self.drag = None;
    }

    // Aborts an in-progress drag: the monitor returns to where the drag began
    // and only stays pending if it already was before the drag.
    fn cancel_drag(&mut self) {
        if let Some(drag) = self.drag.take() {
            if let Some(monitor) = self.monitors.get_mut(drag.monitor)
                && let Some(position) = monitor.position.as_mut()
            {
                position.x = drag.origin.0;
                position.y = drag.origin.1;
            }
            if !drag.was_pending {
                // The move never left the drag: drop the recorded origin too.
                self.move_origins.remove(&drag.monitor);
            }
            self.refresh_pending();
        }
    }

    // Commit a preset to the working layout from the mouse. Unlike the menu's
    // Enter this keeps the change in the working state (persisted) without
    // rewriting the Hyprland config; `w` (or Enter in the menu) writes it.
    pub(crate) fn apply_preset_click(&mut self, name: &str) {
        if crate::config::apply_preset_in(&self.presets_dir(), name, &mut self.monitors).is_err() {
            // The preset is missing or invalid: leave the menu and layout alone.
            return;
        }
        self.active_preset = Some(name.to_string());
        self.preset_backup = None;
        self.show_preset_menu = None;
        // The preset redefined the layout, so any pending move is stale.
        self.move_origins.clear();
        self.moved_monitor = None;
        if let Err(e) = save_monitor_state_in(&self.state_path(), &self.monitors) {
            eprintln!("✗ Failed to save monitor state: {}", e);
        }
    }

    fn exit(&mut self) {
        if let Err(e) = save_monitor_state_in(&self.state_path(), &self.monitors) {
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

    pub(crate) fn write(&mut self) {
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
                is_forceable: false,
            });
        } else {
            let _ = std::process::Command::new("hyprctl")
                .arg("reload")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }

        if let Err(e) = save_monitor_state_in(&self.state_path(), &self.monitors) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::Position;
    use crate::test_utils::tests::TempDir;
    use crossterm::event::KeyModifiers;

    fn monitor() -> Monitor {
        Monitor {
            name: "DP-1".to_string(),
            enabled: true,
            position: Some(Position { x: 0, y: 0 }),
            ..Default::default()
        }
    }

    // Every persistence path has to honour the override, otherwise a test
    // driving an exit path would write into the real config directory.
    #[test]
    fn exit_persists_into_the_injected_state_path() {
        let dir = TempDir::new("app-exit");
        let mut app = App::new(None);
        app.monitors = vec![monitor()];
        app.state_path = Some(dir.path().join("monitor_state.json"));

        app.exit();

        assert!(app.exit, "the app must be marked for shutdown");
        assert!(
            dir.path().join("monitor_state.json").exists(),
            "App::exit must persist through the injected state path"
        );
    }

    // Builds a View-mode app with `monitors` whose positions are as given. The
    // first monitor is left in whatever move state the caller sets up.
    fn app_with(monitors: Vec<Monitor>) -> App {
        let mut app = App::new(None);
        app.monitors = monitors;
        app
    }

    fn moved_monitor_app(current: (i32, i32), origin: (i32, i32)) -> App {
        let mut monitor = monitor();
        monitor.position = Some(Position {
            x: current.0,
            y: current.1,
        });
        let mut app = app_with(vec![monitor]);
        app.note_move(0, origin);
        app
    }

    // Builds an app mid-drag with the given origin and whether a move was
    // already pending before the drag began.
    fn app_mid_drag(current: (i32, i32), origin: (i32, i32), was_pending: bool) -> App {
        let mut app = moved_monitor_app(current, origin);
        if was_pending {
            // A drag of an already-moved monitor keeps the earlier origin.
            app.move_origins.insert(0, (100, 0));
        }
        let canvas =
            Monitor::get_monitors_canvas(&app.monitors, &ratatui::layout::Rect::new(0, 0, 100, 30));
        app.drag = Some(DragState {
            monitor: 0,
            grab: (0.0, 0.0),
            origin,
            guides: Default::default(),
            canvas,
            was_pending,
        });
        app
    }

    #[test]
    fn escape_cancels_an_in_progress_drag() {
        let mut app = app_mid_drag((250, 140), (100, 100), false);

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.drag.is_none(), "the drag is cleared");
        assert_eq!(
            app.monitors[0].position,
            Some(Position { x: 100, y: 100 }),
            "the monitor snaps back to where the drag began"
        );
        assert_eq!(app.moved_monitor, None, "the cancelled move is not pending");
        assert!(app.move_origins.is_empty(), "no move record is left behind");
    }

    #[test]
    fn escape_aborts_a_drag_but_keeps_a_move_that_was_already_pending() {
        let mut app = app_mid_drag((300, 0), (200, 0), true);

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.monitors[0].position, Some(Position { x: 200, y: 0 }));
        assert_eq!(
            app.moved_monitor,
            Some(0),
            "the earlier pending move survives"
        );
        assert_eq!(
            app.move_origins.get(&0),
            Some(&(100, 0)),
            "the original revert target is kept"
        );
    }

    #[test]
    fn escape_reverts_a_pending_move_once_the_drag_has_ended() {
        let mut app = moved_monitor_app((250, 140), (100, 100));
        assert!(app.drag.is_none(), "the drag already ended");

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(
            app.monitors[0].position,
            Some(Position { x: 100, y: 100 }),
            "Escape puts the monitor back"
        );
        assert_eq!(app.moved_monitor, None, "and drops the pending move");
        assert!(app.move_origins.is_empty());
    }

    #[test]
    fn q_reverts_a_pending_move_instead_of_quitting() {
        let mut app = moved_monitor_app((250, 140), (100, 100));

        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

        assert!(!app.exit, "q must not quit while a move is unconfirmed");
        assert_eq!(app.monitors[0].position, Some(Position { x: 100, y: 100 }));
        assert_eq!(app.moved_monitor, None);
    }

    #[test]
    fn other_keys_are_ignored_while_a_move_is_pending() {
        let mut app = moved_monitor_app((250, 140), (100, 100));

        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('m'),
            KeyCode::Char('r'),
            KeyCode::Char('o'),
            KeyCode::Char('K'),
        ] {
            app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
        }

        assert_eq!(app.mode, TUIMode::View, "no sub-mode is entered");
        assert!(!app.show_help, "the help modal is not opened");
        assert!(app.monitors[0].enabled, "the monitor is not disabled");
        assert_eq!(
            app.moved_monitor,
            Some(0),
            "the move is still the only pending action"
        );
    }

    #[test]
    fn escape_reverts_every_pending_move() {
        let mut a = monitor();
        a.position = Some(Position { x: 10, y: 20 });
        let mut b = Monitor {
            name: "DP-2".to_string(),
            ..monitor()
        };
        b.position = Some(Position { x: 900, y: 0 });
        let mut app = app_with(vec![a, b]);
        app.note_move(0, (1, 2));
        app.note_move(1, (3, 4));

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.monitors[0].position, Some(Position { x: 1, y: 2 }));
        assert_eq!(app.monitors[1].position, Some(Position { x: 3, y: 4 }));
        assert!(app.move_origins.is_empty());
        assert_eq!(app.moved_monitor, None);
    }

    #[test]
    fn enter_applies_a_pending_move_written_through_the_state_path() {
        let dir = TempDir::new("app-apply-move");
        let mut app = moved_monitor_app((250, 140), (100, 100));
        app.state_path = Some(dir.path().join("monitor_state.json"));

        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.moved_monitor, None);
        assert!(app.move_origins.is_empty());
        assert!(
            dir.path().join("monitor_state.json").exists(),
            "Enter persists the applied layout"
        );
    }

    #[test]
    fn a_sub_mode_is_not_gated_by_a_pending_move() {
        // Esc keeps its sub-mode meaning outside View; the move record survives
        // until the user is back in View to confirm or revert it.
        let mut app = moved_monitor_app((250, 140), (100, 100));
        app.mode = TUIMode::Resolution;

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.mode, TUIMode::View);
        assert_eq!(
            app.monitors[0].position,
            Some(Position { x: 250, y: 140 }),
            "the move is untouched in a sub-mode"
        );
        assert_eq!(app.moved_monitor, Some(0));
    }

    #[test]
    fn enter_applies_a_drag_that_is_still_in_progress() {
        let dir = TempDir::new("app-apply-drag");
        let mut app = app_mid_drag((250, 140), (100, 100), false);
        app.state_path = Some(dir.path().join("monitor_state.json"));

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert!(app.drag.is_none(), "Space ends the drag");
        assert_eq!(app.moved_monitor, None, "and applies the move");
        assert!(app.move_origins.is_empty());
        assert!(dir.path().join("monitor_state.json").exists());
    }

    #[test]
    fn other_keys_are_ignored_during_a_drag() {
        let mut app = app_mid_drag((250, 140), (100, 100), false);

        app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));

        assert!(app.monitors[0].enabled, "d is ignored mid-drag");
        assert_eq!(app.mode, TUIMode::View, "m is ignored mid-drag");
        assert!(app.drag.is_some(), "the drag is still active");
    }

    #[test]
    fn keys_keep_working_before_a_drag_has_moved_the_monitor() {
        // A press without any displacement is not a confirm state yet, so the
        // normal keys (and the footer that advertises them) still apply.
        let mut app = moved_monitor_app((250, 140), (100, 100));
        app.move_origins.clear();
        app.moved_monitor = None;
        let canvas =
            Monitor::get_monitors_canvas(&app.monitors, &ratatui::layout::Rect::new(0, 0, 100, 30));
        app.drag = Some(DragState {
            monitor: 0,
            grab: (0.0, 0.0),
            origin: (250, 140),
            guides: Default::default(),
            canvas,
            was_pending: false,
        });

        app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));

        assert_eq!(
            app.mode,
            TUIMode::Move,
            "an unmoved drag does not gate keys"
        );
        assert!(app.drag.is_some(), "and the drag stays active");
    }

    #[test]
    fn escape_aborts_an_unmoved_drag_too() {
        let mut app = moved_monitor_app((250, 140), (100, 100));
        app.move_origins.clear();
        app.moved_monitor = None;
        let canvas =
            Monitor::get_monitors_canvas(&app.monitors, &ratatui::layout::Rect::new(0, 0, 100, 30));
        app.drag = Some(DragState {
            monitor: 0,
            grab: (0.0, 0.0),
            origin: (250, 140),
            guides: Default::default(),
            canvas,
            was_pending: false,
        });

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.drag.is_none(), "Escape aborts the drag");
        assert_eq!(
            app.monitors[0].position,
            Some(Position { x: 250, y: 140 }),
            "the position is unchanged"
        );
        assert_eq!(app.mode, TUIMode::View);
    }

    #[test]
    fn discarding_the_primary_move_keeps_the_other_one_pending() {
        let mut a = monitor();
        a.position = Some(Position { x: 1, y: 2 });
        let mut b = Monitor {
            name: "DP-2".to_string(),
            ..monitor()
        };
        b.position = Some(Position { x: 3, y: 4 });
        let mut app = app_with(vec![a, b]);
        app.note_move(0, (10, 20));
        app.note_move(1, (30, 40));
        assert_eq!(app.moved_monitor, Some(1));

        app.discard_move(1);

        assert_eq!(
            app.moved_monitor,
            Some(0),
            "the remaining pending monitor takes over"
        );
        assert_eq!(app.move_origins.get(&0), Some(&(10, 20)));
    }
}
