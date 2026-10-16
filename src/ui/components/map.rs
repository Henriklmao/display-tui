//! Monitor map canvas component.
//!
//! Displays a visual representation of monitor layout.

use crossterm::event::{KeyCode,KeyEvent,KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Stylize,Color,Style},
    symbols::{
        Marker,
        border,
    },
    text::Line,
    widgets::{
        Block,
        Widget,
        canvas::{
            Canvas,
            Rectangle,
        }
    },
};
use crate::{
    App,
    monitor::{Monitor, MonitorCanvas},
    utils::TUIMode,
};

#[derive(Debug)]
pub struct Map<'a>{
    pub selected: usize,
    pub monitors:&'a Vec<Monitor>,
    pub is_previewing: bool,
}

impl<'a> Widget for Map<'a>{

    fn render(self, area: Rect, buf: &mut Buffer) {

        let monitor_canvas = Monitor::get_monitors_canvas(self.monitors,&area);

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
                        self.render_enabled_monitor(ctx,&monitor_canvas, monitor, Color::Blue);
                    }
                    index += 1;
                }
                index = 0;
                for monitor in self.monitors {
                    if self.selected == index && monitor.enabled {
                            self.render_enabled_monitor(ctx,&monitor_canvas,monitor, Color::Yellow);
                    }
                    index += 1;
                }
            })
            .render(area, buf);
    } 

    
}
impl<'a> Map<'a> {
   
    pub fn handle_events(app:&mut App, key_event: KeyEvent) {
        let is_shift = key_event.modifiers.contains(KeyModifiers::SHIFT);
        match key_event.code {
            KeyCode::Char('k') => Map::snap_vertical(app, -1),
            KeyCode::Char('K') => Map::move_vertical(app, -10),
            KeyCode::Up => if is_shift { Map::move_vertical(app, -10) } else { Map::snap_vertical(app, -1) },

            KeyCode::Char('j') => Map::snap_vertical(app, 1),
            KeyCode::Char('J') => Map::move_vertical(app, 10),
            KeyCode::Down => if is_shift { Map::move_vertical(app, 10) } else { Map::snap_vertical(app, 1) },

            KeyCode::Char('h') => Map::snap_horizontal(app, -1),
            KeyCode::Char('H') => Map::move_horizontal(app, -10),
            KeyCode::Left => if is_shift { Map::move_horizontal(app, -10) } else { Map::snap_horizontal(app, -1) },

            KeyCode::Char('l') => Map::snap_horizontal(app, 1),
            KeyCode::Char('L') => Map::move_horizontal(app, 10),
            KeyCode::Right => if is_shift { Map::move_horizontal(app, 10) } else { Map::snap_horizontal(app, 1) },

            KeyCode::Esc | KeyCode::Enter => crate::utils::change_mode(app, TUIMode::View),
            _ => {}
        }
    }

    fn move_vertical(app:&mut App, direction: i32) {
        app.monitors[app.selected_monitor].move_vertical(direction);
    }
    fn snap_vertical(app:&mut App, direction: i32) {
        let selected = app.selected_monitor;
        let mut targets = vec![0.0];

        for (i, monitor) in app.monitors.iter().enumerate() {
            if i == selected || !monitor.enabled { continue; }
            let (_, y, _, h) = monitor.get_geometry();
            targets.push(y);
            targets.push(y + h);
            targets.push(y + h / 2.0);
        }

        let (_, sy, _, sh) = app.monitors[selected].get_geometry();
        let sources = vec![sy, sy + sh, sy + sh / 2.0];

        if let Some(delta) = crate::utils::find_best_delta(&sources, &targets, direction) {
            app.monitors[selected].move_vertical(delta.round() as i32);
        }
    }
    fn move_horizontal(app:&mut App, direction: i32) {
        app.monitors[app.selected_monitor].move_horizontal(direction);
    }
    fn snap_horizontal(app:&mut App, direction: i32) {
        let selected = app.selected_monitor;
        let mut targets = vec![0.0];

        for (i, monitor) in app.monitors.iter().enumerate() {
            if i == selected || !monitor.enabled { continue; }
            let (x, _, w, _) = monitor.get_geometry();
            targets.push(x);
            targets.push(x + w);
            targets.push(x + w / 2.0);
        }

        let (sx, _, sw, _) = app.monitors[selected].get_geometry();
        let sources = vec![sx, sx + sw, sx + sw / 2.0];

        if let Some(delta) = crate::utils::find_best_delta(&sources, &targets, direction) {
            app.monitors[selected].move_horizontal(delta.round() as i32);
        }
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
        let y = monitor_canvas.y_bounds[0] + monitor_canvas.y_bounds[1] - position.y as f64
            - height;

        let x_margin = width * 0.07; 
        let y_margin = height * 0.07;

        ctx.print(
            x + x_margin, 
            y + height - y_margin, 
            Line::styled(
                monitor.name.to_string(),
                color
            )
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
    use ratatui::style::Style;
    use crate::test_utils::tests::test_monitors;
    use crate::monitor::{Position, Resolution};

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

    // Returns the top-left cell of the first row containing `needle`.
    //
    // Symbols are multi-byte UTF-8, so a plain `String::find` would yield a
    // byte offset instead of a column; the columns are tracked alongside.
    fn find_text(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
        for y in buf.area.top()..buf.area.bottom() {
            let mut row = String::new();
            let mut columns = Vec::new();
            for x in buf.area.left()..buf.area.right() {
                let symbol = buf[(x, y)].symbol();
                if symbol.is_empty() {
                    continue;
                }
                row.push_str(symbol);
                columns.push(x);
            }
            if let Some(byte_offset) = row.find(needle) {
                let char_offset = row[..byte_offset].chars().count();
                return Some((*columns.get(char_offset)?, y));
            }
        }
        None
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

    #[test]
    fn render_map() {
        let map = Map {
            selected: 0,
            monitors: &test_monitors(),
            is_previewing: false,
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
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        let text = text_of(&buf);
        assert!(text.contains("Monitor 1"), "enabled monitor missing: {text}");
        assert!(!text.contains("Monitor 2"), "disabled monitor drawn: {text}");
    }

    #[test]
    fn preview_mode_is_marked_in_the_title() {
        let monitors = test_monitors();
        let map = Map {
            selected: 0,
            monitors: &monitors,
            is_previewing: true,
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
        };
        let mut buf = Buffer::empty(area);
        first_selected.render(area, &mut buf);
        let (ax, ay) = find_text(&buf, "Alpha").expect("Alpha label");
        let (bx, by) = find_text(&buf, "Beta").expect("Beta label");
        let first_styles = (
            buf[(ax, ay)].style().fg,
            buf[(bx, by)].style().fg,
        );

        let second_selected = Map {
            selected: 1,
            monitors: &monitors,
            is_previewing: false,
        };
        let mut buf = Buffer::empty(area);
        second_selected.render(area, &mut buf);
        let (ax, ay) = find_text(&buf, "Alpha").expect("Alpha label");
        let (bx, by) = find_text(&buf, "Beta").expect("Beta label");
        let second_styles = (
            buf[(ax, ay)].style().fg,
            buf[(bx, by)].style().fg,
        );

        assert_ne!(
            first_styles.0, second_styles.0,
            "the selected label must change colour"
        );
        assert_ne!(
            first_styles.1, second_styles.1,
            "the deselected label must change colour"
        );
        assert_eq!(
            first_styles.0, Some(Color::Yellow),
            "selected monitor is yellow"
        );
        assert_eq!(
            first_styles.1, Some(Color::Blue),
            "other monitors are blue"
        );
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
        let mut a = app(
            vec![monitor("A", 0, 0), monitor("B", 3000, 2000)],
            1,
        );

        press(&mut a, KeyCode::Char('h'));
        assert_eq!(position_of(&a.monitors[1]), (1920, 2000));

        press(&mut a, KeyCode::Char('k'));
        assert_eq!(position_of(&a.monitors[1]), (1920, 1080));
    }

    #[test]
    fn arrow_keys_snap_unless_shift_is_held() {
        let mut a = app(
            vec![monitor("A", 0, 0), monitor("B", 3000, 2000)],
            1,
        );

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
        // Only modes other than Move/Scale are exercised here: leaving those
        // persists the monitor state into the user's real config directory.
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
    fn escape_from_view_mode_stays_in_view_mode() {
        let mut a = app(vec![monitor("A", 0, 0)], 0);

        press(&mut a, KeyCode::Esc);

        assert_eq!(a.mode, TUIMode::View);
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
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        map.render(buf.area, &mut buf);

        let box_rows: Vec<u16> = (buf.area.top()..buf.area.bottom())
            .filter(|&y| {
                (buf.area.left()..buf.area.right()).any(|x| buf[(x, y)].symbol() == "█")
            })
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
            };
            let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
            map.render(buf.area, &mut buf);
        }
    }
}
