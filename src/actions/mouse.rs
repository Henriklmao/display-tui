//! Mouse event routing and hit-testing.
//!
//! Mouse events mirror the key handler's modal precedence: the help overlay,
//! popups and the preset menu capture input first, then the active mode
//! dispatches to the map, the side table and the monitor list.

use crate::App;
use crate::actions::movement::snap_to_alignment;
use crate::monitor::{Monitor, MonitorCanvas};
use crate::ui::components::{Map, MonitorList, PresetMenu, Resolutions, Scale};
use crate::ui::layouts::ScreenLayout;
use crate::utils::{ScaleValue, TUIMode};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

// World-space distance within which a dragged monitor snaps to alignment.
pub const SNAP_THRESHOLD: f64 = 24.0;

// Maximum gap between two clicks for them to count as a double-click.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

// Routes a mouse event to the component that owns the current view.
pub fn handle_mouse(app: &mut App, event: MouseEvent) {
    handle_mouse_at(app, event, Instant::now());
}

// Body of [`handle_mouse`] with an injectable clock so the double-click
// timing can be tested deterministically.
fn handle_mouse_at(app: &mut App, event: MouseEvent, now: Instant) {
    let layout = match app.layout {
        Some(layout) => layout,
        None => return,
    };

    // A release always ends the drag, even if a modal has taken over since the
    // press so the drag cannot stay armed behind an overlay.
    if matches!(event.kind, MouseEventKind::Up(MouseButton::Left)) {
        app.drag = None;
    }

    if app.show_help {
        if is_left_click(event.kind) {
            // A modal-consumed click must not seed a later double-click.
            app.last_click = None;
            app.show_help = false;
        }
        return;
    }

    if app.show_popup.is_some() {
        if is_left_click(event.kind) {
            // A click is the least deliberate gesture, so it only dismisses.
            // Accepting (which writes the Hyprland config) stays on Enter/f.
            app.last_click = None;
            app.show_popup = None;
        }
        return;
    }

    if app.show_preset_menu.is_some() {
        // The preset menu registers its own clicks (it supports double-click).
        handle_preset(app, layout.area, event, now);
        return;
    }

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => handle_left_down(app, layout, point(event), now),
        MouseEventKind::Drag(MouseButton::Left) => handle_left_drag(app, layout, point(event)),
        MouseEventKind::Up(MouseButton::Left) => app.drag = None,
        // The mouse's Apply: a right click resolves a pending move.
        MouseEventKind::Down(MouseButton::Right) => handle_right_click(app),
        MouseEventKind::ScrollUp => handle_scroll(app, layout, point(event), -1),
        MouseEventKind::ScrollDown => handle_scroll(app, layout, point(event), 1),
        _ => {}
    }
}

// Records a left click and reports whether it completes a double-click.
//
// A detected pair is consumed so a third quick click starts a fresh pair.
fn register_click(app: &mut App, point: (u16, u16), now: Instant) -> bool {
    let double = app.last_click.is_some_and(|(col, row, then)| {
        col.abs_diff(point.0) <= 1
            && row.abs_diff(point.1) <= 1
            && now.saturating_duration_since(then) <= DOUBLE_CLICK_WINDOW
    });
    app.last_click = if double {
        None
    } else {
        Some((point.0, point.1, now))
    };
    double
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

// Keeps the selected monitor index in range before indexing (defensive; the
// navigation paths already stay within bounds).
fn clamp_selection(app: &mut App) {
    app.selected_monitor = app
        .selected_monitor
        .min(app.monitors.len().saturating_sub(1));
}

fn handle_left_down(app: &mut App, layout: ScreenLayout, point: (u16, u16), now: Instant) {
    if app.monitors.is_empty() {
        return;
    }
    clamp_selection(app);
    let double_click = register_click(app, point, now);

    // The map selects a monitor and starts a drag in every mode.
    if inside(layout.canvas, point) {
        // Match the canvas the current frame drew: a frozen one while a drag
        // is active, otherwise the live bounds.
        let bounds = app
            .drag
            .as_ref()
            .map(|drag| drag.canvas)
            .unwrap_or_else(|| Monitor::get_monitors_canvas(&app.monitors, &layout.canvas));

        // The Apply button of a pending monitor takes precedence over dragging.
        let hit_apply = app.move_origins.keys().copied().any(|index| {
            let Some(monitor) = app.monitors.get(index).filter(|monitor| monitor.enabled) else {
                return false;
            };
            Map::apply_button_rect(layout.canvas, &bounds, monitor)
                .is_some_and(|rect| inside(rect, point))
        });
        if hit_apply {
            app.apply_move();
            return;
        }

        if let Some(world) = Map::world_at_in(layout.canvas, &bounds, point.0, point.1)
            && let Some(index) = Map::monitor_at(&app.monitors, app.selected_monitor, world)
        {
            select_and_start_drag(app, index, world, bounds);
        }
        return;
    }

    if let Some(side) = layout.side_table.filter(|side| inside(*side, point)) {
        match app.mode {
            TUIMode::Resolution => {
                let total = app.monitors[app.selected_monitor].modes.len();
                if let Some(row) =
                    Resolutions::row_at(side, app.selected_resolution, total, point.1)
                {
                    app.selected_resolution = row;
                    // A double-click applies the highlighted mode immediately.
                    if double_click {
                        app.monitors[app.selected_monitor].set_current_resolution(row);
                    }
                }
            }
            TUIMode::Scale => {
                if let Some(row) = Scale::row_at(side, app.selected_scale, point.1) {
                    app.selected_scale = row;
                    if double_click {
                        app.monitors[app.selected_monitor].scale =
                            Some(ScaleValue::table()[row].value);
                    }
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
        // `row` is resolved against the current render's scroll window, so a
        // click (and the toggle) always acts on the monitor actually drawn
        // under the cursor.
        let already_selected = app.selected_monitor == row;
        app.selected_monitor = row;
        // The checkbox toggles only once the row is already selected, so the
        // first click selects and the next one enables/disables.
        if already_selected && point.0 < MonitorList::icon_column_end(layout.list) {
            toggle_monitor(app, row);
            // Disabling the pending monitor drops its Apply button and its move.
            if !app.monitors[row].enabled {
                app.discard_move(row);
            }
        }
    }
}

// A right click confirms a pending move, mirroring the Enter/Space gate. It
// only acts in View: in the sub-modes those keys apply the highlighted value,
// and a right click must not commit the move behind the user's back.
fn handle_right_click(app: &mut App) {
    if app.mode == TUIMode::View && app.has_pending_move() {
        app.apply_move();
        // As with Enter/Space: a click during a drag ends the gesture too.
        app.drag = None;
    }
}

fn select_and_start_drag(app: &mut App, index: usize, world: (f64, f64), canvas: MonitorCanvas) {
    app.selected_monitor = index;
    let was_pending = app.move_origins.contains_key(&index);
    if let Some(position) = app.monitors[index].position.clone() {
        app.drag = Some(crate::app::state::DragState {
            monitor: index,
            grab: world,
            origin: (position.x, position.y),
            guides: Default::default(),
            canvas,
            was_pending,
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
    let canvas = drag.canvas;

    // The canvas bounds are frozen for the whole drag, so a column of cursor
    // movement is always the same world distance and the monitor tracks the
    // cursor instead of feeding its own motion back into the mapping.
    let world = Map::world_at_in(layout.canvas, &canvas, point.0, point.1).unwrap_or(grab);
    let dx = (world.0 - grab.0).round() as i32;
    let dy = (world.1 - grab.1).round() as i32;
    if dx == 0 && dy == 0 {
        return;
    }

    if let Some(position) = app.monitors[monitor].position.as_mut() {
        position.x = origin.0 + dx;
        position.y = origin.1 + dy;
    }
    // The move is not applied yet: remember where it started (first time only,
    // so a second drag keeps the original revert target) and show Apply.
    app.note_move(monitor, (origin.0, origin.1));

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
    clamp_selection(app);

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

fn handle_preset(app: &mut App, area: Rect, event: MouseEvent, now: Instant) {
    match event.kind {
        MouseEventKind::ScrollUp => preview_scrolled(app, -1),
        MouseEventKind::ScrollDown => preview_scrolled(app, 1),
        MouseEventKind::Down(MouseButton::Left) => {
            let point = point(event);
            if !inside(PresetMenu::rect(area), point) {
                // Clicking outside cancels; do not let it seed a double-click.
                app.last_click = None;
                app.restore_preset_backup();
                app.show_preset_menu = None;
                return;
            }
            let double_click = register_click(app, point, now);
            let index = app
                .show_preset_menu
                .as_ref()
                .and_then(|menu| menu.row_at(area, point.1));
            let Some(index) = index else { return };
            let name = app
                .show_preset_menu
                .as_mut()
                .and_then(|menu| menu.select_index(index));
            let Some(name) = name else { return };
            if double_click {
                // Applying closes the menu and commits the preset.
                app.apply_preset_click(&name);
            } else {
                app.preview_preset(&name);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::Popup;
    use crate::monitor::{Monitor, Position, Resolution};
    use crate::test_utils::tests::{TempDir, find_cell, find_text, test_monitors, text_of};
    use crate::ui::components::PresetMenu;
    use crate::ui::layouts::screen_layout;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::buffer::Buffer;
    use ratatui::widgets::Widget;
    use std::time::{Duration, Instant};

    // The arrow marker drawn in front of the selected monitor row.
    const MARKER: &str = "\u{f04b}";
    const ENABLED_ICON: &str = "\u{f00c}";
    const DISABLED_ICON: &str = "\u{f00d}";

    fn area() -> Rect {
        Rect::new(0, 0, 120, 40)
    }

    fn monitor(name: &str, x: i32, y: i32, enabled: bool) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled,
            position: Some(Position { x, y }),
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

    fn app_in(screen: Rect, mode: TUIMode, monitors: Vec<Monitor>) -> App {
        let mut app = App::new(None);
        app.monitors = monitors;
        app.mode = mode;
        app.layout = Some(screen_layout(screen, mode));
        app
    }

    fn app(mode: TUIMode, monitors: Vec<Monitor>) -> App {
        app_in(area(), mode, monitors)
    }

    fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn down(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::Down(MouseButton::Left), col, row));
    }

    fn drag(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::Drag(MouseButton::Left), col, row));
    }

    fn up(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::Up(MouseButton::Left), col, row));
    }

    fn right_click(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::Down(MouseButton::Right), col, row));
    }

    fn wheel_down(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::ScrollDown, col, row));
    }

    fn wheel_up(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::ScrollUp, col, row));
    }

    // A left-click at an explicit time, for double-click tests.
    fn down_at(a: &mut App, col: u16, row: u16, now: Instant) {
        handle_mouse_at(
            a,
            mouse(MouseEventKind::Down(MouseButton::Left), col, row),
            now,
        );
    }

    fn current_index(m: &Monitor) -> usize {
        m.modes
            .iter()
            .position(|mode| mode.current)
            .expect("a mode must be selected")
    }

    fn render(app: &App) -> Buffer {
        render_in(app, area())
    }

    fn render_in(app: &App, screen: Rect) -> Buffer {
        let mut buf = Buffer::empty(screen);
        Widget::render(app, screen, &mut buf);
        buf
    }

    fn pos(m: &Monitor) -> (i32, i32) {
        let p = m.position.clone().unwrap();
        (p.x, p.y)
    }

    // The first canvas cell that resolves to the given monitor.
    fn cell_for_monitor(app: &App, index: usize) -> (u16, u16) {
        let canvas = app.layout.unwrap().canvas;
        for row in canvas.top()..canvas.bottom() {
            for col in canvas.left()..canvas.right() {
                if let Some(world) = Map::world_at(canvas, &app.monitors, col, row)
                    && Map::monitor_at(&app.monitors, app.selected_monitor, world) == Some(index)
                {
                    return (col, row);
                }
            }
        }
        panic!("monitor {index} is not visible on the map");
    }

    // --- modal precedence ------------------------------------------------

    #[test]
    fn a_click_closes_the_help_overlay_and_is_consumed() {
        let mut a = app(TUIMode::View, test_monitors());
        a.show_help = true;

        wheel_down(&mut a, 40, 32);
        assert!(a.show_help, "the wheel must not dismiss help");
        assert_eq!(a.selected_monitor, 0);

        // A click on the second monitor row is swallowed by the overlay.
        let row = a.layout.unwrap().list.y + 4;
        down(&mut a, 10, row);
        assert!(!a.show_help, "the click closes help");
        assert_eq!(a.selected_monitor, 0, "the click must not fall through");
    }

    #[test]
    fn a_click_on_a_non_error_popup_dismisses_without_writing() {
        let dir = TempDir::new("mouse-popup-dismiss");
        let mut a = app(TUIMode::View, test_monitors());
        // Point the write targets at temp paths so a regression cannot touch
        // the real Hyprland config.
        a.state_path = Some(dir.path().join("monitor_state.json"));
        a.config.monitors_config_path = Some(
            dir.path()
                .join("monitors.conf")
                .to_string_lossy()
                .into_owned(),
        );
        a.show_popup = Some(Popup {
            title: " Preset Mismatch ".to_string(),
            lines: vec!["missing".to_string()],
            is_error: false,
            apply_preset: None,
            is_forceable: true,
        });

        down(&mut a, 60, 20);

        assert!(a.show_popup.is_none(), "the click dismisses the popup");
        assert!(
            !dir.path().join("monitors.conf").exists(),
            "a mouse click must not write the Hyprland config"
        );
    }

    #[test]
    fn a_click_closes_an_error_popup() {
        let mut a = app(TUIMode::View, test_monitors());
        a.show_popup = Some(Popup {
            title: " Error ".to_string(),
            lines: vec!["boom".to_string()],
            is_error: true,
            apply_preset: None,
            is_forceable: false,
        });

        wheel_down(&mut a, 40, 32);
        assert!(a.show_popup.is_some());

        down(&mut a, 60, 20);
        assert!(a.show_popup.is_none(), "the click dismisses the popup");
    }

    #[test]
    fn a_click_outside_the_preset_menu_closes_it() {
        let mut a = app(TUIMode::View, test_monitors());
        a.preset_backup = Some(vec![]);
        a.show_preset_menu = Some(PresetMenu::new(
            vec!["missing-a".to_string(), "missing-b".to_string()],
            &[],
            None,
        ));

        down(&mut a, 0, 0);
        assert!(
            a.show_preset_menu.is_none(),
            "outside click closes the menu"
        );
    }

    #[test]
    fn a_click_on_a_preset_row_selects_and_previews_it() {
        let mut a = app(TUIMode::View, test_monitors());
        a.preset_backup = Some(vec![]);
        a.show_preset_menu = Some(PresetMenu::new(
            vec!["missing-a".to_string(), "missing-b".to_string()],
            &[],
            None,
        ));

        let top = PresetMenu::rect(area()).y + 2;
        down(&mut a, 40, top + 1);

        assert_eq!(
            a.show_preset_menu.as_ref().unwrap().selected_index,
            1,
            "the clicked row is selected"
        );
        assert!(a.show_preset_menu.is_some(), "the menu stays open");
    }

    #[test]
    fn the_wheel_scrolls_the_preset_list_without_wrapping() {
        let mut a = app(TUIMode::View, test_monitors());
        a.preset_backup = Some(vec![]);
        a.show_preset_menu = Some(PresetMenu::new(
            vec!["missing-a".to_string(), "missing-b".to_string()],
            &[],
            None,
        ));

        let row = PresetMenu::rect(area()).y + 3;
        wheel_down(&mut a, 40, row);
        assert_eq!(a.show_preset_menu.as_ref().unwrap().selected_index, 1);

        wheel_down(&mut a, 40, row);
        assert_eq!(a.show_preset_menu.as_ref().unwrap().selected_index, 1);

        wheel_up(&mut a, 40, row);
        assert_eq!(a.show_preset_menu.as_ref().unwrap().selected_index, 0);
    }

    // --- map: selection and dragging ------------------------------------

    #[test]
    fn clicking_a_monitor_on_the_map_selects_it() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 1920, 0, true)];
        let mut a = app(TUIMode::View, monitors);

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);

        assert_eq!(a.selected_monitor, 1);
    }

    #[test]
    fn a_press_starts_a_drag_and_release_ends_it() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 1920, 0, true)];
        let mut a = app(TUIMode::View, monitors);

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);

        let drag_state = a.drag.as_ref().expect("press starts a drag");
        assert_eq!(drag_state.monitor, 1);
        assert_eq!(drag_state.origin, (1920, 0));

        up(&mut a, col, row);
        assert!(a.drag.is_none(), "release ends the drag");
    }

    #[test]
    fn dragging_moves_the_monitor_in_any_mode() {
        for mode in [TUIMode::View, TUIMode::Move, TUIMode::Resolution] {
            let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
            let mut a = app(mode, monitors);

            let (col, row) = cell_for_monitor(&a, 1);
            down(&mut a, col, row);
            drag(&mut a, col + 6, row);

            assert!(
                pos(&a.monitors[1]).0 > 3000,
                "{mode:?}: dragging right must move B right"
            );
            assert_eq!(
                pos(&a.monitors[1]).1,
                1000,
                "{mode:?}: the row did not change"
            );
            assert_eq!(pos(&a.monitors[0]), (0, 0), "{mode:?}: A must not move");
        }
    }

    #[test]
    fn dragging_follows_the_cursor_vertically() {
        // Dragging the cursor down must move the monitor down, not up.
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 3000, true)];
        let mut a = app(TUIMode::View, monitors);
        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);

        drag(&mut a, col, row + 2);
        let down_pos = pos(&a.monitors[1]);
        assert!(
            down_pos.1 > 3000,
            "dragging down moves it down: {down_pos:?}"
        );
        assert_eq!(down_pos.0, 3000, "the column did not change");

        let (col, row) = cell_for_monitor(&a, 1);
        drag(&mut a, col, row - 4);
        let up_pos = pos(&a.monitors[1]);
        assert!(up_pos.1 < down_pos.1, "dragging up moves it up: {up_pos:?}");
    }

    #[test]
    fn a_click_maps_to_the_monitor_drawn_under_the_cursor() {
        // Upper (y = -1080) is drawn above Lower (y = 0); a click on each
        // label must select the monitor actually rendered there.
        let monitors = vec![
            monitor("Upper", 0, -1080, true),
            monitor("Lower", 0, 0, true),
        ];
        let mut a = app(TUIMode::View, monitors);
        let buf = render(&a);
        let (lx, ly) = find_text(&buf, "Lower").expect("lower label");
        let (ux, uy) = find_text(&buf, "Upper").expect("upper label");

        down(&mut a, lx, ly);
        assert_eq!(a.selected_monitor, 1, "the lower label selects Lower");

        down(&mut a, ux, uy);
        assert_eq!(a.selected_monitor, 0, "the upper label selects Upper");
    }

    #[test]
    fn dragging_snaps_to_the_nearest_alignment() {
        // A single monitor near the origin: a one-column drag lands within the
        // snap threshold of the origin, so it is pulled back onto it.
        let mut a = app(TUIMode::View, vec![monitor("A", 0, 0, true)]);

        let (col, row) = cell_for_monitor(&a, 0);
        down(&mut a, col, row);
        drag(&mut a, col + 1, row);

        assert_eq!(
            pos(&a.monitors[0]),
            (0, 0),
            "the drag snapped onto the origin"
        );
        let guides = a.drag.as_ref().unwrap().guides;
        assert_eq!(guides.x, Some(0.0), "a vertical guide is shown");
        assert_eq!(guides.y, Some(0.0), "a horizontal guide is shown");
    }

    #[test]
    fn dragging_far_from_any_alignment_moves_freely() {
        let mut a = app(TUIMode::View, vec![monitor("A", 0, 300, true)]);

        let (col, row) = cell_for_monitor(&a, 0);
        down(&mut a, col, row);
        drag(&mut a, col + 30, row);

        let (x, y) = pos(&a.monitors[0]);
        assert!(x > 200, "the monitor moved freely to x = {x}");
        assert_eq!(y, 300);
        assert_eq!(
            a.drag.as_ref().unwrap().guides,
            crate::actions::movement::SnapGuides::default(),
            "no guides when nothing is near"
        );
    }

    #[test]
    fn dragging_snaps_a_neighbour_edge_on_a_wide_map() {
        // A wide terminal makes one column a small world step, so a one-column
        // drag lands close enough to A's right edge to snap onto it.
        let screen = Rect::new(0, 0, 240, 40);
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 1900, 0, true)];
        let mut a = app_in(screen, TUIMode::View, monitors);

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 1, row);

        assert_eq!(pos(&a.monitors[1]).0, 1920, "B snapped onto A's right edge");
        assert_eq!(a.drag.as_ref().unwrap().guides.x, Some(1920.0));
    }

    #[test]
    fn a_press_and_release_without_movement_keeps_the_position() {
        let mut a = app(TUIMode::View, vec![monitor("A", 100, 100, true)]);

        let (col, row) = cell_for_monitor(&a, 0);
        down(&mut a, col, row);
        up(&mut a, col, row);

        assert_eq!(pos(&a.monitors[0]), (100, 100));
        assert!(a.drag.is_none());
    }

    #[test]
    fn drag_events_after_a_release_are_ignored() {
        let mut a = app(TUIMode::View, vec![monitor("A", 100, 100, true)]);

        let (col, row) = cell_for_monitor(&a, 0);
        down(&mut a, col, row);
        up(&mut a, col, row);
        drag(&mut a, col + 20, row);

        assert_eq!(pos(&a.monitors[0]), (100, 100), "no drag is in progress");
    }

    #[test]
    fn a_release_ends_the_drag_even_when_a_modal_is_open() {
        let mut a = app(TUIMode::View, vec![monitor("A", 0, 0, true)]);

        let (col, row) = cell_for_monitor(&a, 0);
        down(&mut a, col, row);
        assert!(a.drag.is_some());

        a.show_help = true;
        up(&mut a, col, row);

        assert!(
            a.drag.is_none(),
            "the overlay must not leave the drag armed"
        );
    }

    #[test]
    fn dragging_steps_linearly_without_position_feedback() {
        // Each one-column drag must advance by the same world distance. If the
        // canvas bounds were recomputed from the moving monitor, the steps
        // would accelerate.
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 1920, 0, true)];
        let mut a = app(TUIMode::View, monitors);
        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);

        drag(&mut a, col + 1, row);
        let first = pos(&a.monitors[1]).0 - 1920;
        drag(&mut a, col + 2, row);
        let second = pos(&a.monitors[1]).0 - 1920;

        assert!(first > 0, "the monitor moved: {first}");
        assert!(
            (second - 2 * first).abs() <= 2,
            "steps must be linear: {first} then {second}"
        );
    }

    #[test]
    fn dragging_a_monitor_without_a_position_does_nothing() {
        let mut unplaced = monitor("A", 0, 0, true);
        unplaced.position = None;
        let mut a = app(TUIMode::View, vec![unplaced]);

        // No cell resolves to a placeless monitor, so the press is a no-op.
        down(&mut a, area().x + 2, area().y + 2);
        assert!(a.drag.is_none());
        assert_eq!(a.monitors[0].position, None);
    }

    // --- wheel: display list --------------------------------------------

    #[test]
    fn the_wheel_steps_the_display_list_exactly_once() {
        let mut a = app(TUIMode::View, test_monitors());
        let list = a.layout.unwrap().list;
        let (col, row) = (list.x + 5, list.y + 4);

        wheel_down(&mut a, col, row);
        assert_eq!(a.selected_monitor, 1, "one notch moves one row down");

        wheel_down(&mut a, col, row);
        assert_eq!(a.selected_monitor, 0, "the list wraps");

        wheel_up(&mut a, col, row);
        assert_eq!(a.selected_monitor, 1, "up moves one row back");
    }

    // Regression for the reported scroll bug: the arrow indicator must sit on
    // the selected monitor's row, with no phantom extra step.
    #[test]
    fn the_arrow_indicator_tracks_the_selected_row_after_scrolling() {
        let mut a = app(TUIMode::View, test_monitors());
        let list = a.layout.unwrap().list;

        let buf = render(&a);
        let (_, first_row) = find_text(&buf, "Description 1").unwrap();
        let (_, marker) = find_cell(&buf, MARKER).unwrap();
        assert_eq!(marker, first_row, "the marker starts on the first monitor");

        wheel_down(&mut a, list.x + 5, list.y + 4);

        let buf = render(&a);
        let (_, second_row) = find_text(&buf, "Description 2").unwrap();
        let (_, marker) = find_cell(&buf, MARKER).unwrap();
        assert_eq!(marker, second_row, "the marker follows the selection down");
    }

    #[test]
    fn the_wheel_over_the_map_does_nothing() {
        let mut a = app(TUIMode::View, test_monitors());
        let canvas = a.layout.unwrap().canvas;

        wheel_down(&mut a, canvas.x + 5, canvas.y + 5);

        assert_eq!(a.selected_monitor, 0);
    }

    // --- wheel: resolution and scale tables -----------------------------

    #[test]
    fn the_wheel_scrolls_the_resolution_table() {
        let mut a = app(TUIMode::Resolution, test_monitors());
        let side = a.layout.unwrap().side_table.unwrap();
        let (col, row) = (side.x + 3, side.y + 6);

        wheel_down(&mut a, col, row);
        assert_eq!(a.selected_resolution, 1);

        wheel_down(&mut a, col, row);
        assert_eq!(a.selected_resolution, 0, "wraps");

        wheel_up(&mut a, col, row);
        assert_eq!(a.selected_resolution, 1);
    }

    #[test]
    fn the_wheel_scrolls_the_scale_table() {
        let mut a = app(TUIMode::Scale, test_monitors());
        let side = a.layout.unwrap().side_table.unwrap();
        let (col, row) = (side.x + 3, side.y + 6);

        wheel_down(&mut a, col, row);
        assert_eq!(a.selected_scale, 1);

        wheel_up(&mut a, col, row);
        assert_eq!(a.selected_scale, 0);
    }

    // --- table clicks ----------------------------------------------------

    #[test]
    fn clicking_a_display_row_selects_that_monitor() {
        let mut a = app(TUIMode::View, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "Monitor 2").expect("second display row");

        down(&mut a, col, row);

        assert_eq!(a.selected_monitor, 1);
    }

    #[test]
    fn clicks_still_land_correctly_with_the_duplicate_workspace_warning() {
        // The duplicate-workspace warning steals a list row, shifting the
        // table geometry; clicks must account for it.
        let mut monitors = test_monitors();
        for monitor in &mut monitors {
            monitor.workspace = Some(1);
        }
        let mut a = app(TUIMode::View, monitors);
        let buf = render(&a);
        let (col, row) = find_text(&buf, "Description 2").expect("second display row");

        down(&mut a, col, row);

        assert_eq!(a.selected_monitor, 1);
    }

    #[test]
    fn clicking_a_scrolled_resolution_row_selects_the_right_index() {
        // Plenty of modes in a table shorter than the list force a scroll
        // offset; the click must be mapped through it.
        let modes: Vec<Resolution> = (0..30)
            .map(|i| Resolution {
                width: 2000 - i * 10,
                height: 1000,
                refresh: 60.0,
                preferred: i == 0,
                current: i == 0,
            })
            .collect();
        let mut m = monitor("A", 0, 0, true);
        m.modes = modes;
        let screen = Rect::new(0, 0, 200, 20);
        let mut a = app_in(screen, TUIMode::Resolution, vec![m]);
        a.selected_resolution = 20; // scrolls the window down

        let buf = render_in(&a, screen);
        // Mode 11 (1890x1000) is the first visible row of the scrolled window.
        let (col, row) = find_text(&buf, "1890x1000").expect("scrolled mode row");

        down(&mut a, col, row);

        assert_eq!(
            a.selected_resolution, 11,
            "the click maps through the scroll offset"
        );
    }

    #[test]
    fn the_checkbox_selects_on_the_first_click_and_toggles_on_the_next() {
        let mut a = app(TUIMode::View, test_monitors());

        // Monitor 2 (second row) is not selected yet, so clicking its checkbox
        // only selects it.
        let buf = render(&a);
        let (col, row) = find_cell(&buf, DISABLED_ICON).expect("disabled icon");
        down(&mut a, col, row);
        assert_eq!(a.selected_monitor, 1, "the first click selects the row");
        assert!(!a.monitors[1].enabled, "the first click must not toggle");

        // The row is now selected: the next click toggles it.
        down(&mut a, col, row);
        assert!(a.monitors[1].enabled, "the second click enables it");
        assert_eq!(a.selected_monitor, 1);
    }

    #[test]
    fn the_checkbox_of_an_already_selected_row_toggles_immediately() {
        let mut a = app(TUIMode::View, test_monitors()); // monitor 0 selected + enabled

        let buf = render(&a);
        let (col, row) = find_cell(&buf, ENABLED_ICON).expect("enabled icon");
        down(&mut a, col, row);

        assert!(
            !a.monitors[0].enabled,
            "clicking the selected row toggles it"
        );
    }

    #[test]
    fn clicking_the_name_column_never_toggles() {
        let mut a = app(TUIMode::View, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "Description 1").expect("name column");

        down(&mut a, col, row);

        assert_eq!(a.selected_monitor, 0);
        assert!(a.monitors[0].enabled, "the name column only selects");
    }

    #[test]
    fn clicking_the_list_header_or_border_does_nothing() {
        let mut a = app(TUIMode::View, test_monitors());
        a.selected_monitor = 1;
        let list = a.layout.unwrap().list;

        down(&mut a, list.x + 5, list.y); // top border
        down(&mut a, list.x + 5, list.y + 1); // header row

        assert_eq!(a.selected_monitor, 1, "clicks outside the rows are ignored");
    }

    #[test]
    fn clicking_a_resolution_row_selects_without_applying() {
        let mut a = app(TUIMode::Resolution, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "1280x720").expect("second mode row");

        down(&mut a, col, row);

        assert_eq!(a.selected_resolution, 1, "the row is highlighted");
        assert_eq!(
            a.mode,
            TUIMode::Resolution,
            "the click does not leave the mode"
        );
        assert!(a.monitors[0].modes[0].current, "nothing was applied yet");
    }

    #[test]
    fn clicking_a_scale_row_selects_without_applying() {
        let mut a = app(TUIMode::Scale, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "125%").expect("scale row");

        down(&mut a, col, row);

        let expected = ScaleValue::table()
            .iter()
            .position(|entry| entry.name == "125%")
            .unwrap();
        assert_eq!(a.selected_scale, expected);
        assert_eq!(a.monitors[0].scale, Some(1.0), "nothing was applied yet");
    }

    #[test]
    fn clicking_the_list_in_a_sub_mode_selects_the_monitor_only() {
        let mut a = app(TUIMode::Resolution, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "Monitor 2").expect("second display row");

        down(&mut a, col, row);

        assert_eq!(a.selected_monitor, 1);
        assert_eq!(
            a.selected_resolution, 0,
            "the resolution selection is untouched"
        );
    }

    // --- applying a mouse move ------------------------------------------

    #[test]
    fn dragging_marks_the_monitor_as_pending_apply() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);
        assert!(a.move_origins.is_empty());

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);

        assert!(a.move_origins.contains_key(&1));
        assert_eq!(
            a.move_origins.get(&1),
            Some(&(3000, 1000)),
            "the pre-drag position is recorded for Cancel/Revert"
        );
    }

    #[test]
    fn re_dragging_a_monitor_keeps_the_original_revert_target() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);

        for step in [6, 12] {
            let (col, row) = cell_for_monitor(&a, 1);
            down(&mut a, col, row);
            drag(&mut a, col + step, row);
            up(&mut a, col, row);
        }

        assert_eq!(
            a.move_origins.get(&1),
            Some(&(3000, 1000)),
            "a second drag must not lose the original revert target"
        );
    }

    // The user's scenario end to end: drag, release (the Apply button stays),
    // then press Escape through the real key path.
    #[test]
    fn escape_after_a_drag_reverts_the_released_monitor() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);
        assert!(a.move_origins.contains_key(&1), "the move is pending");

        a.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(
            a.monitors[1].position,
            Some(Position { x: 3000, y: 1000 }),
            "Escape puts the released monitor back"
        );
        assert!(a.move_origins.is_empty(), "and clears the pending move");
    }

    #[test]
    fn the_apply_button_is_only_drawn_for_a_pending_move() {
        let mut a = app(TUIMode::View, test_monitors());
        assert!(
            !text_of(&render(&a)).contains("Apply"),
            "no button initially"
        );

        a.note_move(0, (0, 0));
        assert!(
            text_of(&render(&a)).contains("Apply"),
            "the button appears once a move is pending"
        );
    }

    #[test]
    fn clicking_the_apply_button_persists_the_move() {
        let dir = TempDir::new("mouse-apply-button");
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);
        a.state_path = Some(dir.path().join("monitor_state.json"));

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);
        assert!(a.move_origins.contains_key(&1), "the move is pending");

        // The Apply button sits in the monitor's centre; click it.
        let canvas = a.layout.unwrap().canvas;
        let bounds = Monitor::get_monitors_canvas(&a.monitors, &canvas);
        let rect =
            Map::apply_button_rect(canvas, &bounds, &a.monitors[1]).expect("apply button rect");
        down(&mut a, rect.x, rect.y);

        assert!(a.move_origins.is_empty(), "apply clears the pending move");
        assert!(
            dir.path().join("monitor_state.json").exists(),
            "apply persists the layout"
        );
    }

    #[test]
    fn enter_and_space_apply_a_pending_move_in_view_mode() {
        // Driven through App::handle_key_event: the confirm gate, not the list,
        // resolves a pending move.
        for key in [KeyCode::Enter, KeyCode::Char(' ')] {
            let dir = TempDir::new("mouse-apply-key");
            let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
            let mut a = app(TUIMode::View, monitors);
            a.state_path = Some(dir.path().join("monitor_state.json"));

            let (col, row) = cell_for_monitor(&a, 1);
            down(&mut a, col, row);
            drag(&mut a, col + 6, row);
            up(&mut a, col, row);
            assert!(a.move_origins.contains_key(&1));

            a.handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));

            assert!(!a.has_pending_move(), "{key:?} applies the move");
            assert!(
                dir.path().join("monitor_state.json").exists(),
                "{key:?} persists the layout"
            );
        }
    }

    #[test]
    fn right_clicking_applies_a_pending_move_in_view_mode() {
        let dir = TempDir::new("mouse-apply-right-click");
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);
        a.state_path = Some(dir.path().join("monitor_state.json"));

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);
        assert!(a.has_pending_move());
        let moved = a.monitors[1].position.clone();

        right_click(&mut a, col, row);

        assert!(!a.has_pending_move(), "the right-click applies the move");
        assert_eq!(
            a.monitors[1].position, moved,
            "the right-click keeps the moved position instead of reverting it"
        );
        assert!(
            dir.path().join("monitor_state.json").exists(),
            "and persists the layout"
        );
    }

    #[test]
    fn a_right_click_without_a_pending_move_changes_nothing() {
        let dir = TempDir::new("mouse-right-click-idle");
        let mut a = app(TUIMode::View, test_monitors());
        a.state_path = Some(dir.path().join("monitor_state.json"));

        let row = a.layout.unwrap().list.y + 4;
        right_click(&mut a, 10, row);

        assert_eq!(a.selected_monitor, 0, "a right click must not select");
        assert!(!a.has_pending_move());
        assert!(
            !dir.path().join("monitor_state.json").exists(),
            "an idle right click must not persist anything"
        );
    }

    #[test]
    fn a_right_click_keeps_a_pending_move_in_a_sub_mode() {
        // In Resolution/Scale mode Enter/Space apply the highlighted row, so the
        // right click must not commit the pending move behind the user's back.
        let dir = TempDir::new("mouse-right-click-submode");
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::Resolution, monitors);
        a.state_path = Some(dir.path().join("monitor_state.json"));
        a.note_move(1, (3000, 1000));

        right_click(&mut a, 40, 20);

        assert!(a.has_pending_move(), "the sub-mode keeps the move pending");
        assert!(
            !dir.path().join("monitor_state.json").exists(),
            "and nothing is persisted"
        );
    }

    #[test]
    fn a_right_click_does_not_apply_behind_an_open_help_overlay() {
        let dir = TempDir::new("mouse-right-click-help");
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);
        a.state_path = Some(dir.path().join("monitor_state.json"));
        a.note_move(1, (3000, 1000));
        a.show_help = true;

        right_click(&mut a, 40, 20);

        assert!(a.show_help, "the overlay keeps precedence");
        assert!(a.has_pending_move(), "and the move stays pending");
    }

    #[test]
    fn clicking_the_apply_button_does_not_start_a_drag() {
        let dir = TempDir::new("mouse-button-nodrag");
        let mut a = app(TUIMode::View, vec![monitor("A", 0, 0, true)]);
        a.state_path = Some(dir.path().join("monitor_state.json"));
        a.note_move(0, (0, 0));

        let canvas = a.layout.unwrap().canvas;
        let bounds = Monitor::get_monitors_canvas(&a.monitors, &canvas);
        let rect = Map::apply_button_rect(canvas, &bounds, &a.monitors[0]).unwrap();
        down(&mut a, rect.x, rect.y);

        assert!(a.drag.is_none(), "the button click must not start a drag");
        assert!(a.move_origins.is_empty(), "and it applies the move");
    }

    #[test]
    fn clicking_the_body_of_a_pending_monitor_starts_a_drag() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);

        // Move B so it is pending, then press its body away from the button.
        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);
        assert!(a.move_origins.contains_key(&1));

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        assert!(a.drag.is_some(), "pressing the monitor body drags it");
    }

    // --- double click applies -------------------------------------------

    #[test]
    fn the_double_click_detector_needs_a_close_repeat() {
        let mut a = app(TUIMode::View, test_monitors());
        let t0 = Instant::now();

        assert!(!register_click(&mut a, (10, 10), t0), "first click");
        assert!(
            register_click(&mut a, (10, 10), t0 + Duration::from_millis(100)),
            "quick repeat"
        );
        // The pair is consumed, so a third quick click starts fresh.
        assert!(
            !register_click(&mut a, (10, 10), t0 + Duration::from_millis(150)),
            "third click is not a double"
        );
        // Too slow to pair.
        assert!(
            !register_click(&mut a, (10, 10), t0 + Duration::from_millis(1000)),
            "slow click"
        );
        // A different cell within the window does not pair.
        assert!(
            !register_click(&mut a, (40, 10), t0 + Duration::from_millis(1050)),
            "far cell"
        );
        // A one-cell drift still pairs (hand tremor).
        assert!(!register_click(
            &mut a,
            (0, 0),
            t0 + Duration::from_millis(2000)
        ));
        assert!(register_click(
            &mut a,
            (1, 1),
            t0 + Duration::from_millis(2050)
        ));
    }

    #[test]
    fn double_clicking_a_resolution_row_applies_it() {
        let mut a = app(TUIMode::Resolution, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "1280x720").expect("second mode row");
        let t0 = Instant::now();

        down_at(&mut a, col, row, t0);
        assert_eq!(a.selected_resolution, 1);
        assert!(
            a.monitors[0].modes[0].current,
            "a single click does not apply"
        );

        down_at(&mut a, col, row, t0 + Duration::from_millis(120));
        assert_eq!(current_index(&a.monitors[0]), 1, "the double-click applies");
        assert_eq!(a.mode, TUIMode::Resolution, "and stays in the mode");
    }

    #[test]
    fn double_clicking_a_scale_row_applies_it() {
        let mut a = app(TUIMode::Scale, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "125%").expect("scale row");
        let expected = ScaleValue::table()
            .iter()
            .position(|entry| entry.name == "125%")
            .unwrap();
        let t0 = Instant::now();

        down_at(&mut a, col, row, t0);
        assert_eq!(a.selected_scale, expected);
        assert_eq!(a.monitors[0].scale, Some(1.0), "a single click keeps it");

        down_at(&mut a, col, row, t0 + Duration::from_millis(120));
        assert_eq!(
            a.monitors[0].scale,
            Some(ScaleValue::table()[expected].value)
        );
    }

    #[test]
    fn double_clicking_a_preset_row_applies_it() {
        let dir = TempDir::new("mouse-preset-apply");
        // A real preset in an injected presets directory.
        let presets = dir.path().join("presets");
        std::fs::create_dir_all(&presets).unwrap();
        let state = vec![crate::config::MonitorState {
            name: "Monitor 1".to_string(),
            enabled: true,
            position: Some(Position { x: 111, y: 0 }),
            scale: Some(1.0),
            workspace: None,
            rotation: None,
            resolution: None,
        }];
        std::fs::write(
            presets.join("desk.json"),
            serde_json::to_string(&state).unwrap(),
        )
        .unwrap();

        let mut a = app(TUIMode::View, test_monitors());
        a.state_path = Some(dir.path().join("monitor_state.json"));
        a.presets_dir = Some(presets);
        a.preset_backup = Some(vec![]);
        a.show_preset_menu = Some(PresetMenu::new(vec!["desk".to_string()], &[], None));
        let area = area();
        let top = PresetMenu::rect(area).y + 2;
        let t0 = Instant::now();

        down_at(&mut a, 40, top, t0);
        assert!(
            a.show_preset_menu.is_some(),
            "a single click keeps the menu open"
        );

        down_at(&mut a, 40, top, t0 + Duration::from_millis(120));
        assert!(
            a.show_preset_menu.is_none(),
            "the double-click applies and closes"
        );
        assert_eq!(a.active_preset.as_deref(), Some("desk"));
        assert_eq!(
            a.monitors[0].position,
            Some(Position { x: 111, y: 0 }),
            "the preset was applied"
        );
        assert!(dir.path().join("monitor_state.json").exists());
    }

    #[test]
    fn double_clicking_a_missing_preset_keeps_the_menu_open() {
        let mut a = app(TUIMode::View, test_monitors());
        a.presets_dir = Some(std::path::PathBuf::from("/nonexistent-display-tui-presets"));
        a.preset_backup = Some(vec![]);
        a.show_preset_menu = Some(PresetMenu::new(vec!["ghost".to_string()], &[], None));
        let area = area();
        let top = PresetMenu::rect(area).y + 2;
        let t0 = Instant::now();

        down_at(&mut a, 40, top, t0);
        down_at(&mut a, 40, top, t0 + Duration::from_millis(120));

        assert!(
            a.show_preset_menu.is_some(),
            "a failed apply keeps the menu open"
        );
        assert!(a.active_preset.is_none(), "and marks nothing active");
    }

    #[test]
    fn dismissing_a_modal_does_not_seed_a_double_click() {
        let mut a = app(TUIMode::Resolution, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "1280x720").expect("second mode row");

        // A click used to dismiss the help overlay must not pair with the next.
        a.show_help = true;
        let t0 = Instant::now();
        down_at(&mut a, col, row, t0);
        assert!(!a.show_help, "the click closes help");
        assert!(a.monitors[0].modes[0].current, "and applies nothing");

        down_at(&mut a, col, row, t0 + Duration::from_millis(50));
        assert_eq!(a.selected_resolution, 1, "the next click only selects");
        assert!(a.monitors[0].modes[0].current, "no bleed-through apply");
    }

    #[test]
    fn applying_a_move_persists_state_but_not_the_hyprland_config() {
        let dir = TempDir::new("mouse-apply-nowrite");
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);
        a.state_path = Some(dir.path().join("monitor_state.json"));
        a.config.monitors_config_path = Some(
            dir.path()
                .join("monitors.conf")
                .to_string_lossy()
                .into_owned(),
        );

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);

        let canvas = a.layout.unwrap().canvas;
        let bounds = Monitor::get_monitors_canvas(&a.monitors, &canvas);
        let rect = Map::apply_button_rect(canvas, &bounds, &a.monitors[1]).unwrap();
        down(&mut a, rect.x, rect.y);

        assert!(
            dir.path().join("monitor_state.json").exists(),
            "apply persists the state file"
        );
        assert!(
            !dir.path().join("monitors.conf").exists(),
            "apply must not write the Hyprland config"
        );
    }

    // `MonitorList::disable_monitor` keeps the App invariant on its own, so it
    // is exercised through the component even though the View confirm gate
    // normally handles keys first (see the keyboard-ignored test below).
    #[test]
    fn disabling_a_pending_monitor_clears_its_move() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);
        assert!(a.move_origins.contains_key(&1));

        a.selected_monitor = 1;
        MonitorList::handle_events(
            &mut a,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        );

        assert!(!a.monitors[1].enabled);
        assert!(
            a.move_origins.is_empty(),
            "a disabled monitor drops its recorded move"
        );
    }

    #[test]
    fn keyboard_keys_other_than_apply_or_revert_are_ignored_while_pending() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);
        assert!(a.move_origins.contains_key(&1));

        a.selected_monitor = 1;
        for key in [KeyCode::Char('d'), KeyCode::Char('m'), KeyCode::Char('o')] {
            a.handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));
        }

        assert!(
            a.monitors[1].enabled,
            "d must not disable a pending monitor"
        );
        assert_eq!(a.mode, TUIMode::View, "m must not open a sub-mode");
        assert!(
            a.move_origins.contains_key(&1),
            "the move is still unresolved"
        );
    }

    #[test]
    fn disabling_a_pending_monitor_by_mouse_clears_the_move() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 3000, 1000, true)];
        let mut a = app(TUIMode::View, monitors);

        let (col, row) = cell_for_monitor(&a, 1);
        down(&mut a, col, row);
        drag(&mut a, col + 6, row);
        up(&mut a, col, row);
        assert!(a.move_origins.contains_key(&1));

        // The pending row is selected already, so clicking its checkbox in the
        // list disables it. The first enabled icon is monitor A's row, and the
        // pending monitor B is the table row directly below it.
        let buf = render(&a);
        let (icon_col, first_row) = find_cell(&buf, ENABLED_ICON).expect("enabled icon");
        down(&mut a, icon_col, first_row + 1);

        assert!(!a.monitors[1].enabled, "the click disabled the monitor");
        assert!(a.move_origins.is_empty());
    }

    #[test]
    fn a_slow_second_click_does_not_apply() {
        let mut a = app(TUIMode::Resolution, test_monitors());
        let buf = render(&a);
        let (col, row) = find_text(&buf, "1280x720").expect("second mode row");
        let t0 = Instant::now();

        down_at(&mut a, col, row, t0);
        down_at(&mut a, col, row, t0 + Duration::from_millis(900));

        assert_eq!(a.selected_resolution, 1);
        assert!(a.monitors[0].modes[0].current, "too slow, so no apply");
    }

    // --- robustness ------------------------------------------------------

    #[test]
    fn mouse_without_a_recorded_layout_is_ignored() {
        let mut a = App::new(None);
        a.monitors = test_monitors();

        down(&mut a, 5, 5);
        wheel_down(&mut a, 5, 5);

        assert_eq!(a.selected_monitor, 0);
    }

    #[test]
    fn mouse_with_no_monitors_is_safe() {
        let mut a = app(TUIMode::View, vec![]);

        down(&mut a, 5, 5);
        drag(&mut a, 6, 6);
        up(&mut a, 6, 6);
        wheel_down(&mut a, 5, 5);
        wheel_up(&mut a, 5, 5);

        assert_eq!(a.selected_monitor, 0);
    }

    #[test]
    fn non_left_buttons_and_movement_are_ignored() {
        let monitors = vec![monitor("A", 0, 0, true), monitor("B", 1920, 0, true)];
        let mut a = app(TUIMode::View, monitors);
        let (col, row) = cell_for_monitor(&a, 1);

        handle_mouse(
            &mut a,
            mouse(MouseEventKind::Down(MouseButton::Right), col, row),
        );
        handle_mouse(&mut a, mouse(MouseEventKind::Moved, col, row));
        handle_mouse(&mut a, mouse(MouseEventKind::ScrollLeft, col, row));
        handle_mouse(&mut a, mouse(MouseEventKind::ScrollRight, col, row));

        assert_eq!(a.selected_monitor, 0);
        assert!(a.drag.is_none());
    }
}
