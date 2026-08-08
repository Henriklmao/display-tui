//! Help modal component.

use crate::ui::layouts::centered_rect;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// Help modal display.
pub struct HelpModal;

impl HelpModal {
    pub fn render(area: Rect, buf: &mut Buffer) {
        let popup_area = centered_rect(60, 60, area);
        let text = vec![
            Line::from(
                format!("Display-TUI v{}", env!("CARGO_PKG_VERSION"))
                    .bold()
                    .blue(),
            ),
            Line::from(""),
            Line::from(" --- Global --- ".bold().yellow()),
            Line::from("Save <w> | Quit <q>"),
            Line::from(""),
            Line::from(" --- View Mode --- ".bold().yellow()),
            Line::from("Up <k> | Down <j>"),
            Line::from("Move <m> | Resolution <r> | Scale <s>"),
            Line::from("Rotate <o>"),
            Line::from("Select Workspace for selected screen <0-9>"),
            Line::from("Enable <e> | Disable <d>"),
            Line::from("Show Keybindings <K>"),
            Line::from(""),
            Line::from(" --- Move Mode --- ".bold().yellow()),
            Line::from("Freemove <H/J/K/L> | Snapmove <h/j/k/l>"),
            Line::from("Quit and apply <Esc/Enter/>"),
            Line::from(""),
            Line::from(" --- Res / Scale Mode --- ".bold().yellow()),
            Line::from("Select <Space/Enter> | Quit <Esc> | Up/Down <k/j>"),
            Line::from(""),
            Line::from(" --- Tip --- ".bold().yellow()),
            Line::from(
                "Arrow keys can be used as alternative to h/j/k/l for navigation in all modes."
                    .light_blue(),
            ),
        ];

        let p = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Keybindings/Help ".bold().white())
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .alignment(Alignment::Center);

        Clear.render(popup_area, buf);
        p.render(popup_area, buf);
    }
}
