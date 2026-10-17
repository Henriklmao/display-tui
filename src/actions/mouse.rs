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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::Popup;
    use crate::monitor::{Monitor, Position, Resolution};
    use crate::test_utils::tests::{find_cell, find_text, test_monitors};
    use crate::ui::components::PresetMenu;
    use crate::ui::layouts::screen_layout;
    use crossterm::event::KeyModifiers;
    use ratatui::buffer::Buffer;
    use ratatui::widgets::Widget;

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

    fn wheel_down(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::ScrollDown, col, row));
    }

    fn wheel_up(a: &mut App, col: u16, row: u16) {
        handle_mouse(a, mouse(MouseEventKind::ScrollUp, col, row));
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
        assert!(a.show_preset_menu.is_none(), "outside click closes the menu");
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
            assert_eq!(pos(&a.monitors[1]).1, 1000, "{mode:?}: the row did not change");
            assert_eq!(pos(&a.monitors[0]), (0, 0), "{mode:?}: A must not move");
        }
    }

    #[test]
    fn dragging_snaps_to_the_nearest_alignment() {
        // A single monitor near the origin: a one-column drag lands within the
        // snap threshold of the origin, so it is pulled back onto it.
        let mut a = app(TUIMode::View, vec![monitor("A", 0, 0, true)]);

        let (col, row) = cell_for_monitor(&a, 0);
        down(&mut a, col, row);
        drag(&mut a, col + 1, row);

        assert_eq!(pos(&a.monitors[0]), (0, 0), "the drag snapped onto the origin");
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
    fn clicking_the_checkbox_column_toggles_enable() {
        let mut a = app(TUIMode::View, test_monitors());

        let buf = render(&a);
        let (col, row) = find_cell(&buf, ENABLED_ICON).expect("enabled icon");
        down(&mut a, col, row);
        assert!(!a.monitors[0].enabled, "clicking the icon disables monitor 0");
        assert_eq!(a.selected_monitor, 0);

        let buf = render(&a);
        let (col, row) = find_cell(&buf, DISABLED_ICON).expect("disabled icon");
        down(&mut a, col, row);
        assert!(a.monitors[0].enabled, "clicking again re-enables it");
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
        assert_eq!(a.mode, TUIMode::Resolution, "the click does not leave the mode");
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
        assert_eq!(a.selected_resolution, 0, "the resolution selection is untouched");
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

        handle_mouse(&mut a, mouse(MouseEventKind::Down(MouseButton::Right), col, row));
        handle_mouse(&mut a, mouse(MouseEventKind::Moved, col, row));
        handle_mouse(&mut a, mouse(MouseEventKind::ScrollLeft, col, row));
        handle_mouse(&mut a, mouse(MouseEventKind::ScrollRight, col, row));

        assert_eq!(a.selected_monitor, 0);
        assert!(a.drag.is_none());
    }
}
