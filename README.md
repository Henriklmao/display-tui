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
- set workspace assignments
- save validation with detailed warning (overridable)
- quick-access global help modal (press `shift + K`)
- responsive mini-mode for small terminal windows

## Preview

![Preview of Display TUI](/assets/preview.png)

## Requirements

- Hyprland
- Nerd Font (Not automatically installed)
- Rust
- Cargo
- xdg-terminal-exec (If you want to launch from the .desktop shortcut)

## Usage

Run the TUI:

   ```bash
   display-tui
   ```

Everything is controlled using the keyboard, and the interface provides direct hints at the bottom.
You can navigate through the list of displays using the arrow keys or `j`/`k`.

- `m`: Move mode (use arrow keys or `h`/`j`/`k`/`l` to arrange displays)
- `r`: Resolution mode
- `s`: Scale mode
- `o`: Cycle rotation
- `1-9`: Assign workspace to the selected display (`0` to clear)
- `e`/`d`: Enable/Disable display
- `K`: Open the global keybindings help modal
- `w`: Save the configuration to your hyprland config
- `q`: Quit the application

When saving, the app will automatically validate your layout. If any displays are separated too far or if there are duplicate workspace assignments, a warning modal will appear. You can hit `f` to force save anyway, or `Esc` to go back and fix the issues.

## Installation

### AUR

```bash
yay -S display-tui-git
# or
paru -S display-tui-git
```

AUR installation is fully automatic and adds `display-tui` as a .desktop shortcut, so you can launch it from your application launcher.

### AppImage (Universal / All Distributions)

You can download the latest AppImage from the [Releases](https://github.com/Henriklmao/display-tui/releases) page. The AppImage is built automatically and includes everything needed to run `display-tui` on any Linux distribution.

1. Download the `.AppImage` file from the latest release.
2. Make it executable:

   ```bash
   chmod +x display-tui-*.AppImage
   ```

3. Run it:

   ```bash
   ./display-tui-*.AppImage
   ```

4. Move it to your PATH, e.g., `sudo mv display-tui-*.AppImage /usr/local/bin/display-tui` for easier access)

### Manual

1. Clone the repository and build the project:

   ```bash
   git clone https://github.com/Henriklmao/display-tui.git
   cd display-tui
   cargo build --release
   sudo cp target/release/display-tui /usr/local/bin/ # or your preferred location
   ```

## Configuration & Integration

   Display TUI automatically detects your Hyprland configuration format.

### Hyprland lua 0.55+

   If `~/.config/hypr/hyprland.lua` exists, Display TUI will automatically detect your monitor config it, and writes to it. If it doesn't detect any monitor config it will write into `~/.config/hypr/lua/monitors.lua`), and automatically add the necessary `require("...")` statement to your `hyprland.lua` file. No manual setup is needed!

### Hyprlang (old .conf style)

   Display TUI will fall back to using `~/.config/display-tui/config.json` to know where to save the `.conf` file. The default path is `~/.config/hypr/monitors.conf`. You can create it manually like this:

   ```bash
   mkdir -p ~/.config/display-tui
   echo '{"monitors_config_path": "~/.config/hypr/monitors.conf"}' > ~/.config/display-tui/config.json
   ```

## Contributions

Contributions are always welcome! If you have any ideas for new features, improvements, or bug fixes, feel free to open an issue or submit a pull request.

### Contributor list

[otto-bus-dev](https://github.com/Henriklmao/display-tui/commits/master/?author=otto-bus-dev)

- The idea and most of the implementation comes from him.
- Thank you for starting this project.

[Dan-Kingsley](https://github.com/Henriklmao/display-tui/commits/master/?author=Dan-Kingsley)

- Display rotation
- Arrow key support for navigation
- Snapping and fine control in move mode
