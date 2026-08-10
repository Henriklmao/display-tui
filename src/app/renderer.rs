//! UI rendering for the display-tui application.

use super::state::App;
use crate::ui::components::{HelpModal, Map, MonitorList, Resolutions, Scale};
use crate::utils::TUIMode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let is_previewing = self.show_preset_menu.is_some();

        let mut monitor_list =
            MonitorList::new(&self.monitors, self.mode, Some(self.selected_monitor));

        let canvas = Map {
            selected: self.selected_monitor,
            monitors: &self.monitors,
            is_previewing,
        };
        monitor_list.is_previewing = is_previewing;
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        match self.mode {
            TUIMode::Resolution => {
                let selected = &self.monitors[self.selected_monitor];
                let mut resolutions = Resolutions::new(selected, Some(self.selected_resolution));
                let inner_top_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(outer_layout[0]);
                canvas.render(inner_top_layout[0], buf);
                resolutions.render(inner_top_layout[1], buf);
            }
            TUIMode::Scale => {
                let mut scale = Scale::new(self.selected_scale);
                let inner_top_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![Constraint::Percentage(90), Constraint::Percentage(10)])
                    .split(outer_layout[0]);
                canvas.render(inner_top_layout[0], buf);
                scale.render(inner_top_layout[1], buf);
            }
            _ => {
                canvas.render(outer_layout[0], buf);
            }
        }
        monitor_list.render(outer_layout[1], buf);

        if let Some(ref preset_menu) = self.show_preset_menu {
            preset_menu.render(area, buf);
        }

        if self.show_help {
            HelpModal::render(area, buf);
        }

        if let Some(ref popup) = self.show_popup {
            let popup_area = centered_rect(50, 40, area);
            let mut text = vec![Line::from("")];
            for line in &popup.lines {
                text.push(Line::from(line.clone()));
                text.push(Line::from(""));
            }
            if popup.is_error && popup.is_forceable {
                text.push(Line::from(
                    "Press <f> to force write, or <Esc/Enter> to close.".gray(),
                ));
            } else if popup.is_error {
                text.push(Line::from("<Esc/Enter> to close.".gray()));
            }
            let color = if popup.is_error {
                Color::Red
            } else {
                Color::Yellow
            };
            let border_style = Style::default().fg(color);
            let title = Line::from(popup.title.clone().bold().white());

            let p = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .border_style(border_style),
                )
                .alignment(Alignment::Center);

            Clear.render(popup_area, buf);
            p.render(popup_area, buf);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
