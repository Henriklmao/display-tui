//! UI layout definitions.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

// Canvas area height percentage.
pub const CANVAS_HEIGHT_PERCENT: u16 = 70;

// List area height percentage.
pub const LIST_HEIGHT_PERCENT: u16 = 30;

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
}
