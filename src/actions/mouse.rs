//! Mouse event routing and hit-testing.
//!
//! Mouse events mirror the key handler's modal precedence: the help overlay,
//! popups and the preset menu capture input first, then the active mode
//! dispatches to the map, the side table and the monitor list.

use crate::App;
use crate::actions::movement::snap_to_alignment;
use crate::ui::components::{Map, MonitorList, PresetMenu, Resolutions, Scale};
use crate::ui::layouts::ScreenLayout;
use crate::utils::{ScaleValue, TUIMode};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

// World-space distance within which a dragged monitor snaps to alignment.
pub const SNAP_THRESHOLD: f64 = 24.0;

// Routes a mouse event to the component that owns the current view.
pub fn handle_mouse(app: &mut App, event: MouseEvent) {
    let layout = match app.layout {
        Some(layout) => layout,
        None => return,
    };

    if app.show_help {
        if is_left_click(event.kind) {
            app.show_help = false;
        }
        return;
    }

    if app.show_popup.is_some() {
        if is_left_click(event.kind) {
            accept_popup(app);
        }
        return;
    }

    if app.show_preset_menu.is_some() {
        handle_preset(app, layout.area, event);
        return;
    }

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_down(app, layout, point(event))
        }
        MouseEventKind::Drag(MouseButton::Left) => handle_left_drag(app, layout, point(event)),
        MouseEventKind::Up(MouseButton::Left) => app.drag = None,
        MouseEventKind::ScrollUp => handle_scroll(app, layout, point(event), -1),
        MouseEventKind::ScrollDown => handle_scroll(app, layout, point(event), 1),
        _ => {}
    }
}

// A left button press (the only click we act on).
fn is_left_click(kind: MouseEventKind) -> bool {
    matches!(kind, MouseEventKind::Down(MouseButton::Left))
}

fn point(event: MouseEvent) -> (u16, u16) {
    (event.column, event.row)
}

// Half-open containment test so adjacent rectangles never both claim a cell.
fn inside(rect: Rect, point: (u16, u16)) -> bool {
    point.0 >= rect.x && point.0 < rect.right() && point.1 >= rect.y && point.1 < rect.bottom()
}

// A click on a popup behaves like pressing Enter.
fn accept_popup(app: &mut App) {
    let is_error = app.show_popup.as_ref().is_some_and(|popup| popup.is_error);
    if !is_error {
        app.write();
    }
    app.show_popup = None;
}

fn handle_left_down(app: &mut App, layout: ScreenLayout, point: (u16, u16)) {
    if app.monitors.is_empty() {
        return;
    }

    // The map selects a monitor and starts a drag in every mode.
    if inside(layout.canvas, point) {
        if let Some(world) = Map::world_at(layout.canvas, &app.monitors, point.0, point.1)
            && let Some(index) = Map::monitor_at(&app.monitors, app.selected_monitor, world)
        {
            select_and_start_drag(app, index, world);
        }
        return;
    }

    if let Some(side) = layout.side_table.filter(|side| inside(*side, point)) {
        match app.mode {
            TUIMode::Resolution => {
                let total = app.monitors[app.selected_monitor].modes.len();
                if let Some(row) = Resolutions::row_at(side, app.selected_resolution, total, point.1)
                {
                    app.selected_resolution = row;
                }
            }
            TUIMode::Scale => {
                if let Some(row) = Scale::row_at(side, app.selected_scale, point.1) {
                    app.selected_scale = row;
                }
            }
            _ => {}
        }
        return;
    }

    if inside(layout.list, point)
        && let Some(row) =
            MonitorList::row_at(layout.list, &app.monitors, app.selected_monitor, point.1)
    {
        app.selected_monitor = row;
        // The leading checkbox column toggles the monitor instead of only
        // selecting it.
        if point.0 < MonitorList::icon_column_end(layout.list) {
            toggle_monitor(app, row);
        }
    }
}

fn select_and_start_drag(app: &mut App, index: usize, world: (f64, f64)) {
    app.selected_monitor = index;
    if let Some(position) = app.monitors[index].position.clone() {
        app.drag = Some(crate::app::state::DragState {
            monitor: index,
            grab: world,
            origin: (position.x, position.y),
            guides: Default::default(),
        });
    }
}

fn handle_left_drag(app: &mut App, layout: ScreenLayout, point: (u16, u16)) {
    let Some(drag) = app.drag.as_ref() else {
        return;
    };
    let monitor = drag.monitor;
    let grab = drag.grab;
    let origin = drag.origin;

    // Outside the canvas `world_at` still extrapolates, so the drag keeps
    // following the cursor; a degenerate canvas falls back to the grab point.
    let world = Map::world_at(layout.canvas, &app.monitors, point.0, point.1).unwrap_or(grab);
    let dx = (world.0 - grab.0).round() as i32;
    let dy = (world.1 - grab.1).round() as i32;
    if dx == 0 && dy == 0 {
        return;
    }

    if let Some(position) = app.monitors[monitor].position.as_mut() {
        position.x = origin.0 + dx;
        position.y = origin.1 + dy;
    }

    let guides = snap_to_alignment(&mut app.monitors, monitor, SNAP_THRESHOLD);
    if let Some(drag) = app.drag.as_mut() {
        drag.guides = guides;
    }
}

// Applies a wheel notch, wrapping like the keyboard navigation does.
fn handle_scroll(app: &mut App, layout: ScreenLayout, point: (u16, u16), delta: i32) {
    if app.monitors.is_empty() {
        return;
    }

    if layout.side_table.is_some_and(|side| inside(side, point)) {
        match app.mode {
            TUIMode::Resolution => {
                let length = app.monitors[app.selected_monitor].modes.len();
                app.selected_resolution = step(app.selected_resolution, length, delta);
            }
            TUIMode::Scale => {
                let length = ScaleValue::table().len();
                app.selected_scale = step(app.selected_scale, length, delta);
            }
            _ => {}
        }
        return;
    }

    if inside(layout.list, point) {
        app.selected_monitor = step(app.selected_monitor, app.monitors.len(), delta);
    }
}

fn step(current: usize, length: usize, delta: i32) -> usize {
    if delta < 0 {
        crate::actions::navigation::cycle_backward(current, length)
    } else {
        crate::actions::navigation::cycle_forward(current, length)
    }
}

fn toggle_monitor(app: &mut App, index: usize) {
    let monitor = &mut app.monitors[index];
    if monitor.enabled {
        crate::actions::monitor_actions::disable_monitor(monitor);
    } else {
        crate::actions::monitor_actions::enable_monitor(monitor);
    }
}

fn handle_preset(app: &mut App, area: Rect, event: MouseEvent) {
    match event.kind {
        MouseEventKind::ScrollUp => preview_scrolled(app, -1),
        MouseEventKind::ScrollDown => preview_scrolled(app, 1),
        MouseEventKind::Down(MouseButton::Left) => {
            let point = point(event);
            if !inside(PresetMenu::rect(area), point) {
                // Clicking outside cancels without applying.
                app.restore_preset_backup();
                app.show_preset_menu = None;
                return;
            }
            let index = app
                .show_preset_menu
                .as_ref()
                .and_then(|menu| menu.row_at(area, point.1));
            if let Some(index) = index {
                let name = app
                    .show_preset_menu
                    .as_mut()
                    .and_then(|menu| menu.select_index(index));
                if let Some(name) = name {
                    app.preview_preset(&name);
                }
            }
        }
        _ => {}
    }
}

fn preview_scrolled(app: &mut App, delta: i32) {
    let name = app
        .show_preset_menu
        .as_mut()
        .and_then(|menu| menu.scroll(delta));
    if let Some(name) = name {
        app.preview_preset(&name);
    }
}
