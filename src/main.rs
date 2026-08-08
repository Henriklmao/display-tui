//! Display TUI - Monitor configuration for Hyprland.
//!
//! A terminal user interface for managing monitor configurations
//! in the Hyprland compositor.

use display_tui::app::App;
use std::io;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::default().run(&mut terminal);
    ratatui::restore();
    app_result
}
