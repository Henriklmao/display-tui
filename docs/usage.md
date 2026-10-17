# How to use Display TUI

## TUI Usage

```bash
display-tui
```

Everything is controlled using the keyboard, the interface provides direct hints at the bottom.
A detailed list of keybindings is available in the help modal (`K` in View/Default mode).
You can navigate through the list of displays using the arrow keys or `j`/`k`.

- `m`: Move mode (use arrow keys or `h`/`j`/`k`/`l` to arrange displays)
- `r`: Resolution mode
- `s`: Scale mode
- `o`: Cycle rotation clockwise
- `1-9`: Assign workspace to the selected display (`0` to clear)
- `e`/`d`: Enable/Disable display
- `K`: Open the keybindings help modal (View/Normal mode only)
- `w`: Save the configuration to your hyprland config
- `p`: Open the preset menu
- `q`: Quit the application (in View mode)

In Resolution and Scale modes, `Enter`/`Space` apply the highlighted value and `Esc`/`q` cancel back to View mode.

## Mouse

The interface is fully keyboard driven, but the mouse can drive the same actions:

- **Select**: click a display row, a resolution/scale row, or a monitor on the map.
- **Enable/disable**: click the checkbox at the start of a display row.
- **Move**: drag a monitor on the map; it snaps to nearby edges and centres and draws alignment guides while snapped.
- **Scroll**: use the wheel over a list to step through it (displays, resolutions, scales or presets).
- **Presets**: click a preset to preview it, and click outside the menu to close it.

When saving, the app will automatically validate your layout. If any displays are separated too far, have overlapping regions, or if there are duplicate workspace assignments, a warning pop-up will appear. You can hit `f` to force save anyway, or `Esc` to go back and fix the issues.

## Presets

Presets let you save, recall, and manage named monitor configurations. Press `p` in View mode to open the preset menu. From there you can:

- **Apply** a preset (`Enter`/`Space`)
- **Create** a new preset (`n`)
- **Override** a preset (`o`)
- **Rename** a preset (`r`)
- **Delete** a preset (`d`) — confirmation required
- (`1`–`9`) — jump directly to a preset by its position in the list

## CLI Usage

Display TUI supports command-line arguments for scripting, hotkeys and quick access:

```bash
# Show help
display-tui --help # or -h

# List all presets
display-tui -pl # or -p -l

# Load and apply a preset (exits after writing config)
display-tui -p my-setup

# Open the preset menu directly
display-tui -pm # or -p -m
```

When loading a preset via `-p <name>`, the same validation rules apply as in the TUI. If the preset doesn't match connected monitors, or has zero enabled monitors, or fails validation, the TUI opens with an appropriate warning or error popup so you can fix the configuration.

### Hyprland Config Examples

Here's a few examples of how to use Display TUI for Keybindings in your Hyprland config:

#### Interactively select a preset

In this case spawns a kitty terminal window with the preset menu open, allowing you to select a preset interactively from list.

```lua
  hl.bind("SUPER + CTRL + D", hl.dsp.exec_cmd("kitty display-tui -pm"))
```

#### Load a preset directly

This will load the preset named `my-setup` and apply.

```lua
  hl.bind("SUPER + CTRL + D", hl.dsp.exec_cmd("display-tui -p my-setup"))
```
