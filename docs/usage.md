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

- **Buttons**: the keybind hints at the bottom of the display list, in the preset menu and in popups are buttons. They always show the key they stand for, glow on hover and run exactly that key when clicked. Range hints such as `0-9`, `k/j` and `H/J/K/L` stay plain text because they are not a single action.
- **Select**: click a display row, a resolution/scale row, or a monitor on the map.
- **Enable/disable**: clicking a display row selects it; click its checkbox again (once it is selected) to enable/disable it.
- **Value cells**: click the resolution or scale cell of a display to open the matching table with its current value preselected, click the rotation cell to rotate clockwise, or click the workspace cell to open a small `0-9` picker (`0` clears the assignment).
- **Move**: drag a monitor on the map; it snaps to nearby edges and centres and draws alignment guides (a centre-to-centre snap draws a guide spanning both monitors). An **Apply** button then appears in the centre of every moved monitor — click it, **right-click**, or press `Enter`/`Space` to persist the move. While a move is unconfirmed the footer collapses to movement hints plus **Apply** and **Cancel all**: keep arranging the move with the arrow keys or `h`/`j`/`k`/`l` (or fast-move by 10 px with `H`/`J`/`K`/`L` and shift+arrows; the footer hints are not buttons), apply every pending move with the button, `Enter`/`Space` or a right-click, and put the monitors back with **Cancel all** or `Esc`/`q`. Keyboard-nudged monitors become pending too, so Cancel reverts them in the same step.
- **Hover**: every clickable control glows under the pointer, and resting on it for one second shows a one-line tooltip. Hovering a monitor on the map temporarily selects it in the display list (and the side table) and marks it yellow; leaving the map restores the previously selected monitor, while a click makes the hovered monitor the persistent selection.
- **Double-click**: double-click a resolution, scale or preset row to apply it immediately.
- **Scroll**: use the wheel over a list to step through it (displays, resolutions, scales or presets). The wheel over the footer does nothing, so a stray scroll cannot move the selection.
- **Presets**: click a preset to preview it, double-click to apply it, use the footer buttons for Apply/Override/New/Delete/Rename/Close, and click outside the menu to close it. The create, rename and delete prompts have their own clickable Save/Cancel and Yes/No buttons.

When saving, the app will automatically validate your layout. The rules are: no overlapping displays, no detached displays, unique workspaces, and at least one display starting at `(0,0)`. A warning pop-up appears with the problems; you can hit `f` to force save anyway, or `Esc` to go back and fix the issues.

Validation also runs as soon as you apply a move, resolution, scale or rotation. Offending edges blink red and a red connector line points from the detached display to the middle of the edge it should attach to. When a single move can resolve a problem, `Space` (or the popup's **auto fix** button) applies it, one fix per press: the detached monitor is moved so the middle of its edge meets the middle of the anchor's edge, and a layout without a monitor at `(0,0)` is translated as a whole so the leftmost (two monitors) or centre-most (three or more) monitor starts at the origin.

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
