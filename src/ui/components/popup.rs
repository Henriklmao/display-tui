//! Popup component for errors and messages.

use crate::ui::layouts::centered_rect;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// Popup message display.
pub struct Popup {
    pub title: String,
    pub lines: Vec<String>,
    pub is_error: bool,
}

impl Popup {
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let popup_area = centered_rect(50, 40, area);
        let mut text = vec![Line::from("")];
        for line in &self.lines {
            text.push(Line::from(line.clone()));
            text.push(Line::from(""));
        }
        if self.is_error {
            text.push(Line::from(
                "Press <f> to force, or <Esc/Enter/q> to close.".gray(),
            ));
        }
        let color = if self.is_error {
            Color::Red
        } else {
            Color::Yellow
        };
        let border_style = Style::default().fg(color);
        let title = Line::from(self.title.clone().bold().white());

        let p = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style),
            )
            .alignment(ratatui::layout::Alignment::Center);

        Clear.render(popup_area, buf);
        p.render(popup_area, buf);
    }
}
