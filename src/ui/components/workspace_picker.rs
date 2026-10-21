//! Small workspace picker popover.
//!
//! The picker is anchored above the workspace cell it belongs to. Rendering and
//! hit-testing share [`WorkspacePicker::rect`] and [`WorkspacePicker::digit_at`],
//! so a click always lands on the drawn digit. `0` clears the assignment.

use crate::app::state::HoverTarget;
use crate::ui::styles;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// Numeric order used in the grid: 1-5 on top, 6-0 below.
const DIGITS: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
const COLUMNS: usize = 5;
const CHIP_WIDTH: u16 = 3;
const GAP: u16 = 1;
const PICKER_WIDTH: u16 =
    COLUMNS as u16 * CHIP_WIDTH + (COLUMNS as u16 - 1) * GAP + 2;
const PICKER_HEIGHT: u16 = 4;

// The popover rectangle for a cell anchor, clamped inside `area`.
pub fn rect(anchor: Rect, area: Rect) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }

    let width = PICKER_WIDTH.min(area.width);
    let height = PICKER_HEIGHT.min(area.height);

    // Prefer above the cell, but flip below when the whole box does not fit.
    let mut y = anchor
        .y
        .checked_sub(height)
        .filter(|y| *y >= area.y)
        .unwrap_or_else(|| anchor.bottom());
    let mut x = anchor
        .x
        .saturating_add(anchor.width / 2)
        .saturating_sub(width / 2);

    x = x.clamp(area.x, area.right().saturating_sub(width).max(area.x));
    y = y.clamp(area.y, area.bottom().saturating_sub(height).max(area.y));
    Rect::new(x, y, width, height)
}

// The drawn digit chips for an anchor cell.
fn digit_rects(anchor: Rect, area: Rect) -> Vec<(u8, Rect)> {
    let frame = rect(anchor, area);
    let inner = Block::bordered().inner(frame);
    if inner.width == 0 || inner.height == 0 {
        return Vec::new();
    }

    let total_width =
        COLUMNS as u16 * CHIP_WIDTH + (COLUMNS as u16 - 1) * GAP;
    let start_x = inner
        .x
        .saturating_add(inner.width.saturating_sub(total_width) / 2);

    DIGITS
        .iter()
        .enumerate()
        .filter_map(|(index, digit)| {
            let column = (index % COLUMNS) as u16;
            let row = (index / COLUMNS) as u16;
            let chip = Rect {
                x: start_x.saturating_add(column * (CHIP_WIDTH + GAP)),
                y: inner.y.saturating_add(row),
                width: CHIP_WIDTH,
                height: 1,
            };
            (chip.x >= inner.x
                && chip.right() <= inner.right()
                && chip.bottom() <= inner.bottom())
            .then_some((*digit, chip))
        })
        .collect()
}

// The drawn chip for one digit, if it fits in the popover.
pub fn digit_rect(anchor: Rect, area: Rect, digit: u8) -> Option<Rect> {
    digit_rects(anchor, area)
        .into_iter()
        .find(|(candidate, _)| *candidate == digit)
        .map(|(_, chip)| chip)
}

// The digit under a screen point, if one is drawn there.
pub fn digit_at(anchor: Rect, area: Rect, point: (u16, u16)) -> Option<u8> {
    digit_rects(anchor, area)
        .into_iter()
        .find(|(_, chip)| {
            point.0 >= chip.x
                && point.0 < chip.right()
                && point.1 >= chip.y
                && point.1 < chip.bottom()
        })
        .map(|(digit, _)| digit)
}

// Draws the picker and highlights the hovered digit. Returns its rectangle.
pub fn render(
    anchor: Rect,
    area: Rect,
    buf: &mut Buffer,
    hover: Option<HoverTarget>,
) -> Rect {
    let frame = rect(anchor, area);
    if frame.width == 0 || frame.height == 0 {
        return frame;
    }

    Clear.render(frame, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Workspace ")
        .border_style(Style::default().fg(styles::COLOR_PRIMARY));
    Widget::render(block, frame, buf);

    for (digit, chip) in digit_rects(anchor, area) {
        let label = format!("[{}]", digit);
        let hovered = hover == Some(HoverTarget::WorkspaceDigit(digit));
        let style = if hovered {
            styles::hover_style().reversed()
        } else {
            Style::default().fg(Color::White)
        };
        Paragraph::new(Line::from(label).style(style)).render(chip, buf);
    }

    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{find_text, text_of};

    fn area() -> Rect {
        Rect::new(0, 0, 120, 40)
    }

    fn anchor() -> Rect {
        Rect::new(50, 30, 10, 1)
    }

    #[test]
    fn the_picker_sits_above_its_cell_and_inside_the_area() {
        let frame = rect(anchor(), area());

        assert!(frame.bottom() <= anchor().top(), "prefers above the cell");
        assert!(frame.x >= area().x && frame.right() <= area().right());
        assert!(frame.y >= area().y && frame.bottom() <= area().bottom());
        assert_eq!(frame.width, PICKER_WIDTH);
        assert_eq!(frame.height, PICKER_HEIGHT);
    }

    #[test]
    fn it_flips_below_when_there_is_no_room_above() {
        let anchor = Rect::new(50, 0, 10, 1);
        let frame = rect(anchor, area());

        assert!(frame.y >= anchor.bottom(), "flips below the cell");
        assert!(frame.bottom() <= area().bottom());
    }

    #[test]
    fn every_digit_has_a_chip_inside_the_frame() {
        let frame = rect(anchor(), area());
        let chips = digit_rects(anchor(), area());

        assert_eq!(chips.len(), 10, "all digits are drawn at this size");
        for (digit, chip) in chips {
            assert!(digit <= 9);
            assert!(
                chip.x >= frame.x
                    && chip.right() <= frame.right()
                    && chip.y >= frame.y
                    && chip.bottom() <= frame.bottom(),
                "digit {digit} escapes {frame:?}: {chip:?}"
            );
        }
    }

    #[test]
    fn digit_at_matches_the_drawn_chips() {
        for (digit, chip) in digit_rects(anchor(), area()) {
            assert_eq!(digit_at(anchor(), area(), (chip.x, chip.y)), Some(digit));
            assert_eq!(
                digit_at(anchor(), area(), (chip.right() - 1, chip.y)),
                Some(digit)
            );
        }
        assert_eq!(digit_at(anchor(), area(), (0, 0)), None);
    }

    #[test]
    fn the_hovered_digit_is_inverted() {
        let mut buf = Buffer::empty(area());
        let chips = digit_rects(anchor(), area());
        let (digit, chip) = chips[3];

        render(
            anchor(),
            area(),
            &mut buf,
            Some(HoverTarget::WorkspaceDigit(digit)),
        );

        assert!(
            buf[(chip.x, chip.y)]
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::REVERSED),
            "the hovered chip must be inverted"
        );
    }

    #[test]
    fn zero_is_labelled_and_clears() {
        let mut buf = Buffer::empty(area());
        render(anchor(), area(), &mut buf, None);

        let text = text_of(&buf);
        assert!(text.contains("Workspace"), "title missing: {text}");
        assert!(text.contains("[0]"), "clear digit missing: {text}");
        assert!(find_text(&buf, "[0]").is_some());
    }

    #[test]
    fn tiny_areas_do_not_panic() {
        for (width, height) in [(0u16, 0u16), (1, 1), (5, 3), (20, 8), (40, 12)] {
            let area = Rect::new(0, 0, width, height);
            let mut buf = Buffer::empty(area);
            let _ = render(anchor(), area, &mut buf, None);
            let _ = digit_at(anchor(), area, (0, 0));
        }
    }
}
