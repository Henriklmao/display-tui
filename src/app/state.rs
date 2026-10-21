//! Application state and core logic.
//!
//! Contains the main App struct, its methods, and the Popup type.

use crate::CliAction;
use crate::actions::movement::SnapGuides;
use crate::config::{Configuration, load_monitor_state, save_monitor_state_in};
use crate::monitor::Monitor;
use crate::ui::components::{
    ActionId, HelpModal, ListColumn, Map, MonitorList, PopupAction, PresetButtonAction, PresetMenu,
    Resolutions, Scale,
};
use crate::ui::layouts::ScreenLayout;
use crate::utils::TUIMode;
use crate::validation::diagnostics::{AutoFix, LayoutHighlights, layout_highlights, plan_auto_fix};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::layout::Rect;
use std::io;
use std::time::{Duration, Instant};

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
    // First visible content row of the help modal; reset when it opens.
    pub help_scroll: u16,
    // An in-progress drag of the help scrollbar, if any.
    pub help_scroll_drag: Option<HelpScrollbarDrag>,
    pub show_popup: Option<Popup>,
    pub show_preset_menu: Option<PresetMenu>,
    pub preset_backup: Option<Vec<crate::config::MonitorState>>,
    pub active_preset: Option<String>,
    pub cli_action: Option<CliAction>,
    // Rectangles of the last rendered frame, used to hit-test mouse events.
    pub layout: Option<ScreenLayout>,
    // World-space highlights (overlap edges, detached borders, connectors)
    // armed by the layout check and cleared on cancel/success.
    pub highlights: Option<LayoutHighlights>,
    // The running validation blink, if the highlights are animating.
    pub blink: Option<BlinkState>,
    // The interactive element currently under the pointer, if any.
    pub hover: Option<HoverTarget>,
    // The selection to restore when a map hover ends, plus the monitor the
    // hover selected: (committed selection, hovered monitor). If the selection
    // moves elsewhere while hovering (keyboard), that becomes the committed
    // selection instead.
    pub hover_restore: Option<(usize, usize)>,
    // When the current hover began: the tooltip dwell clock.
    pub hover_since: Option<Instant>,
    // The last pointer cell, used to place the tooltip.
    pub hover_point: (u16, u16),
    // The tooltip to draw this frame, computed in `draw`.
    pub tooltip: Option<crate::ui::tooltip::TooltipState>,
    // The workspace picker opened from a display row's workspace cell.
    pub workspace_picker: Option<WorkspacePickerState>,
    // An in-progress mouse drag of a monitor on the map, if any.
    pub drag: Option<DragState>,
    // Pre-move position of every monitor with an unapplied move, keyed by
    // index. It is the single source of truth for the confirm state: Cancel/
    // Revert restores each of them, Apply drops the record.
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

// An in-progress drag of the help scrollbar: the row offset below the thumb's
// top edge where the pointer grabbed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpScrollbarDrag {
    pub grab: u16,
}

// The workspace picker opened for one display row. The anchor is recomputed
// from the current list layout each frame, so a resize or scroll cannot leave
// the popover detached from its cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspacePickerState {
    pub monitor: usize,
}

// An interactive element the pointer can rest on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverTarget {
    // A monitor body on the map.
    Map(usize),
    // The Apply button of a pending monitor.
    ApplyButton(usize),
    // A display-list row (name, description or position cell).
    ListRow(usize),
    // The enable/disable checkbox cell of a display row.
    ListIcon(usize),
    // The resolution cell of a display row.
    ListResolution(usize),
    // The scale cell of a display row.
    ListScale(usize),
    // The rotation cell of a display row.
    ListRotation(usize),
    // The workspace cell of a display row.
    ListWorkspace(usize),
    // A digit in the workspace picker; 0 clears the assignment.
    WorkspaceDigit(u8),
    // A resolution-table row.
    ResolutionRow(usize),
    // A scale-table row.
    ScaleRow(usize),
    // A preset-menu row.
    PresetRow(usize),
    // A button in the preset menu's footer.
    PresetButton(PresetButtonAction),
    // A popup footer button.
    Popup(PopupAction),
    // The close button embedded in the help modal's footer.
    HelpClose,
    // The draggable scrollbar inside the help modal.
    HelpScrollbar,
    // A button in the display list's action bar.
    Action(ActionId),
}

// How long one blink phase stays on screen.
pub const BLINK_TOGGLE: Duration = Duration::from_millis(500);

// Number of on/off toggles in one validation blink (eight = four flashes).
pub const BLINK_TOGGLES: u32 = 8;

// A finite validation blink. The phase is a counter, not wall-clock derived,
// so rendering and tests are deterministic; `since` only drives scheduling.
pub struct BlinkState {
    // Remaining toggles before the blink settles red and disarms.
    pub toggles_left: u32,
    // Whether the highlights are painted hot right now.
    pub lit: bool,
    // When the current phase began.
    pub since: Instant,
}

impl BlinkState {
    // Starts a blink in the lit phase at `now`.
    pub fn start(now: Instant) -> Self {
        BlinkState {
            toggles_left: BLINK_TOGGLES,
            lit: true,
            since: now,
        }
    }

    // Time until the next phase toggle.
    pub fn remaining(&self, now: Instant) -> Duration {
        BLINK_TOGGLE.saturating_sub(now.saturating_duration_since(self.since))
    }
}

// Popup message displayed to the user.
pub struct Popup {
    pub title: String,
    pub lines: Vec<String>,
    pub is_error: bool,
    pub apply_preset: Option<String>,
    pub is_forceable: bool,
    // A single move that resolves the reported problem, offered on Space.
    pub fix: Option<AutoFix>,
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

    // Records the hovered element. Hovering a map monitor temporarily moves the
    // selection so the list, side table and keyboard actions all follow the
    // pointer; the previous selection is restored when the pointer leaves the
    // map (`in_map` is whether it is still over the canvas), and a click
    // commits the hovered monitor through the normal click path.
    pub(crate) fn set_hover(
        &mut self,
        target: Option<HoverTarget>,
        point: (u16, u16),
        now: Instant,
        in_map: bool,
    ) {
        self.hover_point = point;
        let changed = self.hover != target;
        if changed {
            self.hover = target;
            self.hover_since = Some(now);
        }
        // Sync on a target change, and whenever the pointer is outside the map
        // so that leaving the map with no hovered element still restores the
        // previous selection.
        if changed || !in_map {
            self.sync_hover_selection(in_map);
        }
    }

    // Moves the selection to the hovered monitor, or restores the stored one.
    fn sync_hover_selection(&mut self, in_map: bool) {
        match self.hover {
            Some(HoverTarget::Map(index)) if index < self.monitors.len() => {
                // A selection moved elsewhere while hovering (e.g. with j/k) is
                // the new committed selection, not the value to restore.
                let committed = match self.hover_restore {
                    Some((previous, hovered)) if self.selected_monitor == hovered => previous,
                    _ => self.selected_monitor,
                };
                self.hover_restore = Some((committed, index));
                self.selected_monitor = index;
            }
            // Still over the canvas but not on a monitor: keep the temporary
            // selection until the pointer actually leaves the map.
            None if in_map => {}
            _ => {
                if let Some((previous, hovered)) = self.hover_restore.take()
                    && self.selected_monitor == hovered
                {
                    self.selected_monitor = previous;
                }
            }
        }
    }

    // Whether any monitor has an unapplied move. Drives the confirm state and
    // the on-map Apply buttons; derived from `move_origins` so it cannot drift.
    pub(crate) fn has_pending_move(&self) -> bool {
        !self.move_origins.is_empty()
    }

    // Opens the workspace picker for a display row and selects that row.
    pub(crate) fn open_workspace_picker(&mut self, monitor: usize) {
        self.selected_monitor = monitor;
        self.workspace_picker = Some(WorkspacePickerState { monitor });
        self.clear_hover();
    }

    pub(crate) fn close_workspace_picker(&mut self) {
        self.workspace_picker = None;
    }

    // The screen cell the picker points at, recomputed from the live layout.
    pub(crate) fn workspace_picker_anchor(&self, layout: ScreenLayout) -> Option<Rect> {
        let picker = self.workspace_picker?;
        MonitorList::cell_rect(
            layout.list,
            &self.monitors,
            self.selected_monitor,
            picker.monitor,
            ListColumn::Workspace,
        )
    }

    // Assigns a workspace to the selected monitor, using the same path as the
    // digit keys so the active-preset marker is cleared consistently.
    pub(crate) fn assign_workspace(&mut self, workspace: u8) {
        MonitorList::set_workspace(self, workspace);
        self.clear_active_preset_if_changed();
    }

    // Whether the armed map highlights are currently painted hot. The blink
    // counter gates this; an un-armed layout never draws anything, and once the
    // blink has settled it stays red until the popup is dismissed.
    pub fn highlight_visible(&self) -> bool {
        self.highlights.is_some() && self.blink.as_ref().is_none_or(|blink| blink.lit)
    }

    // Starts a fresh validation blink at `now`, keeping any armed highlights.
    pub fn arm_blink(&mut self, now: Instant) {
        self.blink = Some(BlinkState::start(now));
    }

    // Stops any running blink and drops the highlights.
    pub fn clear_highlights(&mut self) {
        self.blink = None;
        self.highlights = None;
    }

    // Time until the next animation deadline. `None` means nothing animates, so
    // the event loop blocks until real input (zero idle cost). Only validation
    // errors animate; hover is a steady recolour, but the tooltip dwell still
    // needs a wake-up so it appears without any further input.
    pub(crate) fn next_deadline_at(&self, now: Instant) -> Option<Duration> {
        let blink = self.blink.as_ref().map(|blink| blink.remaining(now));
        match (blink, self.tooltip_deadline_at(now)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    // Whether the current layout still draws a help scrollbar.
    fn help_scrollbar_available(&self) -> bool {
        self.layout
            .is_some_and(|layout| HelpModal::scrollbar_lane(layout.area).width > 0)
    }

    // A resize can remove the scrollbar under a resting pointer; drop the
    // hover so a stale tooltip does not survive while there is nothing to hover.
    fn drop_unavailable_help_hover(&mut self) {
        if self.show_help
            && self.hover == Some(HoverTarget::HelpScrollbar)
            && !self.help_scrollbar_available()
        {
            self.hover = None;
            self.hover_since = None;
        }
    }

    // Time until the next tooltip change: the dwell elapsing or the next
    // fade-in step. `None` once the tooltip is fully visible.
    fn tooltip_deadline_at(&self, now: Instant) -> Option<Duration> {
        use crate::ui::tooltip::{TOOLTIP_DELAY, TOOLTIP_FADE};

        let _ = self.hover?;
        let since = self.hover_since?;
        let elapsed = now.saturating_duration_since(since);
        if elapsed < TOOLTIP_DELAY {
            return Some(TOOLTIP_DELAY - elapsed);
        }

        let into = elapsed - TOOLTIP_DELAY;
        let fade_ms = TOOLTIP_FADE.as_millis() as u64;
        let step = into.as_millis() as u64 / fade_ms;
        if step >= 3 {
            return None;
        }
        let phase = into.as_millis() as u64 % fade_ms;
        Some(TOOLTIP_FADE - Duration::from_millis(phase))
    }

    fn next_deadline(&self) -> Option<Duration> {
        self.next_deadline_at(Instant::now())
    }

    // Advances every armed timer by one step if its deadline has passed. Only
    // called when `poll` timed out, so real input always takes priority.
    pub(crate) fn advance_animation_at(&mut self, now: Instant) {
        let mut disarm = false;
        if let Some(blink) = self.blink.as_mut()
            && now.saturating_duration_since(blink.since) >= BLINK_TOGGLE
        {
            blink.since = now;
            blink.toggles_left = blink.toggles_left.saturating_sub(1);
            blink.lit = !blink.lit;
            // The blink has run its course; the red highlights stay armed.
            disarm = blink.toggles_left == 0;
        }
        if disarm {
            self.blink = None;
        }
    }

    // The tooltip for the current hover, once the pointer has rested long
    // enough. Injected `now` keeps the dwell math testable.
    pub(crate) fn tooltip_at(&self, now: Instant) -> Option<crate::ui::tooltip::TooltipState> {
        use crate::ui::tooltip::{TOOLTIP_DELAY, TOOLTIP_FADE, text_for};

        let target = self.hover?;
        let since = self.hover_since?;
        let elapsed = now.saturating_duration_since(since);
        if elapsed < TOOLTIP_DELAY {
            return None;
        }
        let step =
            (((elapsed - TOOLTIP_DELAY).as_millis() / TOOLTIP_FADE.as_millis()) as u8).min(3);
        Some(crate::ui::tooltip::TooltipState {
            text: text_for(target),
            step,
            anchor: self.hover_point,
        })
    }

    // Whether a modal owns the screen. Tooltips must not float above one.
    fn has_open_overlay(&self) -> bool {
        self.show_help
            || self.show_popup.is_some()
            || self.show_preset_menu.is_some()
            || self.workspace_picker.is_some()
    }

    // The tooltip the next frame should draw. Suppressed under any overlay, so
    // a stale hover cannot surface over a modal opened from the keyboard.
    fn tooltip_for_draw(&self, now: Instant) -> Option<crate::ui::tooltip::TooltipState> {
        if self.has_open_overlay() {
            None
        } else {
            self.tooltip_at(now)
        }
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
            // Deadline-driven: a pending animation arms `poll`, real input
            // returns immediately and always wins. With nothing animating this
            // falls back to the old blocking read (zero idle cost).
            match self.next_deadline() {
                Some(timeout) => {
                    if event::poll(timeout)? {
                        self.handle_events()?;
                    } else {
                        self.advance_animation_at(Instant::now());
                    }
                }
                None => self.handle_events()?,
            }
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
                    fix: None,
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
                        fix: None,
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
                                fix: None,
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
        // against exactly what was drawn. The popup's content-sized frame and
        // button rects are computed from the same function the renderer uses.
        let area = frame.area();
        let mut layout = crate::ui::layouts::screen_layout(area, self.mode);
        layout.popup = self.show_popup.as_ref().map(|popup| popup.layout(area));
        self.layout = Some(layout);
        // A resize can drop the workspace column or scroll the row away. Close
        // the picker instead of leaving an invisible modal that swallows input.
        if self.workspace_picker.is_some() && self.workspace_picker_anchor(layout).is_none() {
            self.workspace_picker = None;
            self.clear_hover();
        }
        // The tooltip is derived at draw time.
        let now = Instant::now();
        self.drop_unavailable_help_hover();
        self.tooltip = self.tooltip_for_draw(now);
        frame.render_widget(&*self, area);
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

    // Runs a footer button's action through the real key path, so a click and
    // the advertised key can never diverge.
    pub(crate) fn perform_action(&mut self, action: ActionId) {
        let enabled = self
            .monitors
            .get(self.selected_monitor)
            .is_some_and(|monitor| monitor.enabled);
        self.handle_key_event(KeyEvent::new(action.key(enabled), KeyModifiers::NONE));
    }

    // Runs a preset-menu button through the menu's own key handler.
    pub(crate) fn perform_preset_button(&mut self, action: PresetButtonAction) {
        self.handle_key_event(KeyEvent::new(action.key(), KeyModifiers::NONE));
    }

    // Dispatches one key press. `pub(crate)` so the mouse tests can drive the
    // real key path (e.g. Escape after a drag) instead of the components alone.
    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.show_help {
            // The help is scrollable: `j`/`k`, the arrows and PgUp/PgDn move
            // through the content instead of acting on the list behind it.
            // Closing is reserved for Esc, q/Q and the K toggle.
            match key_event.code {
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc | KeyCode::Char('K') => {
                    self.close_help()
                }
                KeyCode::Up | KeyCode::Char('k') => self.scroll_help(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll_help(1),
                KeyCode::PageUp => self.scroll_help(-(self.help_viewport() as i32)),
                KeyCode::PageDown => self.scroll_help(self.help_viewport() as i32),
                KeyCode::Home => self.help_scroll = 0,
                KeyCode::End => self.help_scroll = self.help_max_scroll(),
                _ => {}
            }
            return;
        }

        if self.show_popup.is_some() {
            let is_forceable = self
                .show_popup
                .as_ref()
                .is_some_and(|popup| popup.is_forceable);
            match key_event.code {
                // Space applies the offered auto-fix; without one it keeps the
                // original "close without action" meaning.
                KeyCode::Char(' ') => {
                    let has_fix = self
                        .show_popup
                        .as_ref()
                        .is_some_and(|popup| popup.fix.is_some());
                    self.handle_popup_action(if has_fix {
                        PopupAction::AutoFix
                    } else {
                        PopupAction::Close
                    });
                }
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.handle_popup_action(PopupAction::Close)
                }
                KeyCode::Enter => self.handle_popup_action(PopupAction::Accept),
                KeyCode::Char('f') | KeyCode::Char('F') if is_forceable => {
                    self.handle_popup_action(PopupAction::Force)
                }
                _ => {
                    // For unrelated keys, keep the popup visible
                }
            }
            if self.show_popup.is_none() {
                // A dismissed popup must not leave its button hover behind.
                self.clear_hover();
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
                                            fix: None,
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
                                                fix: None,
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
            if self.show_preset_menu.is_none() {
                // A consumed menu button must not leave a tooltip on the list.
                self.clear_hover();
            }
            return;
        }

        // The workspace picker is a small modal: digits assign and close it,
        // Esc/q dismiss it, and nothing behind it sees the key.
        if self.workspace_picker.is_some() {
            // The cell may have disappeared after a resize; do not keep an
            // invisible modal that swallows every key.
            if let Some(layout) = self.layout
                && self.workspace_picker_anchor(layout).is_none()
            {
                self.workspace_picker = None;
                self.clear_hover();
                return;
            }
            match key_event.code {
                KeyCode::Char(c @ '0'..='9') => {
                    self.workspace_picker = None;
                    self.assign_workspace(c.to_digit(10).unwrap_or(0) as u8);
                    self.clear_hover();
                }
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.workspace_picker = None;
                    self.clear_hover();
                }
                _ => {}
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
        if self.mode == TUIMode::View && self.has_pending_move() {
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
            KeyCode::Char('w') => {
                // Validate first: a clean layout writes, an invalid one opens
                // the highlighted error popup (and offers an auto-fix).
                if self.check_layout() {
                    self.write();
                }
            }
            KeyCode::Char('K') if self.mode != TUIMode::Move => {
                self.show_help = true;
                self.help_scroll = 0;
                self.help_scroll_drag = None;
                // A list/map tooltip must not linger over the modal.
                self.clear_hover();
            }
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

    // Highest scroll offset the help modal supports right now.
    fn help_max_scroll(&self) -> u16 {
        self.layout
            .map(|layout| HelpModal::max_scroll(layout.area))
            .unwrap_or(0)
    }

    // Visible help rows, used as the page step.
    fn help_viewport(&self) -> u16 {
        self.layout
            .map(|layout| HelpModal::viewport(layout.area))
            .unwrap_or(0)
    }

    // Scrolls the help by `delta` rows, clamped to its content.
    pub(crate) fn scroll_help(&mut self, delta: i32) {
        let max = self.help_max_scroll() as i32;
        self.help_scroll = (self.help_scroll as i32 + delta).clamp(0, max) as u16;
    }

    // Drops the pointer state so a tooltip from behind the modal (or from the
    // modal's own close button) cannot reappear after it is dismissed.
    pub(crate) fn clear_hover(&mut self) {
        let point = self.hover_point;
        self.set_hover(None, point, Instant::now(), false);
        self.hover_since = None;
        self.tooltip = None;
    }

    // Closes the help modal and leaves no stale hover behind.
    pub(crate) fn close_help(&mut self) {
        self.show_help = false;
        self.help_scroll_drag = None;
        self.clear_hover();
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
    pub(crate) fn clear_active_preset_if_changed(&mut self) {
        if let Some(ref active) = self.active_preset.clone() {
            if let Some(state) = crate::config::load_preset_in(&self.presets_dir(), active) {
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

    // Commit the pending monitor move: persist the layout and drop the record.
    pub(crate) fn apply_move(&mut self) {
        // A single pending monitor is the natural candidate to separate when
        // the applied layout overlaps.
        let moved = if self.move_origins.len() == 1 {
            self.move_origins.keys().next().copied()
        } else {
            None
        };
        if let Err(e) = save_monitor_state_in(&self.state_path(), &self.monitors) {
            eprintln!("✗ Failed to save monitor state: {}", e);
        }
        self.move_origins.clear();
        self.clear_active_preset_if_changed();
        self.check_layout_for(moved);
    }

    // Records that `index` has an unapplied move from `origin`. A monitor already
    // on record keeps its original position, so re-dragging does not lose the
    // revert target, and every pending monitor shows its own Apply button.
    pub(crate) fn note_move(&mut self, index: usize, origin: (i32, i32)) {
        self.move_origins.entry(index).or_insert(origin);
    }

    // Drops a monitor's move record without touching its position. Used when the
    // move can no longer be applied (e.g. the monitor was disabled).
    pub(crate) fn discard_move(&mut self, index: usize) {
        self.move_origins.remove(&index);
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

    // Runs the shared validation module over the live layout and flattens the
    // structured errors to the strings the popup shows. Keeping a single rule
    // engine means the write check and the diagnostics highlights can never
    // drift apart.
    fn validate(&self) -> Result<(), Vec<String>> {
        crate::validation::validate(&self.monitors)
    }

    // Runs the layout check with no pending-move hint.
    pub(crate) fn check_layout(&mut self) -> bool {
        self.check_layout_for(None)
    }

    // Validates the live layout: on success the highlights and popup are
    // cleared; on failure the map highlights are armed (and blink) and the
    // error popup gains the next auto-fix, if one exists.
    fn check_layout_for(&mut self, moved: Option<usize>) -> bool {
        let errors = match self.validate() {
            Ok(()) => {
                self.clear_highlights();
                self.show_popup = None;
                return true;
            }
            Err(errors) => errors,
        };

        let highlights = layout_highlights(&self.monitors);
        if highlights.is_empty() {
            // Errors without map geometry (e.g. a missing origin) have nothing
            // to blink; only the popup is needed.
            self.clear_highlights();
        } else {
            self.highlights = Some(highlights);
            self.arm_blink(Instant::now());
        }

        let fix = plan_auto_fix(&self.monitors, moved);
        let mut lines = errors;
        if fix.is_none() {
            // With a fix the "Space auto fix" button advertises the action; the
            // popup only has to say when nothing can be done.
            lines.push(String::new());
            lines.push("No auto fix possible".to_string());
        }

        self.show_popup = Some(Popup {
            title: " Error ".to_string(),
            lines,
            is_error: true,
            apply_preset: None,
            is_forceable: true,
            fix,
        });
        false
    }

    // Applies one auto-fix to the working layout, persists the state (never the
    // Hyprland config) and refreshes the popup with whatever problems remain.
    fn apply_auto_fix(&mut self, fix: AutoFix) {
        let moved = match fix {
            AutoFix::Separate { monitor, dx, dy } | AutoFix::Abut { monitor, dx, dy } => {
                if let Some(position) = self
                    .monitors
                    .get_mut(monitor)
                    .and_then(|monitor| monitor.position.as_mut())
                {
                    position.x += dx;
                    position.y += dy;
                }
                Some(monitor)
            }
            AutoFix::ToOrigin { dx, dy, .. } => {
                // Anchor the whole layout: every monitor moves together, so the
                // relative geometry (and any remaining problems) is unchanged.
                for monitor in &mut self.monitors {
                    if let Some(position) = monitor.position.as_mut() {
                        position.x += dx;
                        position.y += dy;
                    }
                    if let Some(saved) = monitor.saved_position.as_mut() {
                        saved.x += dx;
                        saved.y += dy;
                    }
                }
                // Pending moves are revert targets in absolute coordinates;
                // they must move with the layout or Cancel would teleport a
                // monitor back to its pre-translation spot.
                for origin in self.move_origins.values_mut() {
                    origin.0 += dx;
                    origin.1 += dy;
                }
                None
            }
        };
        self.clear_active_preset_if_changed();

        if let Err(e) = save_monitor_state_in(&self.state_path(), &self.monitors) {
            eprintln!("✗ Failed to save monitor state: {}", e);
        }

        // One fix per press: re-validate and either refresh the popup with the
        // next problem or close it once the layout is clean.
        self.check_layout_for(moved);
    }

    // Dispatches a popup action. The key handler and the mouse buttons share
    // this path, so clicking a button does exactly what its key does.
    pub(crate) fn handle_popup_action(&mut self, action: PopupAction) {
        let (is_error, is_forceable, fix) = match &self.show_popup {
            Some(popup) => (popup.is_error, popup.is_forceable, popup.fix),
            None => return,
        };
        match action {
            PopupAction::AutoFix => match fix {
                Some(fix) => self.apply_auto_fix(fix),
                None => {
                    self.show_popup = None;
                    self.clear_highlights();
                }
            },
            PopupAction::Force => {
                if is_forceable {
                    self.write();
                    self.show_popup = None;
                    self.clear_highlights();
                }
            }
            PopupAction::Accept => {
                if !is_error {
                    // Non-error popups (mismatch/warning) accept by writing.
                    self.write();
                }
                self.show_popup = None;
                self.clear_highlights();
            }
            PopupAction::Close => {
                self.show_popup = None;
                self.clear_highlights();
            }
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
                fix: None,
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
                fix: None,
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

    fn pending_move_app(current: (i32, i32), origin: (i32, i32)) -> App {
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
        let mut app = pending_move_app(current, origin);
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

    // A help overlay on a terminal too short to show every section.
    fn help_app(width: u16, height: u16) -> App {
        let area = ratatui::layout::Rect::new(0, 0, width, height);
        let mut app = App::new(None);
        app.layout = Some(crate::ui::layouts::screen_layout(area, TUIMode::View));
        app.show_help = true;
        app
    }

    #[test]
    fn help_scrolls_with_j_k_and_the_arrows() {
        let area = ratatui::layout::Rect::new(0, 0, 60, 15);
        let mut app = help_app(60, 15);
        let max = HelpModal::max_scroll(area);
        assert!(max > 0, "the test terminal must need scrolling");

        let keys = |code| KeyEvent::new(code, KeyModifiers::NONE);

        app.handle_key_event(keys(KeyCode::Char('j')));
        assert_eq!(app.help_scroll, 1);
        app.handle_key_event(keys(KeyCode::Down));
        assert_eq!(app.help_scroll, 2);
        app.handle_key_event(keys(KeyCode::Char('k')));
        assert_eq!(app.help_scroll, 1);
        app.handle_key_event(keys(KeyCode::Up));
        assert_eq!(app.help_scroll, 0);
        assert!(app.show_help, "scrolling must not close the help");

        app.handle_key_event(keys(KeyCode::End));
        assert_eq!(app.help_scroll, max);
        app.handle_key_event(keys(KeyCode::Char('j')));
        assert_eq!(app.help_scroll, max, "the offset is clamped");
        app.handle_key_event(keys(KeyCode::Home));
        assert_eq!(app.help_scroll, 0);

        app.handle_key_event(keys(KeyCode::PageDown));
        assert_eq!(app.help_scroll, HelpModal::viewport(area).min(max));
        app.handle_key_event(keys(KeyCode::PageUp));
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn help_still_closes_with_q_escape_or_the_toggle() {
        for code in [
            KeyCode::Char('q'),
            KeyCode::Char('Q'),
            KeyCode::Esc,
            KeyCode::Char('K'),
        ] {
            let mut app = help_app(60, 15);
            app.help_scroll = 3;

            app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));

            assert!(!app.show_help, "{code:?} must close the help");
        }
    }

    #[test]
    fn a_stale_scrollbar_hover_is_dropped_when_no_bar_exists() {
        // 120x40 fits all content: the scrollbar lane is empty.
        let mut app = help_app(120, 40);
        let t0 = Instant::now();
        app.hover = Some(HoverTarget::HelpScrollbar);
        app.hover_since = Some(t0);

        // Only the tooltip dwell remains armed; hover itself never animates.
        let deadline = app.next_deadline_at(t0).expect("the tooltip still runs");
        assert!(
            deadline >= Duration::from_secs(1),
            "no short redraw loop without a bar: {deadline:?}"
        );

        app.drop_unavailable_help_hover();
        assert!(app.hover.is_none(), "the stale target is dropped");
        assert!(app.hover_since.is_none());
    }

    #[test]
    fn closing_the_help_clears_its_close_tooltip() {
        let mut app = help_app(60, 15);
        app.hover = Some(HoverTarget::HelpClose);
        app.hover_point = (20, 10);
        app.hover_since = Some(Instant::now() - std::time::Duration::from_secs(10));

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.hover.is_none(), "the help-close target is dropped");
        assert!(
            app.tooltip_at(Instant::now() + std::time::Duration::from_secs(10))
                .is_none(),
            "no stale tooltip is recomputed after closing"
        );
    }

    #[test]
    fn opening_the_help_drops_a_tooltip_from_behind_it() {
        let mut app = help_app(60, 15);
        app.show_help = false;
        app.hover = Some(HoverTarget::ListRow(0));
        app.hover_since = Some(Instant::now() - std::time::Duration::from_secs(10));

        app.handle_key_event(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE));

        assert!(app.show_help);
        assert!(app.hover.is_none(), "the list hover is cleared");
        assert!(
            app.tooltip_at(Instant::now() + std::time::Duration::from_secs(10))
                .is_none(),
            "the list tooltip cannot cover the modal"
        );
    }

    #[test]
    fn opening_and_closing_the_help_restores_a_hovered_selection() {
        let area = ratatui::layout::Rect::new(0, 0, 120, 40);
        let mut second = monitor();
        second.name = "DP-2".to_string();
        second.position = Some(Position { x: 1920, y: 0 });
        let mut app = App::new(None);
        app.monitors = vec![monitor(), second];
        app.layout = Some(crate::ui::layouts::screen_layout(area, TUIMode::View));

        let now = Instant::now();
        app.set_hover(Some(HoverTarget::Map(1)), (10, 10), now, true);
        assert_eq!(app.selected_monitor, 1, "hover selects the monitor");

        app.handle_key_event(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE));
        assert!(app.show_help);
        assert_eq!(
            app.selected_monitor, 0,
            "opening restores the committed row"
        );
        assert!(app.hover.is_none() && app.hover_restore.is_none());

        app.set_hover(Some(HoverTarget::Map(1)), (10, 10), now, true);
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.show_help);
        assert_eq!(
            app.selected_monitor, 0,
            "closing restores the committed row"
        );
        assert!(app.hover.is_none() && app.hover_restore.is_none());
    }

    #[test]
    fn opening_the_help_starts_at_the_top() {
        let mut app = help_app(60, 15);
        app.help_scroll = 4;
        app.show_help = false;

        app.handle_key_event(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE));

        assert!(app.show_help);
        assert_eq!(app.help_scroll, 0, "a fresh help starts at the top");
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
        assert!(
            app.move_origins.is_empty(),
            "the cancelled move is not pending"
        );
    }

    #[test]
    fn escape_aborts_a_drag_but_keeps_a_move_that_was_already_pending() {
        let mut app = app_mid_drag((300, 0), (200, 0), true);

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.monitors[0].position, Some(Position { x: 200, y: 0 }));
        assert!(
            app.move_origins.contains_key(&0),
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
        let mut app = pending_move_app((250, 140), (100, 100));
        assert!(app.drag.is_none(), "the drag already ended");

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(
            app.monitors[0].position,
            Some(Position { x: 100, y: 100 }),
            "Escape puts the monitor back"
        );
        assert!(app.move_origins.is_empty(), "and drops the pending move");
    }

    #[test]
    fn q_reverts_a_pending_move_instead_of_quitting() {
        // The state path is injected so a regression in the gate (which would
        // let `q` fall through to exit) still cannot touch the real config.
        let dir = TempDir::new("app-q-revert");
        let mut app = pending_move_app((250, 140), (100, 100));
        app.state_path = Some(dir.path().join("monitor_state.json"));

        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

        assert!(!app.exit, "q must not quit while a move is unconfirmed");
        assert_eq!(app.monitors[0].position, Some(Position { x: 100, y: 100 }));
        assert!(app.move_origins.is_empty());
    }

    #[test]
    fn other_keys_are_ignored_while_a_move_is_pending() {
        let mut app = pending_move_app((250, 140), (100, 100));

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
        assert!(
            app.move_origins.contains_key(&0),
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
    }

    #[test]
    fn enter_applies_a_pending_move_written_through_the_state_path() {
        let dir = TempDir::new("app-apply-move");
        let mut app = pending_move_app((250, 140), (100, 100));
        app.state_path = Some(dir.path().join("monitor_state.json"));

        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(!app.has_pending_move());
        assert!(
            dir.path().join("monitor_state.json").exists(),
            "Enter persists the applied layout"
        );
    }

    #[test]
    fn a_sub_mode_is_not_gated_by_a_pending_move() {
        // Esc keeps its sub-mode meaning outside View; the move record survives
        // until the user is back in View to confirm or revert it.
        let mut app = pending_move_app((250, 140), (100, 100));
        app.mode = TUIMode::Resolution;

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.mode, TUIMode::View);
        assert_eq!(
            app.monitors[0].position,
            Some(Position { x: 250, y: 140 }),
            "the move is untouched in a sub-mode"
        );
        assert!(app.move_origins.contains_key(&0));
    }

    #[test]
    fn enter_applies_a_drag_that_is_still_in_progress() {
        let dir = TempDir::new("app-apply-drag");
        let mut app = app_mid_drag((250, 140), (100, 100), false);
        app.state_path = Some(dir.path().join("monitor_state.json"));

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert!(app.drag.is_none(), "Space ends the drag");
        assert!(app.move_origins.is_empty(), "and applies the move");
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
        let mut app = pending_move_app((250, 140), (100, 100));
        app.move_origins.clear();
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
        let mut app = pending_move_app((250, 140), (100, 100));
        app.move_origins.clear();
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
    fn discarding_one_move_keeps_the_other_pending() {
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

        app.discard_move(1);

        assert!(
            app.move_origins.contains_key(&0) && !app.move_origins.contains_key(&1),
            "the other move stays pending"
        );
        assert_eq!(app.move_origins.get(&0), Some(&(10, 20)));
    }

    // App::validate must delegate to the shared rule module so the write check
    // and the structured diagnostics report exactly the same problems.
    #[test]
    fn validate_reports_the_shared_rule_messages() {
        let overlap = app_with(vec![
            Monitor {
                name: "A".to_string(),
                enabled: true,
                position: Some(Position { x: 0, y: 0 }),
                workspace: Some(1),
                modes: vec![crate::monitor::Resolution {
                    width: 1920,
                    height: 1080,
                    refresh: 60.0,
                    preferred: true,
                    current: true,
                }],
                ..Default::default()
            },
            Monitor {
                name: "B".to_string(),
                enabled: true,
                position: Some(Position { x: 100, y: 0 }),
                workspace: Some(1),
                modes: vec![crate::monitor::Resolution {
                    width: 1920,
                    height: 1080,
                    refresh: 60.0,
                    preferred: true,
                    current: true,
                }],
                ..Default::default()
            },
        ]);

        let errors = overlap.validate().unwrap_err();

        assert_eq!(
            errors,
            vec![
                "Workspace 1 is assigned to multiple monitors: A, B".to_string(),
                "Monitors overlap: A and B".to_string(),
            ],
            "App::validate must surface the module's structured messages"
        );
    }

    #[test]
    fn validate_accepts_a_contiguous_layout() {
        let mut app = app_with(vec![
            Monitor {
                name: "A".to_string(),
                enabled: true,
                position: Some(Position { x: 0, y: 0 }),
                modes: vec![crate::monitor::Resolution {
                    width: 1920,
                    height: 1080,
                    refresh: 60.0,
                    preferred: true,
                    current: true,
                }],
                ..Default::default()
            },
            Monitor {
                name: "B".to_string(),
                enabled: true,
                position: Some(Position { x: 1920, y: 0 }),
                modes: vec![crate::monitor::Resolution {
                    width: 1920,
                    height: 1080,
                    refresh: 60.0,
                    preferred: true,
                    current: true,
                }],
                ..Default::default()
            },
        ]);

        assert_eq!(app.validate(), Ok(()));
        // Sanity: the delegate is what actually runs.
        app.monitors[1].position = Some(Position { x: 0, y: 0 });
        assert!(app.validate().is_err());
    }

    // --- animation scheduler --------------------------------------------

    #[test]
    fn nothing_animates_without_an_armed_blink() {
        let app = App::new(None);
        assert_eq!(app.next_deadline_at(Instant::now()), None);
    }

    #[test]
    fn the_next_deadline_shrinks_as_the_phase_elapses() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.arm_blink(t0);

        assert_eq!(app.next_deadline_at(t0), Some(BLINK_TOGGLE));
        assert_eq!(
            app.next_deadline_at(t0 + Duration::from_millis(200)),
            Some(Duration::from_millis(300))
        );
        assert_eq!(
            app.next_deadline_at(t0 + Duration::from_secs(5)),
            Some(Duration::ZERO),
            "an overdue deadline polls immediately"
        );
    }

    #[test]
    fn the_blink_toggles_only_once_a_full_phase_has_passed() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.arm_blink(t0);
        assert!(
            app.blink.as_ref().unwrap().lit,
            "the blink starts in the lit phase"
        );

        app.advance_animation_at(t0 + Duration::from_millis(499));
        assert!(app.blink.as_ref().unwrap().lit, "too early to toggle");

        app.advance_animation_at(t0 + Duration::from_millis(500));
        assert!(!app.blink.as_ref().unwrap().lit, "the phase toggled");
        assert_eq!(app.blink.as_ref().unwrap().toggles_left, BLINK_TOGGLES - 1);
    }

    #[test]
    fn the_blink_auto_disarms_after_eight_toggles_and_settles_lit() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.highlights = Some(crate::validation::diagnostics::LayoutHighlights::default());
        app.arm_blink(t0);

        let mut lit_phases = 1; // starts lit
        for step in 1..=BLINK_TOGGLES {
            let was_lit = app.blink.as_ref().unwrap().lit;
            app.advance_animation_at(t0 + Duration::from_millis(500 * step as u64));
            if !was_lit {
                lit_phases += 1;
            }
        }

        assert_eq!(lit_phases, 5, "the initial phase plus four flashes");
        assert!(app.blink.is_none(), "the blink disarmed itself");
        assert!(
            app.highlight_visible(),
            "highlights stay solid red after the blink"
        );
    }

    #[test]
    fn an_armed_blink_does_not_block_keys() {
        let dir = TempDir::new("app-blink-keys");
        let mut app = pending_move_app((250, 140), (100, 100));
        app.state_path = Some(dir.path().join("monitor_state.json"));
        app.arm_blink(Instant::now());

        // The key path runs while the blink is pending, exactly as it would
        // between two poll timeouts.
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.move_origins.is_empty(), "Esc is still handled");
        assert_eq!(app.monitors[0].position, Some(Position { x: 100, y: 100 }));
    }

    #[test]
    fn hovering_a_monitor_temporarily_selects_it() {
        let t0 = Instant::now();
        let mut a = app_with(vec![
            monitor(),
            Monitor {
                name: "DP-2".to_string(),
                ..monitor()
            },
        ]);
        a.selected_monitor = 0;

        // Hovering a map monitor moves the selection.
        a.set_hover(Some(HoverTarget::Map(1)), (10, 10), t0, true);
        assert_eq!(a.selected_monitor, 1, "the hovered monitor is selected");

        // Moving within the map but off the monitors keeps the temporary
        // selection until the pointer leaves the map area.
        a.set_hover(None, (10, 10), t0 + Duration::from_millis(10), true);
        assert_eq!(a.selected_monitor, 1, "still inside the map");

        // Leaving the map restores the previous selection.
        a.set_hover(None, (10, 10), t0 + Duration::from_millis(20), false);
        assert_eq!(a.selected_monitor, 0, "hover-out reverts");

        // Non-map hover targets never override the selection.
        a.set_hover(Some(HoverTarget::ListRow(1)), (10, 10), t0, false);
        assert_eq!(a.selected_monitor, 0);
    }

    // A keyboard selection made while the pointer rests on the map must not be
    // undone when the pointer leaves.
    #[test]
    fn a_keyboard_selection_while_hovering_is_not_undone() {
        let t0 = Instant::now();
        let mut a = app_with(vec![
            monitor(),
            Monitor {
                name: "DP-2".to_string(),
                ..monitor()
            },
            Monitor {
                name: "DP-3".to_string(),
                ..monitor()
            },
        ]);
        a.selected_monitor = 0;

        a.set_hover(Some(HoverTarget::Map(1)), (10, 10), t0, true);
        assert_eq!(a.selected_monitor, 1);

        // j/k selects the third monitor while the pointer still hovers the map.
        a.selected_monitor = 2;
        a.set_hover(None, (10, 10), t0 + Duration::from_millis(10), false);

        assert_eq!(
            a.selected_monitor, 2,
            "the keyboard selection wins over the hover restore"
        );
    }

    // Hovering a different monitor after a keyboard selection keeps the
    // keyboard selection as the restore target.
    #[test]
    fn a_keyboard_selection_becomes_the_restore_target() {
        let t0 = Instant::now();
        let mut a = app_with(vec![
            monitor(),
            Monitor {
                name: "DP-2".to_string(),
                ..monitor()
            },
            Monitor {
                name: "DP-3".to_string(),
                ..monitor()
            },
        ]);
        a.selected_monitor = 0;

        a.set_hover(Some(HoverTarget::Map(1)), (10, 10), t0, true);
        a.selected_monitor = 2; // keyboard
        a.set_hover(Some(HoverTarget::Map(0)), (10, 10), t0, true);
        assert_eq!(a.selected_monitor, 0, "the newly hovered monitor is shown");

        a.set_hover(None, (10, 10), t0 + Duration::from_millis(10), false);

        assert_eq!(
            a.selected_monitor, 2,
            "hover-out restores the keyboard selection, not the pre-hover one"
        );
    }

    #[test]
    fn clearing_the_highlights_stops_the_blink() {
        let mut app = App::new(None);
        app.highlights = Some(crate::validation::diagnostics::LayoutHighlights::default());
        app.arm_blink(Instant::now());

        app.clear_highlights();

        assert!(app.blink.is_none());
        assert!(app.highlights.is_none());
        assert!(!app.highlight_visible());
        assert_eq!(app.next_deadline_at(Instant::now()), None);
    }

    // Hover is a steady glow: it must never arm an animation timer. Only
    // validation errors (the blink) may animate.
    #[test]
    fn hover_does_not_arm_the_blink_only_the_tooltip_dwell() {
        use crate::ui::tooltip::TOOLTIP_DELAY;

        let t0 = Instant::now();
        let mut app = App::new(None);
        app.set_hover(Some(HoverTarget::ListRow(0)), (10, 10), t0, false);

        assert!(app.blink.is_none(), "hover must not start the blink");
        assert_eq!(
            app.next_deadline_at(t0),
            Some(TOOLTIP_DELAY),
            "the only wake-up is the tooltip dwell"
        );

        app.advance_animation_at(t0 + Duration::from_millis(80));
        assert_eq!(
            app.next_deadline_at(t0 + Duration::from_millis(80)),
            Some(TOOLTIP_DELAY - Duration::from_millis(80))
        );

        // Hover-out keeps it idle.
        app.set_hover(None, (10, 10), t0 + Duration::from_millis(100), false);
        assert_eq!(app.next_deadline_at(t0 + Duration::from_millis(100)), None);
    }

    // The tooltip must appear without further input: the loop wakes at the
    // dwell deadline, then once per fade step, and finally idles.
    #[test]
    fn the_tooltip_deadline_wakes_the_loop_and_then_settles() {
        use crate::ui::tooltip::{TOOLTIP_DELAY, TOOLTIP_FADE};

        let t0 = Instant::now();
        let mut app = App::new(None);
        app.set_hover(Some(HoverTarget::Map(0)), (10, 5), t0, true);

        assert_eq!(app.next_deadline_at(t0), Some(TOOLTIP_DELAY));
        assert!(app.tooltip_at(t0 + TOOLTIP_DELAY).is_some());
        assert_eq!(
            app.next_deadline_at(t0 + TOOLTIP_DELAY),
            Some(TOOLTIP_FADE),
            "the first fade step is scheduled"
        );
        assert_eq!(
            app.next_deadline_at(t0 + TOOLTIP_DELAY + TOOLTIP_FADE),
            Some(TOOLTIP_FADE)
        );
        assert_eq!(
            app.next_deadline_at(t0 + TOOLTIP_DELAY + TOOLTIP_FADE * 3),
            None,
            "a fully faded tooltip needs no more wake-ups"
        );
    }

    #[test]
    fn a_hovered_layout_without_errors_does_not_animate() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.set_hover(Some(HoverTarget::Map(0)), (10, 5), t0, true);

        // A clean layout: no blink is armed. Once the tooltip has fully faded
        // in, the loop blocks on input again.
        assert_eq!(app.next_deadline_at(t0 + Duration::from_secs(10)), None);
        assert!(!app.highlight_visible());
    }

    #[test]
    fn hover_does_not_shorten_the_blink_deadline() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.arm_blink(t0);
        app.set_hover(
            Some(HoverTarget::Map(0)),
            (10, 5),
            t0 + Duration::from_millis(450),
            true,
        );

        // Only the blink drives the deadline; hover adds nothing.
        assert_eq!(
            app.next_deadline_at(t0 + Duration::from_millis(450)),
            Some(Duration::from_millis(50))
        );
    }

    #[test]
    fn the_tooltip_appears_only_after_the_dwell_and_fades_in() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.set_hover(Some(HoverTarget::Map(0)), (10, 5), t0, true);

        assert!(
            app.tooltip_at(t0 + Duration::from_millis(1999)).is_none(),
            "nothing before the 2s dwell"
        );

        let state = app
            .tooltip_at(t0 + Duration::from_millis(2000))
            .expect("tooltip");
        assert_eq!(state.text, "Drag to move, click to select");
        assert_eq!(state.step, 0, "first fade step");
        assert_eq!(state.anchor, (10, 5));

        assert_eq!(
            app.tooltip_at(t0 + Duration::from_millis(2250))
                .unwrap()
                .step,
            2
        );
        assert_eq!(
            app.tooltip_at(t0 + Duration::from_secs(5)).unwrap().step,
            3,
            "fully faded in and stable"
        );
    }

    #[test]
    fn changing_the_hover_target_restarts_the_dwell() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.set_hover(Some(HoverTarget::Map(0)), (10, 5), t0, true);
        app.set_hover(
            Some(HoverTarget::Map(1)),
            (20, 5),
            t0 + Duration::from_secs(3),
            true,
        );

        assert!(
            app.tooltip_at(t0 + Duration::from_millis(2100)).is_none(),
            "the new element's dwell starts over"
        );
        assert!(app.tooltip_at(t0 + Duration::from_secs(6)).is_some());
    }

    // The dwell clock must not be disturbed by anything that happens while the
    // pointer rests, so a hovered element still reaches its 3s tooltip.
    #[test]
    fn the_tooltip_dwell_survives_animation_ticks() {
        let t0 = Instant::now();
        let mut app = App::new(None);
        app.set_hover(Some(HoverTarget::Map(0)), (10, 5), t0, true);

        // Run the loop for three seconds, including a validation blink.
        app.highlights = Some(crate::validation::diagnostics::LayoutHighlights::default());
        app.arm_blink(t0);
        for step in 1..=40u64 {
            app.advance_animation_at(t0 + Duration::from_millis(80 * step));
        }

        assert!(
            app.tooltip_at(t0 + Duration::from_millis(2000)).is_some(),
            "the tooltip is due right at the 2s mark"
        );
        assert!(app.tooltip_at(t0 + Duration::from_millis(2200)).is_some());
    }

    // --- validate on apply + Space auto-fix -----------------------------

    fn sized_monitor(name: &str, x: i32, y: i32, w: i32, h: i32) -> Monitor {
        let mut monitor = monitor();
        monitor.name = name.to_string();
        monitor.position = Some(Position { x, y });
        monitor.modes = vec![crate::monitor::Resolution {
            width: w,
            height: h,
            refresh: 60.0,
            preferred: true,
            current: true,
        }];
        monitor
    }

    fn app_with_state(monitors: Vec<Monitor>, tag: &str) -> (App, TempDir) {
        let dir = TempDir::new(tag);
        let mut app = app_with(monitors);
        app.state_path = Some(dir.path().join("monitor_state.json"));
        (app, dir)
    }

    #[test]
    fn applying_an_overlapping_move_arms_highlights_and_an_auto_fix() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 0, 0, 1920, 1080),
                sized_monitor("B", 100, 0, 1920, 1080),
            ],
            "apply-overlap",
        );
        app.note_move(1, (1920, 0));

        app.apply_move();

        let popup = app.show_popup.as_ref().expect("validation popup");
        assert!(popup.lines.iter().any(|line| line.contains("overlap")));
        assert!(popup.fix.is_some(), "a move-only fix exists");
        assert!(
            !popup
                .lines
                .iter()
                .any(|line| line.contains("No auto fix possible")),
            "a fix exists, so the impossible message must stay hidden"
        );
        assert!(app.highlights.is_some(), "the map overlay is armed");
    }

    #[test]
    fn space_applies_the_auto_fix_and_closes_the_popup() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 0, 0, 1920, 1080),
                sized_monitor("B", 1800, 0, 1920, 1080),
            ],
            "fix-close",
        );
        assert!(!app.check_layout(), "the layout is invalid");

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert!(app.show_popup.is_none(), "the fix resolved the problem");
        assert!(app.highlights.is_none());
        assert!(app.blink.is_none());
        assert_eq!(
            app.monitors[1].position,
            Some(Position { x: 1920, y: 0 }),
            "B was pushed flush off A"
        );
    }

    #[test]
    fn each_space_press_applies_the_next_fix() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 0, 0, 100, 100),
                sized_monitor("B", 50, 0, 100, 100),
                sized_monitor("C", 5000, 0, 100, 100),
            ],
            "fix-next",
        );
        assert!(!app.check_layout());

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(app.show_popup.is_some(), "the detachment remains");
        assert_eq!(app.monitors[1].position, Some(Position { x: 100, y: 0 }));
        assert_eq!(
            app.monitors[2].position,
            Some(Position { x: 5000, y: 0 }),
            "one problem per press"
        );
        assert!(app.highlights.is_some());

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(app.show_popup.is_none(), "the second fix cleans the layout");
        assert_eq!(app.monitors[2].position, Some(Position { x: 200, y: 0 }));
        assert!(app.highlights.is_none());
    }

    // The auto fix must centre the detached monitor on its anchor's facing
    // edge, not merely flush it: both axes move.
    #[test]
    fn the_auto_fix_centres_the_detached_monitor_on_the_anchor_edge() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 0, 0, 1920, 1080),
                // To the right of A and 400px lower, so the centres differ.
                sized_monitor("C", 5000, 400, 1920, 1080),
            ],
            "fix-centre",
        );
        assert!(!app.check_layout());

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(
            app.monitors[1].position,
            Some(Position { x: 1920, y: 0 }),
            "flush against A's right edge and vertically centred on it"
        );
        assert!(app.show_popup.is_none(), "the layout is clean");
        assert!(app.highlights.is_none());
    }

    // A contiguous layout with no monitor at (0,0) is an error; Space anchors
    // the whole layout on the chosen monitor.
    #[test]
    fn the_auto_fix_anchors_a_layout_that_is_off_the_origin() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 1000, 0, 1920, 1080),
                sized_monitor("B", 2920, 0, 1920, 1080),
            ],
            "fix-origin",
        );
        assert!(!app.check_layout());

        let popup = app.show_popup.as_ref().expect("origin popup");
        assert!(
            popup.lines.iter().any(|line| line.contains("origin")),
            "got {:?}",
            popup.lines
        );

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(
            app.monitors[0].position,
            Some(Position { x: 0, y: 0 }),
            "the leftmost monitor anchors the layout"
        );
        assert_eq!(
            app.monitors[1].position,
            Some(Position { x: 1920, y: 0 }),
            "every monitor moves together"
        );
        assert!(app.show_popup.is_none(), "the layout is clean");
    }

    // A disabled monitor keeps its saved position across an origin shift, so
    // re-enabling it later does not teleport it.
    #[test]
    fn the_origin_fix_translates_saved_positions() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 1000, 0, 1920, 1080),
                sized_monitor("B", 2920, 0, 1920, 1080),
            ],
            "fix-origin-saved",
        );
        app.monitors[0].enabled = false;
        app.monitors[0].saved_position = Some(Position { x: 1000, y: 0 });
        app.monitors[1].saved_position = Some(Position { x: 2920, y: 0 });

        assert!(!app.check_layout());
        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(
            app.monitors[0].saved_position,
            Some(Position { x: -1920, y: 0 })
        );
        assert_eq!(
            app.monitors[1].saved_position,
            Some(Position { x: 0, y: 0 })
        );
    }

    // A pending move's revert target is in absolute coordinates, so the origin
    // fix must translate it too; otherwise Cancel teleports a monitor.
    #[test]
    fn the_origin_fix_translates_pending_move_origins() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 1000, 0, 1920, 1080),
                sized_monitor("B", 2920, 0, 1920, 1080),
            ],
            "fix-origin-pending",
        );
        // A peculiar pending move that also leaves the origin unresolved.
        app.note_move(1, (4000, 500));

        assert!(!app.check_layout());
        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(
            app.move_origins.get(&1),
            Some(&(3000, 500)),
            "the revert target moved with the layout"
        );

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(
            app.monitors[1].position,
            Some(Position { x: 3000, y: 500 }),
            "Cancel restores the origin-relative position, not the old one"
        );
    }

    #[test]
    fn no_auto_fix_is_reported_when_every_move_would_create_a_new_overlap() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 80, 0, 100, 100),
                sized_monitor("B", 100, 0, 100, 100),
                sized_monitor("C", 250, 0, 100, 100),
                sized_monitor("D", -50, 0, 100, 100),
            ],
            "no-fix",
        );
        assert!(!app.check_layout());

        let popup = app.show_popup.as_ref().expect("validation popup");
        assert_eq!(popup.fix, None);
        assert!(
            popup
                .lines
                .iter()
                .any(|line| line.contains("No auto fix possible")),
            "got {:?}",
            popup.lines
        );

        // Space without a fix keeps the old "close" semantics.
        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(app.show_popup.is_none());
        assert!(app.highlights.is_none());
    }

    #[test]
    fn escape_closes_the_validation_popup_without_touching_the_layout() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 0, 0, 100, 100),
                sized_monitor("B", 50, 0, 100, 100),
            ],
            "escape-validate",
        );
        assert!(!app.check_layout());
        let positions: Vec<_> = app.monitors.iter().map(|m| m.position.clone()).collect();

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.show_popup.is_none());
        assert!(app.highlights.is_none(), "the blink stops");
        assert!(app.blink.is_none());
        let after: Vec<_> = app.monitors.iter().map(|m| m.position.clone()).collect();
        assert_eq!(positions, after);
    }

    #[test]
    fn a_rotation_apply_rechecks_the_layout() {
        let dir = TempDir::new("rotate-validate");
        let mut app = app_with(vec![
            sized_monitor("A", 0, 0, 1920, 1080),
            sized_monitor("B", 1920, 0, 1920, 1080),
        ]);
        app.state_path = Some(dir.path().join("monitor_state.json"));
        assert!(app.check_layout(), "the starting layout is valid");

        // A quarter turn swaps A's logical size, opening a gap to B.
        app.handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));

        assert!(
            app.show_popup.is_some(),
            "the rotation must validate immediately"
        );
        assert!(app.highlights.is_some());
    }

    #[test]
    fn a_scale_apply_rechecks_the_layout() {
        let dir = TempDir::new("scale-validate");
        let mut app = app_with(vec![
            sized_monitor("A", 0, 0, 1920, 1080),
            sized_monitor("B", 1920, 0, 1920, 1080),
        ]);
        app.state_path = Some(dir.path().join("monitor_state.json"));
        app.mode = TUIMode::Scale;
        app.selected_scale = 8; // 200%
        assert!(app.check_layout(), "the starting layout is valid");

        app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(app.monitors[0].scale, Some(2.0));
        assert!(
            app.show_popup.is_some(),
            "the scale change must validate immediately"
        );
        assert!(app.highlights.is_some());
    }

    #[test]
    fn closing_a_popup_with_escape_clears_its_hover() {
        let mut app = App::new(None);
        app.monitors = vec![monitor()];
        app.show_popup = Some(Popup {
            title: " Error ".to_string(),
            lines: vec!["boom".to_string()],
            is_error: true,
            apply_preset: None,
            is_forceable: true,
            fix: None,
        });
        app.hover = Some(HoverTarget::Popup(PopupAction::Close));
        app.hover_since = Some(Instant::now() - Duration::from_secs(10));

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.show_popup.is_none());
        assert!(
            app.hover.is_none(),
            "the popup button hover must be cleared"
        );
        assert!(
            app.tooltip_for_draw(Instant::now()).is_none(),
            "no tooltip may outlive the popup"
        );
    }

    #[test]
    fn closing_the_preset_menu_with_escape_clears_its_hover() {
        let mut app = App::new(None);
        app.preset_backup = Some(vec![]);
        app.show_preset_menu = Some(PresetMenu::new(vec![], &[], None));
        app.hover = Some(HoverTarget::PresetButton(PresetButtonAction::Close));
        app.hover_since = Some(Instant::now() - Duration::from_secs(10));

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.show_preset_menu.is_none());
        assert!(app.hover.is_none(), "the menu button hover must be cleared");
        assert!(app.tooltip_for_draw(Instant::now()).is_none());
    }

    #[test]
    fn a_tooltip_is_suppressed_under_an_open_overlay() {
        let mut app = App::new(None);
        app.monitors = vec![monitor()];
        app.hover = Some(HoverTarget::ListRow(0));
        app.hover_since = Some(Instant::now() - Duration::from_secs(10));
        let now = Instant::now();
        assert!(app.tooltip_at(now).is_some(), "the hover is ready");

        app.show_help = true;

        assert!(
            app.tooltip_for_draw(now).is_none(),
            "an overlay must hide stale hover tooltips"
        );
    }

    #[test]
    fn a_clean_check_closes_a_stale_validation_popup() {
        let (mut app, _dir) = app_with_state(
            vec![
                sized_monitor("A", 0, 0, 100, 100),
                sized_monitor("B", 50, 0, 100, 100),
            ],
            "clean-check",
        );
        assert!(!app.check_layout());
        assert!(app.show_popup.is_some());

        // Fix the layout by hand, then re-check.
        app.monitors[1].position = Some(Position { x: 100, y: 0 });

        assert!(app.check_layout());
        assert!(app.show_popup.is_none());
        assert!(app.highlights.is_none());
    }
}
