//! Resolution selection table component.

use crossterm::event::{KeyCode,KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState},
};

use ratatui::layout::Constraint;
use crate::monitor::Monitor;
use crate::ui::layouts::{row_in_window, scrolled_offset};
use crate::utils::TUIMode;
use crate::App;

#[derive(Debug)]
pub struct Resolutions<'a> {
    pub state: TableState,
    pub monitor:&'a Monitor,
}

impl<'a> Resolutions<'a> {

    pub fn new(monitor: &'a Monitor,selected:Option<usize>) -> Self {
        Resolutions {
            state: TableState::default()
                .with_selected(selected),
            monitor,
        }
    }
 
    pub fn handle_events(app:&mut App, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('k') | KeyCode::Up => Resolutions::previous(app),
            KeyCode::Char('j') | KeyCode::Down => Resolutions::next(app),
            KeyCode::Char(' ') => Resolutions::select(app),
            KeyCode::Enter => {
                Resolutions::select(app);
                crate::utils::change_mode(app, TUIMode::View);
            }
            // Esc cancels; q/Q cancel too instead of quitting the app.
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                crate::utils::change_mode(app, TUIMode::View)
            }
            _ => {}
        }
    }

    // Maps a click's row to a resolution index, honouring the scroll window.
    pub fn row_at(area: Rect, selected: usize, total: usize, y: u16) -> Option<usize> {
        let inner = Block::bordered().inner(area);
        // Header row plus its bottom margin.
        let viewport = inner.height.saturating_sub(2);
        let offset = scrolled_offset(selected, viewport as usize);
        row_in_window(inner.y.saturating_add(2), viewport, offset, total, y)
    }

    fn next(app:&mut App) {
        app.selected_resolution = if app.selected_resolution >= app.monitors[app.selected_monitor].modes.len() - 1 {
            0
        } else {
            app.selected_resolution + 1
        }
    }

    fn previous(app:&mut App) {
        app.selected_resolution = if app.selected_resolution == 0 {
            app.monitors[app.selected_monitor].modes.len() - 1
        } else {
            app.selected_resolution - 1
        }
    }

    fn select(app:&mut App) {
        app.monitors[app.selected_monitor].set_current_resolution(app.selected_resolution);
    }

    fn resolutions_to_rows(&self) -> Vec<Row<'static>> {
        self.monitor.modes
            .iter()
            .map(|mode| {
                Row::new(vec![
                     Cell::from(if mode.current { "".to_string() } else { "".to_string() })
                     .style(
                         Style::default().fg(
                             if mode.current {Color::Green} else {Color::Red}
                         )
                     ),
                    Cell::from(format!("{}x{}", mode.width, mode.height)),
                    Cell::from(mode.refresh.to_string()),
                     Cell::from(if mode.preferred { "".to_string() } else { "".to_string() })
                     .style(
                         Style::default().fg(
                             if mode.preferred {Color::Green} else {Color::Red}
                         )
                     ),
                ])
            })
            .collect()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(" Resolutions ".bold());
        let block = Block::bordered()
            .title(title.white().centered())
            .border_set(border::THICK)
            .border_style(Style::default().fg(Color::Yellow));


        let widths = [
            
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ];   

        let table = Table::new(self.resolutions_to_rows(),widths) 
            .column_spacing(1)
            //.style(Style::new().blue())
            .header(
                Row::new(vec![
                    Cell::from("current"),
                    Cell::from("resolution"),
                    Cell::from("refresh"),
                    Cell::from("preferred"),
                ])
                    .style(Style::new().bold())
                    .bottom_margin(1)
                    .bold()
                    .green()
                    .reversed()
            )
 
            .row_highlight_style(Style::new().yellow())
            .cell_highlight_style(Style::new().blue())
            .highlight_symbol(" ")           
            .block(block);

        StatefulWidget::render(
            table,
            area,
            buf,
            &mut self.state,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;
    use crate::test_utils::tests::test_monitors;

    #[test]
    fn render_resolutions() {

        let mut resolutions = Resolutions {
            state: TableState::default(),
            monitor:&test_monitors()[0],
            
        }; 
        let mut buf = Buffer::empty(Rect::new(0, 0, 65, 7));
        
        resolutions.render(buf.area, &mut buf);

        let mut expected = Buffer::with_lines(vec![
            "┏━━━━━━━━━━━━━━━━━━━━━━━━━ Resolutions ━━━━━━━━━━━━━━━━━━━━━━━━━┓",
            "┃current         resolution      refresh         preferred      ┃",
            "┃                                                               ┃",
            "┃               1920x1080       60                            ┃",
            "┃               1280x720        60                            ┃",
            "┃                                                               ┃",
            "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
        ]);
        let border_style = Style::new().fg(Color::Yellow);
        let title_style = Style::new().bold().fg(Color::White);
        let header_style = Style::new().green().bold().reversed();
        let empty_style = Style::new();
        let ok_style = Style::new().fg(Color::Green);
        let nok_style = Style::new().fg(Color::Red);
        let row_style = Style::new();

        // first line : title
        expected.set_style(Rect::new(0, 0, 26, 1), border_style);
        expected.set_style(Rect::new(26, 0, 13, 1), title_style);
        expected.set_style(Rect::new(39, 0, 26, 1), border_style);       

        // second line : header
        expected.set_style(Rect::new(0, 1, 1, 1), border_style);
        expected.set_style(Rect::new(1, 1, 63, 1), header_style);
        expected.set_style(Rect::new(64, 1, 1, 1), border_style);
        
        // third line : empty
        expected.set_style(Rect::new(0, 2, 1, 1), border_style);
        expected.set_style(Rect::new(1, 2, 63, 1), empty_style);
        expected.set_style(Rect::new(64, 2, 1, 1), border_style);
         
        // fourth line : first row 
        expected.set_style(Rect::new(0, 3, 1, 1), border_style);
        expected.set_style(Rect::new(1, 3, 15, 1), ok_style);
        expected.set_style(Rect::new(16, 3, 33, 1), row_style);
        expected.set_style(Rect::new(49, 3, 15, 1), ok_style);
        expected.set_style(Rect::new(64, 3, 1, 1), border_style);      

        // fifth line : second row 
        expected.set_style(Rect::new(0, 4, 1, 1), border_style);
        expected.set_style(Rect::new(1, 4, 15, 1), nok_style);
        expected.set_style(Rect::new(16, 4, 33, 1), row_style);
        expected.set_style(Rect::new(49, 4, 15, 1), nok_style);
        expected.set_style(Rect::new(64, 4, 1, 1), border_style);  
        
        // fifth line : empty
        expected.set_style(Rect::new(0, 5, 1, 1), border_style);
        expected.set_style(Rect::new(1, 5, 63, 1), empty_style);
        expected.set_style(Rect::new(64, 5, 1, 1), border_style);

        // last line : instructions 
        expected.set_style(Rect::new(0,6, 65, 1), border_style);

        assert_eq!(buf, expected);
    }

    // --- key handling ---------------------------------------------------

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use crate::monitor::{Position, Resolution};

    fn monitor(modes: usize) -> Monitor {
        let modes = (0..modes)
            .map(|i| Resolution {
                width: 1920 - i as i32 * 320,
                height: 1080 - i as i32 * 180,
                refresh: 60.0 + i as f32 * 15.0,
                preferred: i == 0,
                current: i == 0,
            })
            .collect();
        Monitor {
            name: "DP-1".to_string(),
            enabled: true,
            position: Some(Position { x: 0, y: 0 }),
            scale: Some(1.0),
            modes,
            ..Default::default()
        }
    }

    fn app(mode_count: usize) -> App {
        let mut app = App::new(None);
        app.monitors = vec![monitor(mode_count), monitor(mode_count)];
        app.selected_monitor = 0;
        app.mode = TUIMode::Resolution;
        app
    }

    fn press(a: &mut App, code: KeyCode) {
        Resolutions::handle_events(a, KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn current_index(m: &Monitor) -> usize {
        m.modes
            .iter()
            .position(|mode| mode.current)
            .expect("a mode must be selected")
    }

    #[test]
    fn j_and_k_walk_through_the_mode_list_and_wrap() {
        let mut a = app(3);

        press(&mut a, KeyCode::Char('j'));
        assert_eq!(a.selected_resolution, 1);

        press(&mut a, KeyCode::Down);
        assert_eq!(a.selected_resolution, 2);

        // At the last entry the next step wraps around to the first.
        press(&mut a, KeyCode::Char('j'));
        assert_eq!(a.selected_resolution, 0);

        // And backwards from the first entry goes to the last.
        press(&mut a, KeyCode::Char('k'));
        assert_eq!(a.selected_resolution, 2);

        press(&mut a, KeyCode::Up);
        assert_eq!(a.selected_resolution, 1);
    }

    #[test]
    fn navigation_is_confined_to_the_selected_monitor() {
        let mut a = app(2);
        a.selected_monitor = 1;
        a.selected_resolution = 1;

        press(&mut a, KeyCode::Char(' '));

        assert_eq!(current_index(&a.monitors[1]), 1, "selected monitor changed");
        assert_eq!(current_index(&a.monitors[0]), 0, "other monitor untouched");
    }

    #[test]
    fn space_marks_the_highlighted_mode_as_current() {
        let mut a = app(3);
        a.selected_resolution = 2;

        press(&mut a, KeyCode::Char(' '));

        let modes = &a.monitors[0].modes;
        assert!(!modes[0].current, "previous mode deselected");
        assert!(!modes[1].current);
        assert!(modes[2].current, "highlighted mode selected");
        // Mode data itself must not be altered.
        assert_eq!((modes[2].width, modes[2].height, modes[2].refresh), (1280, 720, 90.0));
    }

    #[test]
    fn enter_selects_and_returns_to_view_mode() {
        let mut a = app(3);
        a.selected_resolution = 1;

        press(&mut a, KeyCode::Enter);

        assert_eq!(a.mode, TUIMode::View);
        assert_eq!(current_index(&a.monitors[0]), 1);
    }

    #[test]
    fn escape_leaves_without_selecting_anything() {
        let mut a = app(3);
        a.selected_resolution = 2;

        press(&mut a, KeyCode::Esc);

        assert_eq!(a.mode, TUIMode::View);
        assert_eq!(current_index(&a.monitors[0]), 0, "nothing was applied");
    }

    #[test]
    fn unrelated_keys_are_ignored() {
        let mut a = app(3);
        a.selected_resolution = 1;

        for code in [KeyCode::Char('x'), KeyCode::Char('w'), KeyCode::Tab, KeyCode::F(1)] {
            press(&mut a, code);
            assert_eq!(a.selected_resolution, 1, "{:?} must not navigate", code);
            assert_eq!(a.mode, TUIMode::Resolution, "{:?} must not leave", code);
        }
    }

    #[test]
    fn a_single_mode_list_stays_on_that_mode() {
        let mut a = app(1);

        press(&mut a, KeyCode::Char('j'));
        assert_eq!(a.selected_resolution, 0, "wraps onto itself");

        press(&mut a, KeyCode::Char('k'));
        assert_eq!(a.selected_resolution, 0);
    }
}
