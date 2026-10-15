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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::layouts::centered_rect;

    fn popup(is_error: bool, lines: &[&str]) -> Popup {
        Popup {
            title: "Warning".to_string(),
            lines: lines.iter().map(|l| l.to_string()).collect(),
            is_error,
        }
    }

    // Flattens the buffer into readable text so content can be asserted on.
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

    fn render(p: &Popup, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf);
        buf
    }

    #[test]
    fn popup_renders_title_and_message() {
        let buf = render(&popup(false, &["All good"]), Rect::new(0, 0, 60, 20));
        let text = text_of(&buf);

        assert!(text.contains("Warning"), "missing title: {text}");
        assert!(text.contains("All good"), "missing message: {text}");
    }

    #[test]
    fn popup_renders_every_line_of_a_multi_line_message() {
        let buf = render(
            &popup(false, &["first line", "second line", "third line"]),
            Rect::new(0, 0, 60, 24),
        );
        let text = text_of(&buf);

        for line in ["first line", "second line", "third line"] {
            assert!(text.contains(line), "missing {line:?}: {text}");
        }
    }

    #[test]
    fn error_popup_is_red_and_offers_the_force_hint() {
        let buf = render(
            &popup(true, &["Monitor is busy"]),
            Rect::new(0, 0, 60, 20),
        );

        let popup_area = centered_rect(50, 40, Rect::new(0, 0, 60, 20));
        assert_eq!(
            buf[(popup_area.x, popup_area.y)].style().fg,
            Some(Color::Red),
            "error popups must have a red border"
        );

        let text = text_of(&buf);
        assert!(
            text.contains("Press <f> to force"),
            "error popups must show the key hint: {text}"
        );
    }

    #[test]
    fn info_popup_is_yellow_and_has_no_key_hint() {
        let buf = render(&popup(false, &["Saved"]), Rect::new(0, 0, 60, 20));

        let popup_area = centered_rect(50, 40, Rect::new(0, 0, 60, 20));
        assert_eq!(
            buf[(popup_area.x, popup_area.y)].style().fg,
            Some(Color::Yellow),
            "info popups must have a yellow border"
        );

        let text = text_of(&buf);
        assert!(
            !text.contains("Press <f> to force"),
            "info popups must not show the force hint: {text}"
        );
    }

    #[test]
    fn popup_occupies_only_the_middle_of_the_area() {
        let area = Rect::new(0, 0, 80, 24);
        let buf = render(&popup(false, &["hi"]), area);
        let popup_area = centered_rect(50, 40, area);

        assert!(
            popup_area.x > area.x && popup_area.y > area.y,
            "popup is not inset: {:?}",
            popup_area
        );
        assert!(
            popup_area.x + popup_area.width < area.x + area.width,
            "popup touches the right edge"
        );

        // Nothing may spill outside the popup rectangle: every cell outside
        // of it has to remain an untouched blank.
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if x >= popup_area.x
                    && x < popup_area.x + popup_area.width
                    && y >= popup_area.y
                    && y < popup_area.y + popup_area.height
                {
                    continue;
                }
                let cell = &buf[(x, y)];
                assert_eq!(
                    cell.symbol(),
                    " ",
                    "content spilled outside the popup at ({}, {})",
                    x,
                    y
                );
                assert_eq!(cell.style().fg, Some(Color::Reset));
            }
        }
    }

    #[test]
    fn popup_without_lines_still_renders_a_frame() {
        let buf = render(&popup(false, &[]), Rect::new(0, 0, 40, 12));
        let text = text_of(&buf);

        assert!(text.contains("Warning"));
        assert!(!text.contains("Press <f> to force"));
    }

    #[test]
    fn popup_survives_areas_that_are_smaller_than_itself() {
        for (width, height) in [(0u16, 0u16), (1, 1), (4, 2), (10, 3), (2, 40)] {
            let area = Rect::new(0, 0, width, height);
            let mut buf = Buffer::empty(area);
            popup(true, &["x"]).render(area, &mut buf);
        }
    }
}
