//! UI rendering for the display-tui application.

use super::state::App;
use crate::ui::components::{HelpModal, Map, MonitorList, Resolutions, Scale};
use crate::utils::TUIMode;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let is_previewing = self.show_preset_menu.is_some();

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
        };
        monitor_list.is_previewing = is_previewing;
        monitor_list.moved_pending = self.has_pending_move();

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
                        resolutions.render(side, buf);
                    }
                }
                TUIMode::Scale => {
                    let mut scale = Scale::new(self.selected_scale);
                    scale.render(side, buf);
                }
                _ => {}
            }
        }
        monitor_list.render(layout.list, buf);

        if let Some(ref preset_menu) = self.show_preset_menu {
            preset_menu.render(area, buf);
        }

        if self.show_help {
            HelpModal::render(area, buf);
        }

        if let Some(ref popup) = self.show_popup {
            popup.render(area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{test_monitors, text_of};
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

        assert!(text.contains("[ Esc close ]"), "close button missing: {text}");
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
        assert_eq!(buf[(error_frame.x, error_frame.y)].style().fg, Some(Color::Red));

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
