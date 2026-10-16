//! Scale selection table component.

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
use crate::utils::ScaleValue;
use crate::utils::TUIMode;
use crate::App;
use crate::config::save_monitor_state;

#[derive(Debug)]
pub struct Scale{
    pub state: TableState,
}

impl Scale{
    pub fn new(selected:usize) -> Self {
        Scale {
            state: TableState::default()
                .with_selected(selected),
        }
    }


    pub fn handle_events(app:&mut App, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('k') | KeyCode::Up => Scale::previous(app),
            KeyCode::Char('j') | KeyCode::Down => Scale::next(app),
            KeyCode::Char(' ') => Scale::select(app),
            KeyCode::Enter => {
                Scale::select(app);
                Scale::change_mode(app, TUIMode::View);
            }
            KeyCode::Esc => Scale::change_mode(app, TUIMode::View),
            _ => {}
        }
    }
    fn change_mode(app:&mut App,mode: TUIMode) {
        // Save monitor state when exiting Scale mode
        if app.mode == TUIMode::Scale {
            let _ = save_monitor_state(&app.monitors);
        }
        app.mode = mode;
    }

    fn next(app:&mut App) {
        app.selected_scale = if app.selected_scale >= ScaleValue::table().len() - 1 {
            0
        } else {
            app.selected_scale + 1
        }
    }

    fn previous(app:&mut App) {
        app.selected_scale = if app.selected_scale == 0 {
            ScaleValue::table().len() - 1
        } else {
            app.selected_scale - 1
        }
    }

    fn select(app:&mut App) {
        let scale_value = Some(ScaleValue::table()[app.selected_scale].value);
        app.monitors[app.selected_monitor].scale = scale_value;
    }



    fn scale_to_rows(&self) -> Vec<Row<'static>> {
        
        ScaleValue::table()
            .iter()
            .map(|scale| {
                Row::new(vec![
                    Cell::from(scale.name),
                ])
            })
            .collect()
    }
}

impl Scale{
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(" Scale ".bold());
        let block = Block::bordered()
            .title(title.white().centered())
            .border_set(border::THICK)
            .border_style(Style::default().fg(Color::Yellow));


        let widths = [
            Constraint::Percentage(100),
        ];   

        let table = Table::new(self.scale_to_rows(),widths) 
            .column_spacing(1)
            .row_highlight_style(Style::new().yellow())
            .cell_highlight_style(Style::new().blue())
            .highlight_symbol("  ")            
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

    #[test]
    fn render_scale() {

        let mut scales = Scale{
            state: TableState::default(),
        }; 
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 11));
        
        scales.render(buf.area, &mut buf);

        let mut expected = Buffer::with_lines(vec![
            "┏━━━━━ Scale ━━━━━━┓",
            "┃50%               ┃",
            "┃66%               ┃",
            "┃75%               ┃",
            "┃80%               ┃",
            "┃100%              ┃",
            "┃125%              ┃",
            "┃160%              ┃",
            "┃175%              ┃",
            "┃200%              ┃",
            "┗━━━━━━━━━━━━━━━━━━┛",
        ]);

        let border_style = Style::new().fg(Color::Yellow);
        let title_style = Style::new().bold().fg(Color::White);
        let row_style = Style::new();

        // first line : title
        expected.set_style(Rect::new(0, 0, 6, 1), border_style);
        expected.set_style(Rect::new(6, 0, 7, 1), title_style);
        expected.set_style(Rect::new(13, 0, 7, 1), border_style);       

        // second line : row 
        for i in 0..ScaleValue::table().len() {
            expected.set_style(Rect::new(0, (i + 1) as u16, 1, 1), border_style);
            expected.set_style(Rect::new(1, (i + 1) as u16, 18, 1), row_style);
            expected.set_style(Rect::new(19, (i + 1) as u16, 1, 1), border_style);
        }

        // last line : instructions 
        expected.set_style(Rect::new(0,10, 20, 1), border_style);

        assert_eq!(buf, expected);
    }

    // --- key handling ---------------------------------------------------

    use crossterm::event::KeyModifiers;
    use crate::monitor::{Monitor, Position};

    fn monitor() -> Monitor {
        Monitor {
            name: "DP-1".to_string(),
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
        }
    }

    // The mode is deliberately not TUIMode::Scale: leaving Scale mode
    // persists the monitor state into the user's real config directory,
    // which a unit test must not do.
    fn app() -> App {
        let mut app = App::new(None);
        app.monitors = vec![monitor(), monitor()];
        app.selected_monitor = 0;
        app.mode = TUIMode::Resolution;
        app
    }

    fn press(a: &mut App, code: KeyCode) {
        Scale::handle_events(a, KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn last_scale_index() -> usize {
        ScaleValue::table().len() - 1
    }

    #[test]
    fn j_and_k_walk_through_the_scale_table_and_wrap() {
        let mut a = app();

        press(&mut a, KeyCode::Char('j'));
        assert_eq!(a.selected_scale, 1);

        press(&mut a, KeyCode::Down);
        assert_eq!(a.selected_scale, 2);

        while a.selected_scale != last_scale_index() {
            press(&mut a, KeyCode::Char('j'));
        }
        assert_eq!(a.selected_scale, last_scale_index());

        // Past the end it wraps back to the first entry.
        press(&mut a, KeyCode::Char('j'));
        assert_eq!(a.selected_scale, 0);

        // And backwards from the start goes to the last entry.
        press(&mut a, KeyCode::Char('k'));
        assert_eq!(a.selected_scale, last_scale_index());

        press(&mut a, KeyCode::Up);
        assert_eq!(a.selected_scale, last_scale_index() - 1);
    }

    #[test]
    fn space_applies_the_highlighted_scale() {
        let mut a = app();
        a.selected_scale = last_scale_index();

        press(&mut a, KeyCode::Char(' '));

        assert_eq!(a.monitors[0].scale, Some(2.0));
        assert_eq!(
            a.monitors[0].scale,
            Some(ScaleValue::table()[last_scale_index()].value)
        );
    }

    #[test]
    fn every_table_entry_maps_to_its_value() {
        for (index, scale) in ScaleValue::table().iter().enumerate() {
            let mut a = app();
            a.selected_scale = index;

            press(&mut a, KeyCode::Char(' '));

            assert_eq!(
                a.monitors[0].scale,
                Some(scale.value),
                "entry {} ({}) did not apply",
                index,
                scale.name
            );
        }
    }

    #[test]
    fn only_the_selected_monitor_is_re_scaled() {
        let mut a = app();
        a.selected_monitor = 1;
        a.selected_scale = 4; // 100%

        press(&mut a, KeyCode::Char(' '));

        assert_eq!(a.monitors[1].scale, Some(1.0));
        assert_eq!(a.monitors[0].scale, None, "untouched monitor keeps its scale");
    }

    #[test]
    fn enter_applies_and_returns_to_view_mode() {
        let mut a = app();
        a.selected_scale = 8;

        press(&mut a, KeyCode::Enter);

        assert_eq!(a.mode, TUIMode::View);
        assert_eq!(a.monitors[0].scale, Some(2.0));
    }

    #[test]
    fn escape_leaves_without_applying_anything() {
        let mut a = app();
        a.selected_scale = 8;

        press(&mut a, KeyCode::Esc);

        assert_eq!(a.mode, TUIMode::View);
        assert_eq!(a.monitors[0].scale, None, "nothing was applied");
    }

    #[test]
    fn unrelated_keys_are_ignored() {
        let mut a = app();
        a.selected_scale = 3;

        for code in [KeyCode::Char('x'), KeyCode::Char('w'), KeyCode::Tab, KeyCode::F(1)] {
            press(&mut a, code);
            assert_eq!(a.selected_scale, 3, "{:?} must not navigate", code);
            assert_eq!(a.monitors[0].scale, None, "{:?} must not apply", code);
            assert_eq!(a.mode, TUIMode::Resolution, "{:?} must not leave", code);
        }
    }
}
