//! Responsive popup rendering, content sizing and button geometry.
//!
//! The popup is one component for every modal message: the frame is sized from
//! its content (wrapping long lines instead of clipping them), and the footer
//! hints are rendered as clickable buttons that embed their key. Rendering and
//! mouse hit-testing share [`Popup::layout`], so a click always lands on the
//! button that was drawn.

use crate::app::state::Popup;
use crate::ui::layouts::fitted_rect;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Margin, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

// Popup width is capped at this percentage of the terminal.
pub const POPUP_MAX_WIDTH_PERCENT: u16 = 70;

// Below this width the popup refuses to shrink any further.
pub const POPUP_MIN_WIDTH: u16 = 20;

// Popup height is capped at this percentage of the terminal.
pub const POPUP_MAX_HEIGHT_PERCENT: u16 = 90;

// Horizontal padding inside the border, per side.
const HORIZONTAL_PADDING: u16 = 2;

// An action a popup button (and its key) dispatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupAction {
    Close,
    Accept,
    Force,
    AutoFix,
}

// A clickable footer button, positioned in screen cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupButton {
    pub action: PopupAction,
    pub rect: Rect,
}

// The popup's frame and buttons for one screen area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupFrame {
    pub frame: Rect,
    pub buttons: [Option<PopupButton>; 3],
}

impl Default for PopupFrame {
    fn default() -> Self {
        PopupFrame {
            frame: Rect::new(0, 0, 0, 0),
            buttons: [None; 3],
        }
    }
}

impl PopupFrame {
    // The action of the button under `point`, if any.
    pub fn button_at(&self, point: (u16, u16)) -> Option<PopupAction> {
        self.buttons
            .iter()
            .flatten()
            .find(|button| inside(button.rect, point))
            .map(|button| button.action)
    }

    // The rect of a specific button, if it is drawn.
    pub fn button(&self, action: PopupAction) -> Option<Rect> {
        self.buttons
            .iter()
            .flatten()
            .find(|button| button.action == action)
            .map(|button| button.rect)
    }

    // Whether no popup is present.
    pub fn is_empty(&self) -> bool {
        self.frame.width == 0 || self.frame.height == 0
    }
}

// Half-open containment so adjacent rects never both claim a cell.
fn inside(rect: Rect, point: (u16, u16)) -> bool {
    point.0 >= rect.x
        && point.0 < rect.right()
        && point.1 >= rect.y
        && point.1 < rect.bottom()
}

// A run of buttons on one footer row.
type ButtonRow = Vec<(String, PopupAction)>;

// Greedy word-wrap line count for one logical line at `width` columns.
//
// Mirrors `Paragraph`'s `Wrap { trim: false }` closely enough to size the
// frame; the paragraph itself does the real wrapping.
fn wrapped_line_count(text: &str, width: u16) -> usize {
    if width == 0 {
        return 0;
    }
    let width = width as usize;
    if text.is_empty() {
        return 1;
    }
    let mut lines = 1;
    let mut current = 0usize;
    for word in text.split(' ') {
        let word_len = word.chars().count();
        if word_len > width {
            if current > 0 {
                lines += 1;
            }
            let mut remaining = word_len;
            while remaining > width {
                lines += 1;
                remaining -= width;
            }
            current = remaining;
            continue;
        }
        let needed = if current == 0 {
            word_len
        } else {
            current + 1 + word_len
        };
        if current > 0 && needed > width {
            lines += 1;
            current = word_len;
        } else {
            current = needed;
        }
    }
    lines
}

// Greedily wraps button labels into rows that fit `width`.
fn wrap_buttons(labels: &[(String, PopupAction)], width: u16) -> Vec<ButtonRow> {
    let mut rows: Vec<ButtonRow> = Vec::new();
    let mut row: ButtonRow = Vec::new();
    let mut row_width: u16 = 0;

    for (label, action) in labels {
        let label_width = label.chars().count() as u16;
        let needed = if row.is_empty() {
            label_width
        } else {
            row_width + 1 + label_width
        };
        if !row.is_empty() && needed > width {
            rows.push(std::mem::take(&mut row));
            row_width = 0;
        }
        row_width = if row.is_empty() {
            label_width
        } else {
            row_width + 1 + label_width
        };
        row.push((label.clone(), *action));
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

impl Popup {
    // The footer actions offered for this popup, in display order.
    pub fn actions(&self) -> Vec<PopupAction> {
        let mut actions = Vec::new();
        if self.is_error {
            if self.is_forceable {
                actions.push(PopupAction::Force);
            }
            if self.fix.is_some() {
                actions.push(PopupAction::AutoFix);
            }
        } else {
            actions.push(PopupAction::Accept);
        }
        actions.push(PopupAction::Close);
        actions
    }

    // The button label, with the key it embeds.
    pub fn label(&self, action: PopupAction) -> String {
        match action {
            PopupAction::Force => "[ f force write ]".to_string(),
            PopupAction::AutoFix => "[ Space auto fix ]".to_string(),
            PopupAction::Accept if !self.is_error && self.is_forceable => {
                "[ Enter accept anyway ]".to_string()
            }
            PopupAction::Accept => "[ Enter accept ]".to_string(),
            PopupAction::Close if self.is_error => "[ Esc close ]".to_string(),
            PopupAction::Close => "[ Esc cancel ]".to_string(),
        }
    }

    // The message lines, without the footer.
    fn content_lines(&self) -> Vec<Line<'static>> {
        self.lines.iter().map(|line| Line::from(line.clone())).collect()
    }

    // Computes the content-sized frame and the button rects for `area`.
    pub fn layout(&self, area: Rect) -> PopupFrame {
        if area.width == 0 || area.height == 0 {
            return PopupFrame::default();
        }

        let actions = self.actions();
        let labels: Vec<(String, PopupAction)> = actions
            .iter()
            .map(|action| (self.label(*action), *action))
            .collect();

        // Width: content sized, min 20, max 70% of the screen.
        let max_width = area
            .width
            .min(((area.width as u32 * POPUP_MAX_WIDTH_PERCENT as u32 / 100) as u16).max(1))
            .max(1);
        let min_width = POPUP_MIN_WIDTH.min(max_width);
        let longest = self
            .lines
            .iter()
            .map(|line| line.chars().count() as u16)
            .max()
            .unwrap_or(0);
        let footer_width: u16 = labels
            .iter()
            .map(|(label, _)| label.chars().count() as u16)
            .sum::<u16>()
            .saturating_add(labels.len().saturating_sub(1) as u16);
        let desired_width = longest
            .max(footer_width)
            .saturating_add(2 * HORIZONTAL_PADDING + 2);
        let width = desired_width.min(max_width).max(min_width);
        let inner_width = width.saturating_sub(2 * HORIZONTAL_PADDING + 2).max(1);

        // Height: wrapped content + wrapped footer + borders and padding.
        let content_height: u16 = self
            .lines
            .iter()
            .map(|line| wrapped_line_count(line, inner_width))
            .sum::<usize>()
            .max(1) as u16;
        let footer_rows = wrap_buttons(&labels, inner_width);
        let footer_height = footer_rows.len() as u16;

        let max_height = area
            .height
            .min(((area.height as u32 * POPUP_MAX_HEIGHT_PERCENT as u32 / 100) as u16).max(1))
            .max(1);
        let desired_height = content_height
            .saturating_add(footer_height)
            .saturating_add(3); // one blank separator line + two borders
        let height = desired_height.min(max_height).max(3.min(area.height));

        let frame = fitted_rect(area, width, height);
        let content_area = frame.inner(Margin::new(HORIZONTAL_PADDING, 1));
        // The footer is anchored to the bottom of the padded area so it stays
        // visible even when the content has to clip on a tiny terminal.
        let footer_top = content_area
            .bottom()
            .saturating_sub(footer_height)
            .max(content_area.top());

        let mut buttons = [None; 3];
        let mut index = 0;
        for (row, row_buttons) in footer_rows.iter().enumerate() {
            let row_width: u16 = row_buttons
                .iter()
                .map(|(label, _)| label.chars().count() as u16)
                .sum::<u16>()
                .saturating_add(row_buttons.len().saturating_sub(1) as u16);
            let mut x = content_area.x + content_area.width.saturating_sub(row_width) / 2;
            let y = footer_top.saturating_add(row as u16);
            for (label, action) in row_buttons {
                if index >= buttons.len() {
                    break;
                }
                let button_width = label.chars().count() as u16;
                buttons[index] = Some(PopupButton {
                    action: *action,
                    rect: Rect::new(x, y, button_width, 1),
                });
                index += 1;
                x = x.saturating_add(button_width).saturating_add(1);
            }
        }

        PopupFrame { frame, buttons }
    }

    // Draws the popup and returns the geometry used, for hit-testing.
    pub fn render(&self, area: Rect, buf: &mut Buffer) -> PopupFrame {
        self.render_with(area, buf, None, 0)
    }

    // Draws the popup with hover styling for the button under the pointer.
    pub fn render_with(
        &self,
        area: Rect,
        buf: &mut Buffer,
        hover: Option<crate::app::state::HoverTarget>,
        hover_tick: u32,
    ) -> PopupFrame {
        let layout = self.layout(area);
        let frame = layout.frame;
        if layout.is_empty() {
            return layout;
        }

        let color = if self.is_error {
            Color::Red
        } else {
            Color::Yellow
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color))
            .title(Line::from(self.title.clone().bold().white()));

        Clear.render(frame, buf);
        Widget::render(block, frame, buf);

        let content_area = frame.inner(Margin::new(HORIZONTAL_PADDING, 1));
        let footer_top = layout
            .buttons
            .iter()
            .flatten()
            .map(|button| button.rect.y)
            .min()
            .unwrap_or(content_area.bottom());
        let content_height = footer_top.saturating_sub(content_area.y);
        if content_height > 0 {
            Paragraph::new(self.content_lines())
                .wrap(Wrap { trim: false })
                .alignment(Alignment::Center)
                .render(
                    Rect {
                        width: content_area.width,
                        height: content_height,
                        ..content_area
                    },
                    buf,
                );
        }

        for button in layout.buttons.iter().flatten() {
            let hovered = hover == Some(crate::app::state::HoverTarget::Popup(button.action));
            let style = if hovered {
                // Inverted, bold and colour-shifted: an obvious "click me".
                crate::ui::styles::hover_style(hover_tick).reversed()
            } else {
                Style::default().fg(Color::Gray)
            };
            Paragraph::new(Line::from(self.label(button.action)).style(style))
                .alignment(Alignment::Left)
                .render(button.rect, buf);
        }

        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::find_text;

    fn popup(is_error: bool, is_forceable: bool, lines: &[&str]) -> Popup {
        Popup {
            title: "Warning".to_string(),
            lines: lines.iter().map(|line| line.to_string()).collect(),
            is_error,
            apply_preset: None,
            is_forceable,
            fix: None,
        }
    }

    fn render(popup: &Popup, area: Rect) -> (Buffer, PopupFrame) {
        let mut buf = Buffer::empty(area);
        let layout = popup.render(area, &mut buf);
        (buf, layout)
    }

    fn text(buf: &Buffer) -> String {
        crate::test_utils::tests::text_of(buf)
    }

    #[test]
    fn popup_renders_title_and_message() {
        let (buf, _) = render(&popup(false, false, &["All good"]), Rect::new(0, 0, 60, 20));
        let text = text(&buf);

        assert!(text.contains("Warning"), "missing title: {text}");
        assert!(text.contains("All good"), "missing message: {text}");
    }

    #[test]
    fn popup_renders_every_line_of_a_multi_line_message() {
        let (buf, _) = render(
            &popup(false, false, &["first line", "second line", "third line"]),
            Rect::new(0, 0, 60, 24),
        );
        let text = text(&buf);

        for line in ["first line", "second line", "third line"] {
            assert!(text.contains(line), "missing {line:?}: {text}");
        }
    }

    #[test]
    fn a_long_line_is_wrapped_instead_of_clipped() {
        let message = "Monitors not contiguous. Disconnected: a-very-long-monitor-name, another-very-long-monitor-name";
        let (buf, layout) = render(&popup(true, false, &[message]), Rect::new(0, 0, 60, 24));
        let text = text(&buf);

        assert!(
            text.contains("another-very-long-monitor-name"),
            "the tail of the wrapped line must stay visible: {text}"
        );
        assert!(layout.frame.width <= 60);
    }

    #[test]
    fn the_frame_is_content_sized_within_its_caps() {
        let area = Rect::new(0, 0, 120, 40);
        let (_, small) = render(&popup(false, false, &["hi"]), area);
        let (_, wide) = render(
            &popup(false, false, &["a message that is clearly longer than the minimum width"]),
            area,
        );

        assert!(
            small.frame.width >= POPUP_MIN_WIDTH,
            "the frame keeps a usable minimum: {:?}",
            small.frame
        );
        assert!(wide.frame.width > small.frame.width);
        assert!(wide.frame.width <= area.width * POPUP_MAX_WIDTH_PERCENT / 100 + 1);
        assert!(wide.frame.width < area.width, "never the whole screen");
    }

    #[test]
    fn error_popups_are_red_and_offer_force_and_close() {
        let area = Rect::new(0, 0, 60, 20);
        let (buf, layout) = render(&popup(true, true, &["Monitor is busy"]), area);

        assert_eq!(
            buf[(layout.frame.x, layout.frame.y)].style().fg,
            Some(Color::Red),
            "error popups must have a red border"
        );
        assert!(layout.button(PopupAction::Force).is_some());
        assert!(layout.button(PopupAction::Close).is_some());
        let text = text(&buf);
        assert!(text.contains("[ f force write ]"), "got: {text}");
        assert!(text.contains("[ Esc close ]"), "got: {text}");
    }

    #[test]
    fn info_popups_offer_accept_and_cancel_without_force() {
        let (buf, layout) = render(&popup(false, true, &["missing"]), Rect::new(0, 0, 80, 20));

        assert!(layout.button(PopupAction::Accept).is_some());
        assert!(layout.button(PopupAction::Close).is_some());
        assert!(layout.button(PopupAction::Force).is_none());
        let text = text(&buf);
        assert!(text.contains("[ Enter accept anyway ]"), "got: {text}");
        assert!(text.contains("[ Esc cancel ]"), "got: {text}");
        assert!(!text.contains("force write"), "got: {text}");
    }

    #[test]
    fn a_validation_popup_offers_the_auto_fix_button() {
        let mut p = popup(true, true, &["Monitors overlap: A and B"]);
        p.fix = Some(crate::validation::diagnostics::AutoFix::Separate {
            monitor: 0,
            dx: 1,
            dy: 0,
        });
        let (buf, layout) = render(&p, Rect::new(0, 0, 100, 30));

        assert!(layout.button(PopupAction::AutoFix).is_some());
        assert!(layout.button(PopupAction::Force).is_some());
        assert!(layout.button(PopupAction::Close).is_some());
        assert!(text(&buf).contains("[ Space auto fix ]"));
    }

    #[test]
    fn button_rects_lie_inside_the_frame() {
        let area = Rect::new(0, 0, 100, 30);
        let (_, layout) = render(&popup(true, true, &["boom"]), area);

        for button in layout.buttons.iter().flatten() {
            assert!(
                button.rect.x >= layout.frame.x
                    && button.rect.right() <= layout.frame.right()
                    && button.rect.y >= layout.frame.y
                    && button.rect.bottom() <= layout.frame.bottom(),
                "button {:?} escapes {:?}",
                button.rect,
                layout.frame
            );
        }
    }

    #[test]
    fn button_at_maps_the_drawn_cells() {
        let area = Rect::new(0, 0, 100, 30);
        let (_, layout) = render(&popup(true, true, &["boom"]), area);

        for action in [PopupAction::Force, PopupAction::Close] {
            let rect = layout.button(action).expect("button");
            assert_eq!(
                layout.button_at((rect.x + 1, rect.y)),
                Some(action),
                "a click on {action:?} must map back to it"
            );
        }
        // A point outside every button and outside the frame is not a hit.
        assert_eq!(layout.button_at((0, 0)), None);
    }

    #[test]
    fn popup_occupies_only_its_fitted_frame() {
        let area = Rect::new(0, 0, 80, 24);
        let (buf, layout) = render(&popup(false, false, &["hi"]), area);

        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let inside_frame = x >= layout.frame.x
                    && x < layout.frame.right()
                    && y >= layout.frame.y
                    && y < layout.frame.bottom();
                if inside_frame {
                    continue;
                }
                assert_eq!(
                    buf[(x, y)].symbol(),
                    " ",
                    "content spilled at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn popup_without_lines_still_renders_a_frame() {
        let (buf, layout) = render(&popup(false, false, &[]), Rect::new(0, 0, 40, 12));

        assert!(text(&buf).contains("Warning"));
        assert!(layout.frame.width > 0);
    }

    // A tiny screen must wrap the footer rather than lose its buttons.
    #[test]
    fn the_footer_wraps_on_narrow_screens() {
        let area = Rect::new(0, 0, 22, 20);
        let (buf, layout) = render(&popup(true, true, &["overlap"]), area);

        assert!(layout.button(PopupAction::Force).is_some());
        assert!(layout.button(PopupAction::Close).is_some());
        let force = layout.button(PopupAction::Force).unwrap();
        let close = layout.button(PopupAction::Close).unwrap();
        assert!(force.y < close.y || force.x < close.x, "buttons must not overlap");
        assert!(text(&buf).contains("force"));
    }

    #[test]
    fn a_hovered_button_is_reversed_and_the_others_are_not() {
        use crate::app::state::HoverTarget;
        use ratatui::style::Modifier;

        let area = Rect::new(0, 0, 100, 30);
        let p = popup(true, true, &["boom"]);
        let mut buf = Buffer::empty(area);
        let layout = p.render_with(
            area,
            &mut buf,
            Some(HoverTarget::Popup(PopupAction::Close)),
            0,
        );

        let close = layout.button(PopupAction::Close).expect("close");
        assert!(
            buf[(close.x, close.y)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED),
            "the hovered button is highlighted"
        );
        let force = layout.button(PopupAction::Force).expect("force");
        assert!(
            !buf[(force.x, force.y)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED),
            "the other button stays plain"
        );
    }

    #[test]
    fn popup_survives_areas_that_are_smaller_than_itself() {        for (width, height) in [
            (0u16, 0u16),
            (1, 1),
            (4, 2),
            (10, 3),
            (20, 8),
            (2, 40),
        ] {
            let area = Rect::new(0, 0, width, height);
            let mut buf = Buffer::empty(area);
            popup(true, true, &["x"]).render(area, &mut buf);
        }
    }

    #[test]
    fn every_rendered_cell_stays_inside_the_area() {
        // find_text is only a smoke test here; the real assertion is that
        // rendering to a matrix of sizes never panics.
        for (width, height) in [(0u16, 0u16), (20, 8), (40, 12), (120, 40), (240, 80)] {
            let area = Rect::new(0, 0, width, height);
            let (buf, _) = render(&popup(true, true, &["a longish message that needs wrapping"]), area);
            assert_eq!(buf.area, area);
            let _ = find_text(&buf, "Warning");
        }
    }
}
