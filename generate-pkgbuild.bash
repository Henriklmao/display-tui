#!/usr/bin/env bash
# Generate a PKGBUILD for the AUR package "display-tui" (built from source).
# Usage: bash ./generate-pkgbuild.bash <tag>   (e.g. v0.9.6)
set -euo pipefail

REPO="Henriklmao/display-tui"
TAG="${1:-${GITHUB_REF_NAME:-}}"
if [[ -z "$TAG" ]]; then
  echo "usage: $0 <tag>" >&2
  exit 1
fi
# SRCVER keeps the real tag spelling, which the tarball URL and the extracted
# directory name (display-tui-0.9.6-3) must use.
SRCVER="${TAG#v}"

# Tags look like "v0.9.6" (a release) or "v0.9.6-3" (the third revision of 0.9.6).
# AUR versions are "<pkgver>-<pkgrel>", so map a trailing integer revision to
# pkgrel: v0.9.6-3 -> pkgver=0.9.6, pkgrel=3, which shows as "0.9.6-3".
# Any other suffix (e.g. v0.9.5-pre) cannot go into pkgver verbatim because that
# field must not contain hyphens/colons/slashes, so replace '-' with '.'.
PKGREL=1
if [[ "$SRCVER" =~ ^(.+)-([0-9]+)$ ]]; then
  PKGVER="${BASH_REMATCH[1]}"
  PKGREL="${BASH_REMATCH[2]}"
else
  PKGVER="${SRCVER//-/.}"
fi

URL="https://github.com/${REPO}"

# Download the release tarball to compute its checksum
TARBALL="/tmp/v${SRCVER}.tar.gz"
curl -fsSL -o "$TARBALL" "${URL}/archive/refs/tags/v${SRCVER}.tar.gz"
SHA256="$(sha256sum "$TARBALL" | cut -d' ' -f1)"

cat > PKGBUILD <<EOF
# Maintainer: Henriklmao
pkgname=display-tui
pkgver=${PKGVER}
pkgrel=${PKGREL}
pkgdesc="Minimalistic Hyprland monitor configuration TUI"
arch=('x86_64')
url="${URL}"
license=('GPL-3.0-or-later')
depends=('glibc' 'gcc-libs')
provides=('display-tui')
makedepends=('cargo')
options=('!lto')
source=("${SRCVER}.tar.gz::${URL}/archive/refs/tags/v${SRCVER}.tar.gz")
sha256sums=('${SHA256}')

prepare() {
  cd "display-tui-${SRCVER}"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked
}

build() {
  cd "display-tui-${SRCVER}"
  export RUSTUP_TOOLCHAIN=stable
  cargo build --frozen --release
}

package() {
  cd "display-tui-${SRCVER}"
  install -Dm755 target/release/display-tui "\${pkgdir}/usr/bin/display-tui"
  install -Dm644 assets/display-tui.desktop "\${pkgdir}/usr/share/applications/display-tui.desktop"
  install -Dm644 assets/desktop-icon.png "\${pkgdir}/usr/share/pixmaps/display-tui.png"
}
EOF

echo "PKGBUILD generated for display-tui ${PKGVER}"
