# Display TUI

A simple and minimalistic TUI to manage display settings for Hyprland.
Built with Rust and the [crossterm](https://github.com/crossterm-rs/crossterm) and [ratatui](https://ratatui.rs/) libraries, it provides a user-friendly interface to control your display configurations.
Started as a Fork of [otto-bus-dev/display-tui](https://github.com/otto-bus-dev/display-tui) with some extra features.
Thanks for your work, you are the biggest contributor to this project!
> [!NOTE]  
> Supports and will always support old hyprland 0.55+ lua style and hyprlang config style.

## Features

- enable/disable display
- set display position
- set display resolution
- set display scale
- set display rotation
- set workspace assignments (using keys 1-9 in the list view, 0 to clear)

## Preview

![Preview of Display TUI](/assets/preview.png)

## Requirements

- Hyprland
- Hyprctl
- Nerd Font
- Rust
- Cargo

## Installation

### AUR
>
> coming soon
>
### Manual

1. Clone the repository and build the project:

   ```bash
   git clone https://github.com/Henriklmao/display-tui.git
   cd display-tui
   cargo build --release
   sudo cp target/release/display-tui /usr/local/bin/ # or your preferred location
   ```

2. Configuration & Integration

   Display TUI automatically detects your Hyprland configuration format.

   **For Hyprland 0.55+ (Lua Config):**
   If `~/.config/hypr/hyprland.lua` exists, Display TUI will automatically detect your monitor config it, and writes to it. If it doesn't detect any monitor config it will write into `~/.config/hypr/lua/monitors.lua`), and automatically add the necessary `require("...")` statement to your `hyprland.lua` file. No manual setup is needed!

   **For older Hyprland versions (hyprlang .conf):**
   Display TUI will fall back to using `~/.config/display-tui/config.json` to know where to save the `.conf` file. The default path is `~/.config/hypr/monitors.conf`. You can create it manually like this:

   ```bash
   mkdir -p ~/.config/display-tui
   echo '{"monitors_config_path": "~/.config/hypr/monitors.conf"}' > ~/.config/display-tui/config.json
   ```

   Then, add this line to your `~/.config/hypr/hyprland.conf`:

   ```bash
   source = ~/.config/hypr/monitors.conf
   ```

3. Run the TUI and Save your configuration:

   ```bash
   display-tui
   ```
