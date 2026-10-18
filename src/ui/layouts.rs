//! UI layout definitions.

use crate::ui::components::popup::PopupFrame;
use crate::utils::TUIMode;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

// Canvas area height percentage.
pub const CANVAS_HEIGHT_PERCENT: u16 = 70;

// List area height percentage.
pub const LIST_HEIGHT_PERCENT: u16 = 30;

// Width percentage of the resolution table in Resolution mode.
pub const RESOLUTION_TABLE_WIDTH_PERCENT: u16 = 30;

// Width percentage of the scale table in Scale mode.
pub const SCALE_TABLE_WIDTH_PERCENT: u16 = 10;

// Rectangles of every interactive region of the main screen.
//
// This is the single source of truth shared by rendering and mouse
// hit-testing: the renderer lays the widgets out from it and the mouse
// handler maps a click back onto the same rectangles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenLayout {
    // The whole terminal area.
    pub area: Rect,
    // The monitor map canvas.
    pub canvas: Rect,
    // The resolution or scale table, when the mode shows one.
    pub side_table: Option<Rect>,
    // The monitor list at the bottom.
    pub list: Rect,
    // The open popup's frame and button rects, when one is drawn.
    pub popup: Option<PopupFrame>,
}

// Computes the interactive rectangles for the current mode.
pub fn screen_layout(area: Rect, mode: TUIMode) -> ScreenLayout {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(CANVAS_HEIGHT_PERCENT),
            Constraint::Percentage(LIST_HEIGHT_PERCENT),
        ])
        .split(area);

    let (canvas, side_table) = match mode {
        TUIMode::Resolution => {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(100 - RESOLUTION_TABLE_WIDTH_PERCENT),
                    Constraint::Percentage(RESOLUTION_TABLE_WIDTH_PERCENT),
                ])
                .split(outer[0]);
            (split[0], Some(split[1]))
        }
        TUIMode::Scale => {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(100 - SCALE_TABLE_WIDTH_PERCENT),
                    Constraint::Percentage(SCALE_TABLE_WIDTH_PERCENT),
                ])
                .split(outer[0]);
            (split[0], Some(split[1]))
        }
        _ => (outer[0], None),
    };

    ScreenLayout {
        area,
        canvas,
        side_table,
        list: outer[1],
        popup: None,
    }
}

// Index of the first visible row of a table that keeps `selected` in view.
//
// The tables are re-selected from scratch every frame (offset 0) and every
// row is exactly one line tall, so ratatui scrolls just far enough to show
// the selected row at the bottom of a `viewport_rows` tall window. This
// mirrors that behaviour for hit-testing; a regression test renders a
// scrolled table and asserts a click lands on the expected row.
pub fn scrolled_offset(selected: usize, viewport_rows: usize) -> usize {
    selected.saturating_sub(viewport_rows.saturating_sub(1))
}

// Maps a screen row inside a table's row window to a row index.
//
// `top` is the first row of the window, `viewport_rows` its height and
// `offset` its scroll offset. Returns None for clicks above the window, in
// empty space past the last row, or outside the visible scroll window.
pub fn row_in_window(
    top: u16,
    viewport_rows: u16,
    offset: usize,
    total: usize,
    y: u16,
) -> Option<usize> {
    if viewport_rows == 0 || total == 0 {
        return None;
    }
    if y < top {
        return None;
    }
    let local = (y - top) as usize;
    if local >= viewport_rows as usize {
        return None;
    }
    let index = local + offset;
    if index < total { Some(index) } else { None }
}

// Centers a `width` x `height` box inside `area`, clamped to fit.
//
// Unlike [`centered_rect`] this sizes the box from its content, so a popup
// grows or shrinks with the message and never clips a long error line.
pub fn fitted_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

// Creates a centered popup rectangle.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::centered_rect;
    use ratatui::layout::Rect;

    #[test]
    fn centered_rect_places_a_half_size_popup_in_the_middle() {
        let area = Rect::new(0, 0, 80, 24);

        let popup = centered_rect(50, 50, area);

        assert_eq!(popup, Rect::new(20, 6, 40, 12));
        assert_eq!(popup.x + popup.width / 2, area.width / 2);
        assert_eq!(popup.y + popup.height / 2, area.height / 2);
    }

    #[test]
    fn full_percent_covers_the_whole_area() {
        let area = Rect::new(3, 5, 40, 10);

        assert_eq!(centered_rect(100, 100, area), area);
    }

    #[test]
    fn horizontal_and_vertical_percents_are_independent() {
        // 80x20 so that every percentage divides evenly.
        let area = Rect::new(0, 0, 80, 20);

        let wide = centered_rect(80, 20, area);
        assert_eq!(wide.width, 64, "80% of 80");
        assert_eq!(wide.height, 4, "20% of 20");

        let tall = centered_rect(20, 80, area);
        assert_eq!(tall.width, 16, "20% of 80");
        assert_eq!(tall.height, 16, "80% of 20");
    }

    #[test]
    fn popup_is_centered_inside_the_area() {
        let area = Rect::new(0, 0, 80, 24);

        for &(px, py) in &[(10u16, 10u16), (33, 66), (50, 50), (75, 25), (100, 100)] {
            let popup = centered_rect(px, py, area);

            let left = popup.x - area.x;
            let right = area.x + area.width - (popup.x + popup.width);
            let top = popup.y - area.y;
            let bottom = area.y + area.height - (popup.y + popup.height);

            assert!(
                left.abs_diff(right) <= 1,
                "asymmetric horizontal margins for {}%: {} vs {}",
                px,
                left,
                right
            );
            assert!(
                top.abs_diff(bottom) <= 1,
                "asymmetric vertical margins for {}%: {} vs {}",
                py,
                top,
                bottom
            );
        }
    }

    #[test]
    fn popup_never_escapes_the_area() {
        // Deliberately a small cross-section of shapes: each call runs the
        // layout solver, so a full sweep would take minutes.
        let widths = [1u16, 2, 3, 7, 40, 80];
        let heights = [1u16, 2, 3, 7, 24];
        let percents = [0u16, 25, 50, 100];

        for &width in &widths {
            for &height in &heights {
                let area = Rect::new(0, 0, width, height);
                for &px in &percents {
                    for &py in &percents {
                        let popup = centered_rect(px, py, area);
                        assert!(
                            popup.x >= area.x
                                && popup.y >= area.y
                                && popup.x + popup.width <= area.x + area.width
                                && popup.y + popup.height <= area.y + area.height,
                            "{}x{} popup {}x% {}x% escapes {:?}",
                            popup.width,
                            popup.height,
                            px,
                            py,
                            area
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn offset_areas_are_handled() {
        let area = Rect::new(10, 4, 80, 24);

        let popup = centered_rect(50, 50, area);

        assert_eq!(popup, Rect::new(30, 10, 40, 12));
    }

    #[test]
    fn zero_percent_yields_an_empty_rect() {
        let area = Rect::new(0, 0, 80, 24);

        let popup = centered_rect(0, 0, area);

        assert_eq!(popup.width, 0);
        assert_eq!(popup.height, 0);
    }

    #[test]
    fn tiny_areas_do_not_panic() {
        for width in 0..4u16 {
            for height in 0..4u16 {
                let area = Rect::new(0, 0, width, height);
                let _ = centered_rect(50, 50, area);
                let _ = centered_rect(100, 100, area);
                let _ = centered_rect(1, 1, area);
            }
        }
    }

    // --- screen layout ---------------------------------------------------

    use super::{row_in_window, screen_layout, scrolled_offset};
    use crate::utils::TUIMode;

    #[test]
    fn view_and_move_modes_have_no_side_table() {
        for mode in [TUIMode::View, TUIMode::Move] {
            let layout = screen_layout(Rect::new(0, 0, 100, 40), mode);
            assert_eq!(layout.side_table, None, "mode {mode:?}");
            assert_eq!(layout.canvas, Rect::new(0, 0, 100, 28));
            assert_eq!(layout.list, Rect::new(0, 28, 100, 12));
        }
    }

    #[test]
    fn resolution_mode_splits_the_canvas_and_the_table() {
        let layout = screen_layout(Rect::new(0, 0, 100, 40), TUIMode::Resolution);

        assert_eq!(layout.canvas, Rect::new(0, 0, 70, 28));
        assert_eq!(layout.side_table, Some(Rect::new(70, 0, 30, 28)));
        assert_eq!(layout.list, Rect::new(0, 28, 100, 12));
    }

    #[test]
    fn scale_mode_leaves_a_narrow_table_column() {
        let layout = screen_layout(Rect::new(0, 0, 100, 40), TUIMode::Scale);

        assert_eq!(layout.canvas, Rect::new(0, 0, 90, 28));
        assert_eq!(layout.side_table, Some(Rect::new(90, 0, 10, 28)));
    }

    #[test]
    fn the_regions_tile_the_area_without_gaps_or_overlap() {
        let area = Rect::new(0, 0, 120, 40);
        for mode in [
            TUIMode::View,
            TUIMode::Move,
            TUIMode::Resolution,
            TUIMode::Scale,
        ] {
            let layout = screen_layout(area, mode);
            assert_eq!(layout.canvas.y, area.y);
            assert_eq!(layout.canvas.height + layout.list.height, area.height);
            assert_eq!(layout.list.y, layout.canvas.y + layout.canvas.height);
            if let Some(side) = layout.side_table {
                assert_eq!(layout.canvas.width + side.width, area.width);
                assert_eq!(side.x, layout.canvas.x + layout.canvas.width);
                assert_eq!(side.height, layout.canvas.height);
            }
        }
    }

    #[test]
    fn offset_areas_shift_every_region() {
        let layout = screen_layout(Rect::new(10, 3, 100, 40), TUIMode::Resolution);

        assert_eq!(layout.area, Rect::new(10, 3, 100, 40));
        assert_eq!(layout.canvas, Rect::new(10, 3, 70, 28));
        assert_eq!(layout.list, Rect::new(10, 31, 100, 12));
    }

    #[test]
    fn screen_layout_survives_tiny_areas() {
        for width in 0..4u16 {
            for height in 0..4u16 {
                let area = Rect::new(0, 0, width, height);
                for mode in [
                    TUIMode::View,
                    TUIMode::Move,
                    TUIMode::Resolution,
                    TUIMode::Scale,
                ] {
                    let layout = screen_layout(area, mode);
                    assert!(
                        layout.canvas.x >= area.x
                            && layout.canvas.x + layout.canvas.width <= area.right()
                            && layout.list.y + layout.list.height <= area.bottom(),
                        "layout escapes {area:?} in {mode:?}"
                    );
                }
            }
        }
    }

    // --- scroll offset + row window --------------------------------------

    #[test]
    fn no_scroll_while_the_selection_fits() {
        for selected in 0..8 {
            assert_eq!(scrolled_offset(selected, 8), 0, "selected {selected}");
        }
    }

    #[test]
    fn selection_below_the_window_scrolls_it_into_view() {
        // Window of 5 rows: the sixth row (index 5) pulls the window down one.
        assert_eq!(scrolled_offset(5, 5), 1);
        assert_eq!(scrolled_offset(9, 5), 5);
        assert_eq!(scrolled_offset(100, 5), 96);
    }

    #[test]
    fn degenerate_windows_never_underflow() {
        assert_eq!(scrolled_offset(3, 0), 3);
        assert_eq!(scrolled_offset(0, 0), 0);
        assert_eq!(scrolled_offset(0, 1), 0);
        assert_eq!(scrolled_offset(7, 1), 7);
    }

    #[test]
    fn row_in_window_maps_clicks_to_row_indices() {
        // Rows 2..7 visible (offset 2, five-row window) starting at y = 10.
        assert_eq!(row_in_window(10, 5, 2, 20, 10), Some(2));
        assert_eq!(row_in_window(10, 5, 2, 20, 14), Some(6));
    }

    #[test]
    fn row_in_window_rejects_clicks_outside_the_window() {
        // Above the window.
        assert_eq!(row_in_window(10, 5, 0, 20, 9), None);
        // Inside the viewport but past the last row.
        assert_eq!(row_in_window(10, 5, 0, 3, 13), None);
        // Below the window.
        assert_eq!(row_in_window(10, 5, 0, 20, 15), None);
    }

    #[test]
    fn row_in_window_handles_empty_tables_and_windows() {
        assert_eq!(row_in_window(10, 5, 0, 0, 10), None);
        assert_eq!(row_in_window(10, 0, 0, 5, 10), None);
    }
}
