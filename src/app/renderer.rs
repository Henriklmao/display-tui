//! UI rendering for the display-tui application.

use super::state::{App, HoverTarget};
use crate::ui::components::{HelpModal, Map, MonitorList, Resolutions, Scale};
use crate::utils::TUIMode;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let is_previewing = self.show_preset_menu.is_some();

        let hover = self.hover;

        let mut monitor_list =
            MonitorList::new(&self.monitors, self.mode, Some(self.selected_monitor));

        // Every monitor with an unapplied move gets an Apply button; sort for a
        // deterministic draw order (the map does not care about the set order).
        let mut pending: Vec<usize> = self.move_origins.keys().copied().collect();
        pending.sort_unstable();

        let canvas = Map {
            selected: self.selected_monitor,
            monitors: &self.monitors,
            is_previewing,
            guides: self.drag.as_ref().map(|drag| drag.guides),
            canvas: self.drag.as_ref().map(|drag| drag.canvas),
            pending: &pending,
            highlights: self.highlights.as_ref(),
            highlight_phase: self.highlight_visible(),
            hover,
        };
        monitor_list.is_previewing = is_previewing;
        monitor_list.moved_pending = self.has_pending_move();
        monitor_list.hover = hover;

        let layout = self
            .layout
            .unwrap_or_else(|| crate::ui::layouts::screen_layout(area, self.mode));

        canvas.render(layout.canvas, buf);
        if let Some(side) = layout.side_table {
            match self.mode {
                TUIMode::Resolution => {
                    if let Some(selected) = self.monitors.get(self.selected_monitor) {
                        let mut resolutions =
                            Resolutions::new(selected, Some(self.selected_resolution));
                        resolutions.hover = hover;
                        resolutions.render(side, buf);
                    }
                }
                TUIMode::Scale => {
                    let mut scale = Scale::new(self.selected_scale);
                    scale.hover = hover;
                    scale.render(side, buf);
                }
                _ => {}
            }
        }
        monitor_list.render(layout.list, buf);

        if let Some(ref preset_menu) = self.show_preset_menu {
            preset_menu.render_with(area, buf, hover);
        }

        if self.show_help {
            HelpModal::render(
                area,
                buf,
                self.help_scroll,
                self.hover == Some(HoverTarget::HelpClose),
            );
        }

        if let Some(ref popup) = self.show_popup {
            popup.render_with(area, buf, hover);
        }

        // Topmost layer: the hover tooltip.
        if let Some(ref tooltip) = self.tooltip {
            crate::ui::tooltip::render(tooltip, area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{find_cell, find_text, test_monitors, text_of};
    use crate::ui::components::PresetMenu;
    use ratatui::style::Color;
    use ratatui::widgets::Widget;

    use super::super::state::Popup;

    fn app(mode: TUIMode) -> App {
        let mut app = App::new(None);
        app.monitors = test_monitors();
        app.mode = mode;
        app
    }

    fn render_app(app: &App, area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
        buf
    }

    fn error_popup(is_forceable: bool) -> Popup {
        Popup {
            title: "Write failed".to_string(),
            lines: vec!["hyprctl refused".to_string()],
            is_error: true,
            apply_preset: None,
            is_forceable,
            fix: None,
        }
    }

    #[test]
    fn view_mode_shows_map_and_display_list() {
        let text = text_of(&render_app(&app(TUIMode::View), Rect::new(0, 0, 120, 40)));

        assert!(text.contains("Map"), "no map: {text}");
        assert!(text.contains("Displays"), "no monitor list: {text}");
        assert!(text.contains("Monitor 1"), "monitor missing: {text}");
        assert!(!text.contains("Resolutions"), "wrong mode rendered: {text}");
    }

    #[test]
    fn resolution_mode_adds_the_resolution_table() {
        let text = text_of(&render_app(
            &app(TUIMode::Resolution),
            Rect::new(0, 0, 120, 40),
        ));

        assert!(text.contains("Map"), "map must stay visible: {text}");
        assert!(text.contains("Resolutions"), "table missing: {text}");
        assert!(text.contains("1920x1080"), "modes missing: {text}");
    }

    #[test]
    fn scale_mode_adds_the_scale_table() {
        let text = text_of(&render_app(&app(TUIMode::Scale), Rect::new(0, 0, 120, 40)));

        assert!(text.contains("Map"), "map must stay visible: {text}");
        assert!(text.contains("Scale"), "scale table missing: {text}");
        assert!(text.contains("100%"), "scale steps missing: {text}");
    }

    #[test]
    fn help_overlay_is_drawn_on_top_of_the_normal_view() {
        let mut a = app(TUIMode::View);
        a.show_help = true;

        let text = text_of(&render_app(&a, Rect::new(0, 0, 120, 40)));

        assert!(text.contains("Keybindings/Help"), "help missing: {text}");
        assert!(text.contains("Map"), "background must stay: {text}");
    }

    #[test]
    fn preset_menu_overlay_is_drawn_when_open() {
        let mut a = app(TUIMode::View);
        a.show_preset_menu = Some(PresetMenu::new(vec![], &[], None));

        let text = text_of(&render_app(&a, Rect::new(0, 0, 120, 40)));

        assert!(text.contains("Presets"), "menu missing: {text}");
        assert!(
            text.contains("No presets found"),
            "placeholder missing: {text}"
        );
    }

    #[test]
    fn forceable_error_popup_advertises_the_force_key() {
        let mut a = app(TUIMode::View);
        a.show_popup = Some(error_popup(true));

        let text = text_of(&render_app(&a, Rect::new(0, 0, 120, 40)));

        assert!(text.contains("hyprctl refused"), "message missing: {text}");
        assert!(
            text.contains("[ f force write ]"),
            "force button missing: {text}"
        );
    }

    #[test]
    fn error_popup_without_force_only_offers_to_close() {
        let mut a = app(TUIMode::View);
        a.show_popup = Some(error_popup(false));

        let text = text_of(&render_app(&a, Rect::new(0, 0, 120, 40)));

        assert!(
            text.contains("[ Esc close ]"),
            "close button missing: {text}"
        );
        assert!(
            !text.contains("force write"),
            "force must not be offered: {text}"
        );
    }

    #[test]
    fn popup_border_colour_follows_the_error_flag() {
        let area = Rect::new(0, 0, 120, 40);

        let mut error = app(TUIMode::View);
        let error_popup = error_popup(false);
        let error_frame = error_popup.layout(area).frame;
        error.show_popup = Some(error_popup);
        let buf = render_app(&error, area);
        assert_eq!(
            buf[(error_frame.x, error_frame.y)].style().fg,
            Some(Color::Red)
        );

        let mut info = app(TUIMode::View);
        let info_popup = Popup {
            title: "Saved".to_string(),
            lines: vec!["Preset written".to_string()],
            is_error: false,
            apply_preset: None,
            is_forceable: false,
            fix: None,
        };
        let info_frame = info_popup.layout(area).frame;
        info.show_popup = Some(info_popup);
        let buf = render_app(&info, area);
        assert_eq!(
            buf[(info_frame.x, info_frame.y)].style().fg,
            Some(Color::Yellow)
        );
    }

    #[test]
    fn every_mode_survives_tiny_terminals() {
        for (width, height) in [(1u16, 1u16), (5, 3), (10, 5), (20, 8), (40, 12)] {
            let area = Rect::new(0, 0, width, height);
            for mode in [
                TUIMode::View,
                TUIMode::Move,
                TUIMode::Resolution,
                TUIMode::Scale,
            ] {
                let mut a = app(mode);
                a.show_help = true;
                render_app(&a, area);
            }
        }
    }

    #[test]
    fn every_mode_renders_with_zero_monitors() {
        for mode in [
            TUIMode::View,
            TUIMode::Move,
            TUIMode::Resolution,
            TUIMode::Scale,
        ] {
            let mut a = App::new(None);
            a.mode = mode;
            // Must not panic even though the tables index a selected monitor.
            let _ = render_app(&a, Rect::new(0, 0, 120, 40));
        }
    }

    // The Resolution/Scale side table must show — and act on — the hovered
    // monitor, matching the display list and map.
    #[test]
    fn the_resolution_table_follows_the_hovered_monitor() {
        use crate::app::state::HoverTarget;
        use crate::monitor::Resolution;
        use std::time::Instant;

        let mut a = App::new(None);
        a.mode = TUIMode::Resolution;
        let mut first = powered_monitor("Alpha", 0);
        first.modes.truncate(1); // only 1920x1080
        let mut second = powered_monitor("Beta", 1920);
        second.modes.push(Resolution {
            width: 2560,
            height: 1440,
            refresh: 60.0,
            preferred: false,
            current: false,
        });
        a.monitors = vec![first, second];
        a.selected_monitor = 0;

        let area = Rect::new(0, 0, 200, 40);
        let before = text_of(&render_app(&a, area));
        assert!(
            !before.contains("2560x1440"),
            "the stored selection's table has no such mode"
        );

        a.set_hover(Some(HoverTarget::Map(1)), (10, 10), Instant::now(), true);
        let after = text_of(&render_app(&a, area));
        assert!(
            after.contains("2560x1440"),
            "the table shows the hovered monitor's modes: {after}"
        );
    }

    #[test]
    fn hovering_a_monitor_selects_it_visually_without_persisting() {
        use crate::app::state::HoverTarget;
        use std::time::Instant;

        let mut a = App::new(None);
        a.monitors = vec![powered_monitor("Alpha", 0), powered_monitor("Beta", 1920)];
        a.selected_monitor = 0;
        a.set_hover(Some(HoverTarget::Map(1)), (10, 10), Instant::now(), true);
        let area = Rect::new(0, 0, 120, 40);

        let buf = render_app(&a, area);
        let layout = crate::ui::layouts::screen_layout(area, TUIMode::View);
        let (_, marker_row) = find_cell(&buf, "\u{f04b}").expect("selection marker");
        assert_eq!(
            marker_row,
            layout.list.y + 4,
            "the list marker follows the hovered (second) monitor"
        );

        let (bx, by) = find_text(&buf, "Beta").expect("Beta map label");
        assert_eq!(
            buf[(bx, by)].style().fg,
            Some(Color::Yellow),
            "the hovered monitor is marked yellow on the map"
        );
        let (ax, ay) = find_text(&buf, "Alpha").expect("Alpha map label");
        assert_eq!(
            buf[(ax, ay)].style().fg,
            Some(Color::Blue),
            "the previously selected monitor loses the yellow highlight"
        );
        assert_eq!(
            a.selected_monitor, 1,
            "the hover temporarily selects the monitor"
        );

        // Leaving the map reverts to the stored selection.
        a.set_hover(None, (10, 10), Instant::now(), false);
        assert_eq!(
            a.selected_monitor, 0,
            "hover must not persist the selection"
        );
        let buf = render_app(&a, area);
        let (_, marker_row) = find_cell(&buf, "\u{f04b}").expect("selection marker");
        assert_eq!(
            marker_row,
            layout.list.y + 3,
            "the list reverts to the stored selection on hover-out"
        );
    }

    // An enabled monitor of 1920x1080 at (x, 0).
    fn powered_monitor(name: &str, x: i32) -> crate::monitor::Monitor {
        let mut monitor = test_monitors()[0].clone();
        monitor.name = name.to_string();
        monitor.position = Some(crate::monitor::Position { x, y: 0 });
        monitor
    }

    #[test]
    fn the_tooltip_is_drawn_on_top_of_everything() {
        let mut a = app(TUIMode::View);
        a.tooltip = Some(crate::ui::tooltip::TooltipState {
            text: "Click to save your changes",
            step: 3,
            anchor: (10, 10),
        });

        let text = text_of(&render_app(&a, Rect::new(0, 0, 120, 40)));

        assert!(
            text.contains("Click to save your changes"),
            "tooltip missing: {text}"
        );
    }

    #[test]
    fn every_overlay_survives_the_size_matrix() {
        for (width, height) in [
            (0u16, 0u16),
            (1, 1),
            (6, 3),
            (20, 8),
            (40, 12),
            (80, 24),
            (120, 40),
            (240, 80),
        ] {
            let area = Rect::new(0, 0, width, height);
            for mode in [
                TUIMode::View,
                TUIMode::Move,
                TUIMode::Resolution,
                TUIMode::Scale,
            ] {
                let mut a = app(mode);
                a.show_help = true;
                render_app(&a, area);

                a.show_help = false;
                a.show_preset_menu = Some(PresetMenu::new(vec![], &[], None));
                render_app(&a, area);

                let mut a = app(mode);
                a.show_popup = Some(error_popup(true));
                render_app(&a, area);
            }
        }
    }
}
