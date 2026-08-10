# How to fix .desktop shortcut when installing from AUR

As the AUR is currently closed for pushes I can't update the package to use the correct .desktop file. If you installed `display-tui` from the AUR, please follow the steps below to fix the .desktop shortcut.

> The AUR version still relies on xdg-terminal-exec for the .desktop file to function. This package is now outdated, therefore I [dropped it's support.](commit/4cdb90fd745f12804db09c5c9db90db2ab106a5d) \
> xdg-terminal-exec was an optional dependency, so you luckily don't have to install an outdated package.

## Quickfix

```bash
git clone https://github.com/Henriklmao/display-tui.git
cd display-tui
sudo install -Dm 644 "assets/display-tui.desktop" "/usr/share/applications/display-tui.desktop"

```
