//! One-line hover tooltips.
//!
//! The placement is pure geometry so it can be unit-tested: the box sits
//! adjacent to the pointer, is clamped inside the screen and fades in over
//! three style steps once the pointer has rested long enough.

use crate::app::state::HoverTarget;
use crate::ui::components::PopupAction;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::time::Duration;

// How long the pointer must rest before the tooltip appears.
pub const TOOLTIP_DELAY: Duration = Duration::from_secs(2);

// How long each fade-in step lasts.
pub const TOOLTIP_FADE: Duration = Duration::from_millis(100);

// Maximum tooltip width in cells (including the border).
pub const TOOLTIP_MAX_WIDTH: u16 = 46;

// A tooltip ready to be drawn: its text, fade step and pointer anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooltipState {
    pub text: &'static str,
    // 0..=3; 3 is fully faded in.
    pub step: u8,
    // The pointer cell the box should avoid covering.
    pub anchor: (u16, u16),
}

// The one-line description for an interactive element.
pub fn text_for(target: HoverTarget) -> &'static str {
    match target {
        HoverTarget::Map(_) => "Drag to move, click to select",
        HoverTarget::ApplyButton(_) => "Click to apply the moved monitor",
        HoverTarget::ListRow(_) => "Click selects this display",
        HoverTarget::ListIcon(_) => "Click selects; click again toggles",
        HoverTarget::ListResolution(_) => "Click to choose a resolution",
        HoverTarget::ListScale(_) => "Click to choose a scale",
        HoverTarget::ListRotation(_) => "Click to rotate clockwise",
        HoverTarget::ListWorkspace(_) => "Click to assign a workspace",
        HoverTarget::WorkspaceDigit(_) => "Assign this workspace (0 clears)",
        HoverTarget::ResolutionRow(_) => "Click selects, double-click applies",
        HoverTarget::ScaleRow(_) => "Click selects, double-click applies",
        HoverTarget::PresetRow(_) => "Click previews, double-click applies",
        HoverTarget::PresetButton(action) => action.description(),
        HoverTarget::HelpClose => "Close the keybindings help",
        HoverTarget::HelpScrollbar => "Drag or click to scroll the help",
        HoverTarget::Action(action) => action.description(),
        HoverTarget::Popup(action) => match action {
            PopupAction::Close => "Close without applying",
            PopupAction::Accept => "Accept and write the configuration",
            PopupAction::Force => "Write the config despite validation errors",
            PopupAction::AutoFix => "Apply the suggested fix to the layout",
        },
    }
}

// Truncates `text` to `max` cells, ending with an ellipsis when cut.
fn fit_text(text: &str, max: u16) -> String {
    let max = max as usize;
    if text.chars().count() <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

// The tooltip rectangle for an anchor cell and one-line text.
//
// Prefers below-right of the pointer, flips to the other side when there is no
// room and finally clamps inside `area`.
pub fn tooltip_rect(area: Rect, anchor: (u16, u16), text: &str, max_width: u16) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::new(area.x, area.y, 0, 0);
    }
    let inner_width = max_width
        .saturating_sub(2)
        .min(area.width.saturating_sub(2));
    let text = fit_text(text, inner_width);
    let width = (text.chars().count() as u16)
        .saturating_add(2)
        .min(max_width)
        .min(area.width)
        .max(1);
    let height = 3u16.min(area.height);

    let anchor_rect = Rect::new(anchor.0, anchor.1, 1, 1);
    let mut x = anchor_rect.right().saturating_add(1);
    let mut y = anchor_rect.bottom().saturating_add(1);

    if x.saturating_add(width) > area.right() {
        x = anchor_rect.x.saturating_sub(width);
    }
    if y.saturating_add(height) > area.bottom() {
        y = anchor_rect.y.saturating_sub(height);
    }

    let max_x = area.right().saturating_sub(width);
    let max_y = area.bottom().saturating_sub(height);
    Rect::new(
        x.clamp(area.x, max_x.max(area.x)),
        y.clamp(area.y, max_y.max(area.y)),
        width,
        height,
    )
}

// The fade-in colour for a step: DarkGray -> Gray -> White.
fn fade_color(step: u8) -> Color {
    match step {
        0 => Color::DarkGray,
        1 => Color::Gray,
        _ => Color::White,
    }
}

// Renders the tooltip on top of everything else and returns its rect.
pub fn render(state: &TooltipState, area: Rect, buf: &mut Buffer) -> Rect {
    let inner_width = TOOLTIP_MAX_WIDTH.saturating_sub(2);
    let text = fit_text(state.text, inner_width);
    let rect = tooltip_rect(area, state.anchor, &text, TOOLTIP_MAX_WIDTH);
    if rect.width == 0 || rect.height == 0 {
        return rect;
    }

    let color = fade_color(state.step);
    Clear.render(rect, buf);
    Paragraph::new(Line::from(text).fg(color).bold())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color)),
        )
        .render(rect, buf);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltip_prefers_below_right_of_the_pointer() {
        let area = Rect::new(0, 0, 80, 24);
        let rect = tooltip_rect(area, (10, 5), "short", TOOLTIP_MAX_WIDTH);

        assert!(rect.x > 10, "to the right of the pointer");
        assert!(rect.y > 5, "below the pointer");
        assert!(rect.right() <= area.right());
        assert!(rect.bottom() <= area.bottom());
    }

    #[test]
    fn it_flips_left_and_up_near_the_edges() {
        let area = Rect::new(0, 0, 80, 24);
        let rect = tooltip_rect(area, (79, 23), "short", TOOLTIP_MAX_WIDTH);

        assert!(rect.right() <= area.right());
        assert!(rect.bottom() <= area.bottom());
        assert!(
            rect.x <= 79 && rect.y <= 23,
            "the box must not run off the screen"
        );
    }

    #[test]
    fn it_never_covers_the_anchor_when_there_is_room() {
        let area = Rect::new(0, 0, 80, 24);
        let anchor = (20, 10);
        let rect = tooltip_rect(area, anchor, "hello", TOOLTIP_MAX_WIDTH);

        let covers = anchor.0 >= rect.x
            && anchor.0 < rect.right()
            && anchor.1 >= rect.y
            && anchor.1 < rect.bottom();
        assert!(!covers, "the tooltip covers the pointer cell: {rect:?}");
    }

    #[test]
    fn long_text_is_ellipsized_to_the_max_width() {
        let area = Rect::new(0, 0, 120, 40);
        let text = "a very long description that cannot possibly fit into one line of the box";
        let rect = tooltip_rect(area, (5, 5), text, 20);

        assert!(rect.width <= 20);
    }

    #[test]
    fn it_is_clamped_inside_tiny_areas() {
        for (width, height) in [(0u16, 0u16), (1, 1), (3, 2), (10, 4), (2, 40), (40, 2)] {
            let area = Rect::new(0, 0, width, height);
            let rect = tooltip_rect(area, (0, 0), "a tooltip", 20);
            assert!(
                rect.x >= area.x
                    && rect.y >= area.y
                    && rect.right() <= area.right()
                    && rect.bottom() <= area.bottom(),
                "{rect:?} escapes {area:?}"
            );
        }
    }

    #[test]
    fn every_step_renders_within_the_area_and_fades() {
        let area = Rect::new(0, 0, 80, 24);
        let mut colors = Vec::new();
        for step in 0..=3u8 {
            let state = TooltipState {
                text: "Click to save your changes",
                step,
                anchor: (10, 10),
            };
            let mut buf = Buffer::empty(area);
            let rect = render(&state, area, &mut buf);
            assert!(rect.right() <= area.right());
            assert!(rect.bottom() <= area.bottom());
            colors.push(buf[(rect.x, rect.y)].style().fg);
        }

        assert_eq!(colors[0], Some(Color::DarkGray));
        assert_eq!(colors[1], Some(Color::Gray));
        assert_eq!(colors[2], Some(Color::White));
        assert_eq!(colors[3], Some(Color::White));
    }

    #[test]
    fn every_target_has_a_short_description() {
        for target in [
            HoverTarget::Map(0),
            HoverTarget::ApplyButton(0),
            HoverTarget::ListRow(0),
            HoverTarget::ListIcon(0),
            HoverTarget::ListResolution(0),
            HoverTarget::ListScale(0),
            HoverTarget::ListRotation(0),
            HoverTarget::ListWorkspace(0),
            HoverTarget::WorkspaceDigit(1),
            HoverTarget::ResolutionRow(0),
            HoverTarget::ScaleRow(0),
            HoverTarget::PresetRow(0),
            HoverTarget::PresetButton(crate::ui::components::PresetButtonAction::Apply),
            HoverTarget::PresetButton(crate::ui::components::PresetButtonAction::Close),
            HoverTarget::HelpClose,
            HoverTarget::HelpScrollbar,
            HoverTarget::Action(crate::ui::components::ActionId::Save),
            HoverTarget::Action(crate::ui::components::ActionId::Apply),
            HoverTarget::Popup(PopupAction::Close),
            HoverTarget::Popup(PopupAction::Accept),
            HoverTarget::Popup(PopupAction::Force),
            HoverTarget::Popup(PopupAction::AutoFix),
        ] {
            let text = text_for(target);
            assert!(!text.is_empty(), "{target:?} has no description");
            assert!(
                text.chars().count() as u16 <= TOOLTIP_MAX_WIDTH - 2,
                "{target:?} is too long: {text}"
            );
        }
    }
}
