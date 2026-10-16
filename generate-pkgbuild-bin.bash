#!/usr/bin/env bash
# Generate a PKGBUILD for the AUR package "display-tui-bin" (pre-built binary
# downloaded from the GitHub Release produced by rust.yml).
# Usage: bash ./generate-pkgbuild-bin.bash <tag>   (e.g. v0.9.6)
set -euo pipefail

REPO="Henriklmao/display-tui"
TAG="${1:-${GITHUB_REF_NAME:-}}"
if [[ -z "$TAG" ]]; then
  echo "usage: $0 <tag>" >&2
  exit 1
fi
PKGVER="${TAG#v}"

URL="https://github.com/${REPO}"
BIN_NAME="display-tui-${TAG}-x86_64-unknown-linux-gnu"
BIN_URL="${URL}/releases/download/${TAG}/${BIN_NAME}"

# rust.yml builds the release binary in parallel with this workflow, so the
# asset may not exist yet: poll the download URL until it becomes available.
BINARY="/tmp/${BIN_NAME}"
echo "Waiting for ${BIN_URL} ..."
for attempt in $(seq 1 60); do
  if curl -fsSL -o "$BINARY" "$BIN_URL"; then
    echo "Downloaded ${BIN_NAME}"
    break
  fi
  if [[ "$attempt" -eq 60 ]]; then
    echo "error: release binary did not appear after 30 minutes" >&2
    exit 1
  fi
  sleep 30
done
SHA256="$(sha256sum "$BINARY" | cut -d' ' -f1)"

# Desktop entry and icon, fetched at the matching tag
DESKTOP="/tmp/display-tui.desktop"
ICON="/tmp/desktop-icon.png"
curl -fsSL -o "$DESKTOP" "https://raw.githubusercontent.com/${REPO}/${TAG}/assets/display-tui.desktop"
curl -fsSL -o "$ICON"    "https://raw.githubusercontent.com/${REPO}/${TAG}/assets/desktop-icon.png"
DESKTOP_SHA256="$(sha256sum "$DESKTOP" | cut -d' ' -f1)"
ICON_SHA256="$(sha256sum "$ICON" | cut -d' ' -f1)"

cat > PKGBUILD <<EOF
# Maintainer: Henriklmao
pkgname=display-tui-bin
pkgver=${PKGVER}
pkgrel=1
pkgdesc="Minimalistic Hyprland monitor configuration TUI (pre-built binary)"
arch=('x86_64')
url="${URL}"
license=('GPL-3.0-or-later')
depends=('glibc' 'gcc-libs')
provides=('display-tui')
conflicts=('display-tui')
options=('!strip' '!lto')
source=(
  "${BIN_NAME}::${BIN_URL}"
  "https://raw.githubusercontent.com/${REPO}/${TAG}/assets/display-tui.desktop"
  "https://raw.githubusercontent.com/${REPO}/${TAG}/assets/desktop-icon.png"
)
sha256sums=(
  '${SHA256}'
  '${DESKTOP_SHA256}'
  '${ICON_SHA256}'
)

package() {
  install -Dm755 "${BIN_NAME}" "\${pkgdir}/usr/bin/display-tui"
  install -Dm644 display-tui.desktop "\${pkgdir}/usr/share/applications/display-tui.desktop"
  install -Dm644 desktop-icon.png "\${pkgdir}/usr/share/pixmaps/display-tui.png"
}
EOF

echo "PKGBUILD generated for display-tui-bin ${PKGVER}"
