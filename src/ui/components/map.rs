//! Monitor map canvas component.
//!
//! Displays a visual representation of monitor layout.

use crate::{
    App,
    actions::movement::{self, SnapGuides},
    monitor::{Monitor, MonitorCanvas},
    utils::TUIMode,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style, Stylize},
    symbols::{Marker, border},
    text::Line,
    widgets::{
        Block, Clear, Paragraph, Widget,
        canvas::{Canvas, Line as CanvasLine, Rectangle},
    },
};

#[derive(Debug)]
pub struct Map<'a> {
    pub selected: usize,
    pub monitors: &'a [Monitor],
    pub is_previewing: bool,
    // Alignment guides to overlay while a drag is snapped.
    pub guides: Option<SnapGuides>,
    // Frozen canvas bounds to use instead of recomputing from the (moving)
    // monitor list, so a drag stays linear in world space.
    pub canvas: Option<MonitorCanvas>,
    // Monitor with an unapplied move; shows the clickable Apply button.
    pub moved: Option<usize>,
}

impl<'a> Widget for Map<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let monitor_canvas = self
            .canvas
            .unwrap_or_else(|| Monitor::get_monitors_canvas(self.monitors, &area));

        let title_text = if self.is_previewing {
            " Map (Preview) "
        } else {
            " Map "
        };
        let title = Line::from(title_text.white().bold());

        let block = Block::bordered()
            .title(title.centered())
            .border_set(border::THICK)
            .border_style(Style::default().fg(Color::Yellow));

        Canvas::default()
            .marker(Marker::HalfBlock)
            .block(block)
            .x_bounds(monitor_canvas.x_bounds)
            .y_bounds(monitor_canvas.y_bounds)
            .paint(|ctx| {
                let mut index = 0;
                for monitor in self.monitors {
                    if self.selected != index && monitor.enabled {
                        self.render_enabled_monitor(ctx, &monitor_canvas, monitor, Color::Blue);
                    }
                    index += 1;
                }
                index = 0;
                for monitor in self.monitors {
                    if self.selected == index && monitor.enabled {
                        self.render_enabled_monitor(ctx, &monitor_canvas, monitor, Color::Yellow);
                    }
                    index += 1;
                }
                if let Some(guides) = self.guides {
                    let [left, right] = monitor_canvas.x_bounds;
                    let [bottom, top] = monitor_canvas.y_bounds;
                    if let Some(gx) = guides.x {
                        ctx.draw(&CanvasLine {
                            x1: gx,
                            y1: bottom,
                            x2: gx,
                            y2: top,
                            color: Color::Magenta,
                        });
                    }
                    if let Some(gy) = guides.y {
                        // The map draws in pre-flipped canvas coordinates
                        // (see render_enabled_monitor), so world y values must
                        // be mirrored the same way.
                        let canvas_y = bottom + top - gy;
                        ctx.draw(&CanvasLine {
                            x1: left,
                            y1: canvas_y,
                            x2: right,
                            y2: canvas_y,
                            color: Color::Magenta,
                        });
                    }
                }
            })
            .render(area, buf);

        // A monitor with an unapplied move shows a clickable Apply button in
        // its centre.
        if let Some(index) = self.moved
            && let Some(monitor) = self.monitors.get(index)
            && monitor.enabled
            && let Some(rect) = Map::apply_button_rect(area, &monitor_canvas, monitor)
        {
            Clear.render(rect, buf);
            Paragraph::new(Line::from(" Apply ").green().bold())
                .alignment(Alignment::Center)
                .render(rect, buf);
        }
    }
}
impl<'a> Map<'a> {
    pub fn handle_events(app: &mut App, key_event: KeyEvent) {
        let is_shift = key_event.modifiers.contains(KeyModifiers::SHIFT);
        let selected = app.selected_monitor;
        match key_event.code {
            KeyCode::Char('k') => movement::snap_vertical(&mut app.monitors, selected, -1),
            KeyCode::Char('K') => movement::move_vertical(&mut app.monitors[selected], -10),
            KeyCode::Up => {
                if is_shift {
                    movement::move_vertical(&mut app.monitors[selected], -10)
                } else {
                    movement::snap_vertical(&mut app.monitors, selected, -1)
                }
            }

            KeyCode::Char('j') => movement::snap_vertical(&mut app.monitors, selected, 1),
            KeyCode::Char('J') => movement::move_vertical(&mut app.monitors[selected], 10),
            KeyCode::Down => {
                if is_shift {
                    movement::move_vertical(&mut app.monitors[selected], 10)
                } else {
                    movement::snap_vertical(&mut app.monitors, selected, 1)
                }
            }

            KeyCode::Char('h') => movement::snap_horizontal(&mut app.monitors, selected, -1),
            KeyCode::Char('H') => movement::move_horizontal(&mut app.monitors[selected], -10),
            KeyCode::Left => {
                if is_shift {
                    movement::move_horizontal(&mut app.monitors[selected], -10)
                } else {
                    movement::snap_horizontal(&mut app.monitors, selected, -1)
                }
            }

            KeyCode::Char('l') => movement::snap_horizontal(&mut app.monitors, selected, 1),
            KeyCode::Char('L') => movement::move_horizontal(&mut app.monitors[selected], 10),
            KeyCode::Right => {
                if is_shift {
                    movement::move_horizontal(&mut app.monitors[selected], 10)
                } else {
                    movement::snap_horizontal(&mut app.monitors, selected, 1)
                }
            }

            // Esc/Enter/q leave the mode; q no longer quits from Move mode.
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('Q') => {
                crate::utils::change_mode(app, TUIMode::View)
            }
            _ => {}
        }
    }

    // Converts a screen cell inside the canvas into world (monitor) pixels.
    //
    // The canvas draws `x_bounds` left-to-right and `y_bounds` bottom-to-top
    // (matching the y flip used when drawing), so this is the inverse mapping.
    // Points outside the canvas return coordinates outside the bounds, which
    // keeps dragging beyond the edge working.
    pub fn world_at(area: Rect, monitors: &[Monitor], col: u16, row: u16) -> Option<(f64, f64)> {
        let canvas = Monitor::get_monitors_canvas(monitors, &area);
        Map::world_at_in(area, &canvas, col, row)
    }

    // Like [`Map::world_at`] but against explicit bounds; used during a drag
    // so the mapping does not shift as the dragged monitor moves.
    pub fn world_at_in(
        area: Rect,
        canvas: &MonitorCanvas,
        col: u16,
        row: u16,
    ) -> Option<(f64, f64)> {
        let inner = Block::bordered().inner(area);
        if inner.width == 0 || inner.height == 0 {
            return None;
        }
        let [left, right] = canvas.x_bounds;
        let [bottom, top] = canvas.y_bounds;

        // Signed so points beyond the left/top edge extrapolate as well.
        let fx = (col as i32 - inner.x as i32) as f64 / inner.width as f64;
        let fy = (row as i32 - inner.y as i32) as f64 / inner.height as f64;

        // Screen rows grow downwards and so does the world y axis, so the top
        // row maps to the smallest bound (no flip); the flip lives in drawing.
        // Points outside the canvas return coordinates outside the bounds,
        // which keeps dragging beyond every edge working.
        Some((left + fx * (right - left), bottom + fy * (top - bottom)))
    }

    // Screen rectangle of the Apply button drawn in the centre of a monitor.
    //
    // Shared by rendering and hit-testing so the clickable area always matches
    // the drawn label.
    pub fn apply_button_rect(
        area: Rect,
        canvas: &MonitorCanvas,
        monitor: &Monitor,
    ) -> Option<Rect> {
        let inner = Block::bordered().inner(area);
        if inner.width == 0 || inner.height == 0 {
            return None;
        }
        let (x, y, w, h) = monitor.get_geometry();
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let [left, right] = canvas.x_bounds;
        let [bottom, top] = canvas.y_bounds;

        let width = 9u16.min(inner.width);
        let center_x =
            inner.x as f64 + ((x + w / 2.0) - left) / (right - left) * inner.width as f64;
        let center_y =
            inner.y as f64 + ((y + h / 2.0) - bottom) / (top - bottom) * inner.height as f64;

        let max_x = (inner.right() as f64 - width as f64).max(inner.x as f64);
        let bx = (center_x - width as f64 / 2.0)
            .round()
            .clamp(inner.x as f64, max_x) as u16;
        let by = center_y
            .round()
            .clamp(inner.y as f64, inner.bottom().saturating_sub(1) as f64) as u16;
        Some(Rect::new(bx, by, width, 1))
    }

    // Returns the index of the topmost enabled monitor containing `world`.
    //
    // The selected monitor is drawn last (on top), so it wins when rectangles
    // overlap.
    pub fn monitor_at(monitors: &[Monitor], selected: usize, world: (f64, f64)) -> Option<usize> {
        let (wx, wy) = world;
        let contains = |m: &Monitor| {
            if !m.enabled {
                return false;
            }
            let (x, y, w, h) = m.get_geometry();
            w > 0.0 && h > 0.0 && wx >= x && wx < x + w && wy >= y && wy < y + h
        };

        if monitors.get(selected).is_some_and(&contains) {
            return Some(selected);
        }
        monitors.iter().position(contains)
    }

    pub fn render_enabled_monitor(
        &self,
        ctx: &mut ratatui::widgets::canvas::Context,
        monitor_canvas: &MonitorCanvas,
        monitor: &Monitor,
        color: Color,
    ) {
        // A monitor without a position cannot be drawn.
        let position = match &monitor.position {
            Some(position) => position,
            None => return,
        };
        let (width, height) = monitor.get_logical_dimensions();
        let x = position.x as f64;
        // Monitor coordinates grow downwards, the canvas y axis grows
        // upwards: mirror the position around the centre of the bounds so
        // that stacked monitors render in their real order. Deriving the
        // mirror from the bounds keeps the drawing centred with its margin
        // no matter where the monitors sit — a monitor at y = 0 gets the
        // same 50px border on both sides.
        let y =
            monitor_canvas.y_bounds[0] + monitor_canvas.y_bounds[1] - position.y as f64 - height;

        let x_margin = width * 0.07;
        let y_margin = height * 0.07;

        ctx.print(
            x + x_margin,
            y + height - y_margin,
            Line::styled(monitor.name.to_string(), color),
        );

        ctx.draw(&Rectangle {
            x,
            y,
            width,
            height,
            color,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{Position, Resolution};
    use crate::test_utils::tests::{TempDir, find_text, test_monitors, text_of};
    use ratatui::style::Style;

    fn monitor(name: &str, x: i32, y: i32) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
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

    fn app(monitors: Vec<Monitor>, selected: usize) -> App {
        let mut app = App::new(None);
        app.monitors = monitors;
        app.selected_monitor = selected;
        app
    }

    fn press(a: &mut App, code: KeyCode) {
        Map::handle_events(a, KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn press_with(a: &mut App, code: KeyCode, modifiers: KeyModifiers) {
        Map::handle_events(a, KeyEvent::new(code, modifiers));
    }

    fn position_of(m: &Monitor) -> (i32, i32) {
        let p = m.position.clone().unwrap();
        (p.x, p.y)
    }

    #[test]
    fn render_map() {
        let map = Map {
            selected: 0,
            monitors: &test_monitors(),
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));

        map.render(buf.area, &mut buf);

        let mut expected = Buffer::with_lines(vec![
            "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ Map ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
            "┃                                                                                                  ┃",
            "┃  █▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀█  ┃",
            "┃  █     Monitor 1                                                                              █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █                                                                                            █  ┃",
            "┃  █▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄█  ┃",
            "┃                                                                                                  ┃",
            "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
        ]);
        let vertical_line_style = Style::new().fg(Color::Yellow).bg(Color::Yellow);

        let horizontal_line_style = Style::new().fg(Color::Yellow);
        let border_style = Style::new().fg(Color::Yellow);
        let title_style = Style::new().white().bold();
        let empty_style = Style::new();

        expected.set_style(Rect::new(0, 0, 47, 1), border_style);
        expected.set_style(Rect::new(47, 0, 5, 1), title_style);
        expected.set_style(Rect::new(52, 0, 48, 1), border_style);

        expected.set_style(Rect::new(0, 1, 1, 28), border_style);
        expected.set_style(Rect::new(1, 1, 98, 28), empty_style);
        expected.set_style(Rect::new(99, 1, 1, 28), border_style);

        expected.set_style(Rect::new(0, 29, 100, 1), border_style);

        // Monitor styles
        // Top line y=2
        expected.set_style(Rect::new(3, 2, 1, 1), vertical_line_style);
        expected.set_style(Rect::new(4, 2, 92, 1), horizontal_line_style);
        expected.set_style(Rect::new(96, 2, 1, 1), vertical_line_style);

        // Sides y=3..26
        expected.set_style(Rect::new(3, 3, 1, 24), vertical_line_style);
        expected.set_style(Rect::new(96, 3, 1, 24), vertical_line_style);

        // Bottom line y=27
        expected.set_style(Rect::new(3, 27, 1, 1), vertical_line_style);
        expected.set_style(Rect::new(4, 27, 92, 1), horizontal_line_style);
        expected.set_style(Rect::new(96, 27, 1, 1), vertical_line_style);

        // Text y=3
        expected.set_style(Rect::new(9, 3, 9, 1), horizontal_line_style);

        assert_eq!(buf, expected);
    }

    #[test]
    fn only_enabled_monitors_are_drawn() {
        // test_monitors() has a disabled second monitor.
        let monitors = test_monitors();
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        let text = text_of(&buf);
        assert!(
            text.contains("Monitor 1"),
            "enabled monitor missing: {text}"
        );
        assert!(
            !text.contains("Monitor 2"),
            "disabled monitor drawn: {text}"
        );
    }

    #[test]
    fn preview_mode_is_marked_in_the_title() {
        let monitors = test_monitors();
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: true,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        assert!(text_of(&buf).contains("Map (Preview)"));
    }

    #[test]
    fn every_enabled_monitor_gets_a_label() {
        let monitors = vec![monitor("Alpha", 0, 0), monitor("Beta", 1920, 0)];
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        let text = text_of(&buf);
        assert!(text.contains("Alpha"), "first label missing: {text}");
        assert!(text.contains("Beta"), "second label missing: {text}");
    }

    #[test]
    fn the_selected_monitor_is_highlighted() {
        let monitors = vec![monitor("Alpha", 0, 0), monitor("Beta", 1920, 0)];
        let area = Rect::new(0, 0, 100, 30);

        let first_selected = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(area);
        first_selected.render(area, &mut buf);
        let (ax, ay) = find_text(&buf, "Alpha").expect("Alpha label");
        let (bx, by) = find_text(&buf, "Beta").expect("Beta label");
        let first_styles = (buf[(ax, ay)].style().fg, buf[(bx, by)].style().fg);

        let second_selected = Map {
            selected: 1,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(area);
        second_selected.render(area, &mut buf);
        let (ax, ay) = find_text(&buf, "Alpha").expect("Alpha label");
        let (bx, by) = find_text(&buf, "Beta").expect("Beta label");
        let second_styles = (buf[(ax, ay)].style().fg, buf[(bx, by)].style().fg);

        assert_ne!(
            first_styles.0, second_styles.0,
            "the selected label must change colour"
        );
        assert_ne!(
            first_styles.1, second_styles.1,
            "the deselected label must change colour"
        );
        assert_eq!(
            first_styles.0,
            Some(Color::Yellow),
            "selected monitor is yellow"
        );
        assert_eq!(first_styles.1, Some(Color::Blue), "other monitors are blue");
    }

    #[test]
    fn capital_keys_nudge_the_selected_monitor_by_ten() {
        let mut a = app(vec![monitor("A", 100, 100)], 0);

        press(&mut a, KeyCode::Char('H'));
        assert_eq!(position_of(&a.monitors[0]), (90, 100));

        press(&mut a, KeyCode::Char('L'));
        assert_eq!(position_of(&a.monitors[0]), (100, 100));

        press(&mut a, KeyCode::Char('K'));
        assert_eq!(position_of(&a.monitors[0]), (100, 90));

        press(&mut a, KeyCode::Char('J'));
        assert_eq!(position_of(&a.monitors[0]), (100, 100));
    }

    #[test]
    fn lowercase_keys_snap_to_the_neighbour() {
        // Second monitor sits right/below the first one, so snapping moves it
        // onto the shared edge.
        let mut a = app(vec![monitor("A", 0, 0), monitor("B", 3000, 2000)], 1);

        press(&mut a, KeyCode::Char('h'));
        assert_eq!(position_of(&a.monitors[1]), (1920, 2000));

        press(&mut a, KeyCode::Char('k'));
        assert_eq!(position_of(&a.monitors[1]), (1920, 1080));
    }

    #[test]
    fn arrow_keys_snap_unless_shift_is_held() {
        let mut a = app(vec![monitor("A", 0, 0), monitor("B", 3000, 2000)], 1);

        press_with(&mut a, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(
            position_of(&a.monitors[1]),
            (1920, 2000),
            "plain arrow snaps"
        );

        press_with(&mut a, KeyCode::Left, KeyModifiers::SHIFT);
        assert_eq!(
            position_of(&a.monitors[1]),
            (1910, 2000),
            "shifted arrow moves by ten"
        );

        press_with(&mut a, KeyCode::Up, KeyModifiers::SHIFT);
        assert_eq!(position_of(&a.monitors[1]), (1910, 1990));

        press_with(&mut a, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(
            position_of(&a.monitors[1]),
            (1910, 1990),
            "nothing to snap onto below"
        );
    }

    #[test]
    fn unrelated_keys_are_ignored() {
        let mut a = app(vec![monitor("A", 100, 100)], 0);

        for code in [
            KeyCode::Char('z'),
            KeyCode::Char('w'),
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::F(5),
            KeyCode::Delete,
        ] {
            let before = position_of(&a.monitors[0]);
            press(&mut a, code);
            assert_eq!(
                position_of(&a.monitors[0]),
                before,
                "{:?} must not move the monitor",
                code
            );
        }
    }

    #[test]
    fn escape_returns_from_sub_modes_to_view() {
        // Non-persisting modes only here; leaving Move/Scale writes the
        // monitor state and is covered by the dedicated test below.
        let mut a = app(vec![monitor("A", 0, 0)], 0);
        a.mode = TUIMode::Resolution;

        press(&mut a, KeyCode::Esc);

        assert_eq!(a.mode, TUIMode::View);

        let mut a = app(vec![monitor("A", 0, 0)], 0);
        a.mode = TUIMode::Resolution;

        press(&mut a, KeyCode::Enter);

        assert_eq!(a.mode, TUIMode::View);
    }

    #[test]
    fn escape_from_move_mode_persists_the_moved_monitor() {
        let dir = TempDir::new("map-state");
        let mut a = app(vec![monitor("A", 100, 100)], 0);
        a.mode = TUIMode::Move;
        a.state_path = Some(dir.path().join("monitor_state.json"));

        press(&mut a, KeyCode::Char('L')); // nudge +10 on x
        press(&mut a, KeyCode::Esc);

        assert_eq!(a.mode, TUIMode::View);
        let content = std::fs::read_to_string(dir.path().join("monitor_state.json"))
            .expect("leaving Move mode must have written the state file");
        let saved: Vec<crate::config::MonitorState> =
            serde_json::from_str(&content).expect("state file must hold valid JSON");
        assert_eq!(
            saved[0].position,
            Some(Position { x: 110, y: 100 }),
            "the moved position is persisted"
        );
    }

    #[test]
    fn escape_from_view_mode_stays_in_view_mode() {
        let mut a = app(vec![monitor("A", 0, 0)], 0);

        press(&mut a, KeyCode::Esc);

        assert_eq!(a.mode, TUIMode::View);
    }

    #[test]
    fn q_leaves_move_mode_and_persists() {
        let dir = TempDir::new("map-q");
        let mut a = app(vec![monitor("A", 100, 100)], 0);
        a.mode = TUIMode::Move;
        a.state_path = Some(dir.path().join("monitor_state.json"));

        press(&mut a, KeyCode::Char('q'));

        assert_eq!(a.mode, TUIMode::View, "q cancels back to View");
        assert!(
            dir.path().join("monitor_state.json").exists(),
            "leaving Move mode still persists the layout"
        );
    }

    #[test]
    fn a_monitor_without_a_position_is_skipped_rather_than_panicking() {
        let mut unplaced = monitor("Ghost", 0, 0);
        unplaced.position = None;
        let monitors = vec![unplaced, monitor("A", 0, 0)];

        let map = Map {
            selected: 1,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        let text = text_of(&buf);
        assert!(text.contains("A"), "placed monitor missing: {text}");
        assert!(!text.contains("Ghost"), "unplaced monitor drawn: {text}");
    }

    #[test]
    fn stacked_monitors_render_in_their_real_vertical_order() {
        // In Hyprland coordinates y grows downwards, so the monitor at
        // y = -1080 sits above the one at y = 0 and must render above it.
        let monitors = vec![monitor("Lower", 0, 0), monitor("Upper", 0, -1080)];
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        let (_, upper_y) = find_text(&buf, "Upper").expect("Upper label");
        let (_, lower_y) = find_text(&buf, "Lower").expect("Lower label");
        assert!(
            upper_y < lower_y,
            "Upper (y=-1080) must render above Lower (y=0): rows {upper_y} vs {lower_y}"
        );
    }

    #[test]
    fn a_monitor_away_from_the_origin_is_still_drawn_centred() {
        // y = 100 keeps the whole layout on positive coordinates; the box
        // must still get the same margin above and below instead of sitting
        // flush against the bottom edge of the canvas.
        let monitors = vec![monitor("A", 0, 100)];
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        let box_rows: Vec<u16> = (buf.area.top()..buf.area.bottom())
            .filter(|&y| (buf.area.left()..buf.area.right()).any(|x| buf[(x, y)].symbol() == "█"))
            .collect();
        let first = *box_rows.first().expect("top edge of the box");
        let last = *box_rows.last().expect("bottom edge of the box");

        // Interior rows run from 1 to bottom - 2; equal distances to both
        // edges prove the centring.
        assert_eq!(
            first - buf.area.top() - 1,
            buf.area.bottom() - 2 - last,
            "box spans rows {first}..{last}, not centred"
        );
    }

    #[test]
    fn map_survives_tiny_areas() {
        for (width, height) in [(1u16, 1u16), (3, 2), (10, 5), (40, 12)] {
            let monitors = vec![monitor("A", 0, 0), monitor("B", 1920, 0)];
            let map = Map {
                selected: 0,
                monitors: &monitors,
                is_previewing: false,
                guides: None,
                canvas: None,
                moved: None,
            };
            let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
            map.render(buf.area, &mut buf);
        }
    }

    // --- world-space hit testing ----------------------------------------

    #[test]
    fn world_at_maps_the_canvas_corner_to_the_bounds() {
        let monitors = vec![monitor("A", 0, 0)];
        let area = Rect::new(0, 0, 100, 30);
        let inner = Block::bordered().inner(area);
        let canvas = Monitor::get_monitors_canvas(&monitors, &area);

        let (x, y) = Map::world_at(area, &monitors, inner.x, inner.y).unwrap();

        // Top-left inner cell is the (left, smallest-y) corner of the world:
        // the small world y is the top of the screen.
        assert_eq!(x, canvas.x_bounds[0]);
        assert_eq!(y, canvas.y_bounds[0]);
    }

    #[test]
    fn world_coordinates_grow_right_and_downwards() {
        let monitors = vec![monitor("A", 0, 0)];
        let area = Rect::new(0, 0, 100, 30);
        let inner = Block::bordered().inner(area);

        let (x0, y0) = Map::world_at(area, &monitors, inner.x, inner.y).unwrap();
        let (x1, _) = Map::world_at(area, &monitors, inner.right() - 1, inner.y).unwrap();
        let (_, y1) = Map::world_at(area, &monitors, inner.x, inner.bottom() - 1).unwrap();

        assert!(x1 > x0, "moving right increases world x");
        assert!(y1 > y0, "moving down increases world y (as Hyprland does)");
    }

    #[test]
    fn world_at_rejects_a_degenerate_canvas() {
        let monitors = vec![monitor("A", 0, 0)];
        // A 1x1 area leaves no inner cells for the canvas.
        assert_eq!(Map::world_at(Rect::new(0, 0, 1, 1), &monitors, 0, 0), None);
    }

    #[test]
    fn world_at_extrapolates_beyond_every_edge() {
        let monitors = vec![monitor("A", 0, 0)];
        let area = Rect::new(0, 0, 100, 30);
        let inner = Block::bordered().inner(area);

        let (cx, cy) = Map::world_at(area, &monitors, inner.x, inner.y).unwrap();
        // Left of / above the inner area: without signed math these would be
        // clamped to the bound instead of extrapolating.
        let (lx, ly) = Map::world_at(area, &monitors, 0, 0).unwrap();

        assert!(lx < cx, "left of the canvas must extrapolate: {lx} < {cx}");
        assert!(ly < cy, "above the canvas must extrapolate: {ly} < {cy}");
    }

    #[test]
    fn monitor_at_finds_the_monitor_under_a_world_point() {
        let monitors = vec![monitor("A", 0, 0), monitor("B", 1920, 0)];

        assert_eq!(Map::monitor_at(&monitors, 0, (10.0, 10.0)), Some(0));
        assert_eq!(Map::monitor_at(&monitors, 0, (1930.0, 10.0)), Some(1));
    }

    #[test]
    fn monitor_at_returns_none_outside_every_monitor() {
        let monitors = vec![monitor("A", 0, 0), monitor("B", 1920, 0)];

        assert_eq!(Map::monitor_at(&monitors, 0, (5000.0, 10.0)), None);
        assert_eq!(Map::monitor_at(&monitors, 0, (10.0, -5.0)), None);
    }

    #[test]
    fn monitor_edges_are_half_open() {
        // 1920x1080 at the origin: the right/bottom edge belongs to the next
        // cell, not to the monitor.
        let monitors = vec![monitor("A", 0, 0)];

        assert_eq!(Map::monitor_at(&monitors, 0, (1919.9, 1079.9)), Some(0));
        assert_eq!(Map::monitor_at(&monitors, 0, (1920.0, 500.0)), None);
        assert_eq!(Map::monitor_at(&monitors, 0, (500.0, 1080.0)), None);
    }

    #[test]
    fn monitor_at_prefers_the_selected_monitor_on_overlap() {
        let monitors = vec![monitor("A", 0, 0), monitor("B", 0, 0)];

        assert_eq!(Map::monitor_at(&monitors, 1, (10.0, 10.0)), Some(1));
        assert_eq!(Map::monitor_at(&monitors, 0, (10.0, 10.0)), Some(0));
    }

    #[test]
    fn monitor_at_ignores_disabled_monitors() {
        let mut disabled = monitor("B", 1920, 0);
        disabled.enabled = false;
        let monitors = vec![monitor("A", 0, 0), disabled];

        assert_eq!(Map::monitor_at(&monitors, 1, (1930.0, 10.0)), None);
    }

    #[test]
    fn monitor_at_ignores_monitors_without_a_position() {
        let mut unplaced = monitor("B", 0, 0);
        unplaced.position = None;
        let monitors = vec![unplaced];

        assert_eq!(Map::monitor_at(&monitors, 0, (10.0, 10.0)), None);
    }

    // --- apply button ---------------------------------------------------

    #[test]
    fn the_apply_button_rect_stays_inside_the_monitor_and_area() {
        let monitors = vec![monitor("A", 0, 0)];
        let area = Rect::new(0, 0, 100, 30);
        let canvas = Monitor::get_monitors_canvas(&monitors, &area);

        let rect = Map::apply_button_rect(area, &canvas, &monitors[0]).expect("button rect");
        let inner = Block::bordered().inner(area);

        assert!(rect.width >= 1 && rect.height == 1);
        assert!(rect.x >= inner.x && rect.right() <= inner.right());
        assert!(rect.y >= inner.y && rect.bottom() <= inner.bottom());
    }

    #[test]
    fn the_apply_button_rect_is_none_without_geometry() {
        let mut unplaced = monitor("A", 0, 0);
        unplaced.position = None;
        let monitors = vec![unplaced];
        let area = Rect::new(0, 0, 100, 30);
        let canvas = Monitor::get_monitors_canvas(&monitors, &area);

        assert!(Map::apply_button_rect(area, &canvas, &monitors[0]).is_none());
    }

    #[test]
    fn the_apply_button_label_is_drawn_only_for_the_moved_monitor() {
        let monitors = vec![monitor("A", 0, 0)];
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));

        let plain = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: None,
        };
        plain.render(buf.area, &mut buf);
        assert!(!text_of(&buf).contains("Apply"), "no button without a move");

        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        let moved = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: None,
            canvas: None,
            moved: Some(0),
        };
        moved.render(buf.area, &mut buf);
        assert!(text_of(&buf).contains("Apply"), "the button is drawn");
    }

    #[test]
    fn rendering_the_apply_button_survives_tiny_areas() {
        for (width, height) in [(1u16, 1u16), (3, 2), (10, 5), (40, 12)] {
            let monitors = vec![monitor("A", 0, 0)];
            let map = Map {
                selected: 0,
                monitors: &monitors,
                is_previewing: false,
                guides: None,
                canvas: None,
                moved: Some(0),
            };
            let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
            map.render(buf.area, &mut buf);
        }
    }

    // --- alignment guides -----------------------------------------------

    #[test]
    fn a_horizontal_guide_renders_on_the_same_side_as_the_world_coordinate() {
        // A guide at world y = 0 marks the top edge of a monitor at the origin,
        // so it must render near the top of the canvas, not the bottom.
        let monitors = vec![monitor("A", 0, 0)];
        let area = Rect::new(0, 0, 100, 30);
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: Some(SnapGuides {
                x: None,
                y: Some(0.0),
            }),
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(area);
        map.render(area, &mut buf);

        let row = (area.top()..area.bottom())
            .find(|&y| {
                (area.left()..area.right()).any(|x| buf[(x, y)].style().fg == Some(Color::Magenta))
            })
            .expect("the guide is drawn");
        assert!(
            row < area.height / 2,
            "guide drawn on the wrong side: row {row}"
        );
    }

    #[test]
    fn a_vertical_guide_renders_near_the_world_x() {
        let monitors = vec![monitor("A", 0, 0)];
        let area = Rect::new(0, 0, 100, 30);
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: false,
            guides: Some(SnapGuides {
                x: Some(0.0),
                y: None,
            }),
            canvas: None,
            moved: None,
        };
        let mut buf = Buffer::empty(area);
        map.render(area, &mut buf);

        let col = (area.left()..area.right())
            .find(|&x| {
                (area.top()..area.bottom()).any(|y| buf[(x, y)].style().fg == Some(Color::Magenta))
            })
            .expect("the guide is drawn");
        assert!(
            col < area.width / 2,
            "vertical guide on the wrong side: col {col}"
        );
    }
}
