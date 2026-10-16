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
PKGVER="${TAG#v}"

URL="https://github.com/${REPO}"

# Download the release tarball to compute its checksum
TARBALL="/tmp/v${PKGVER}.tar.gz"
curl -fsSL -o "$TARBALL" "${URL}/archive/refs/tags/v${PKGVER}.tar.gz"
SHA256="$(sha256sum "$TARBALL" | cut -d' ' -f1)"

cat > PKGBUILD <<EOF
# Maintainer: Henriklmao
pkgname=display-tui
pkgver=${PKGVER}
pkgrel=1
pkgdesc="Minimalistic Hyprland monitor configuration TUI"
arch=('x86_64')
url="${URL}"
license=('GPL-3.0-or-later')
depends=('glibc' 'gcc-libs')
makedepends=('cargo')
options=('!lto')
source=("${PKGVER}.tar.gz::${URL}/archive/refs/tags/v${PKGVER}.tar.gz")
sha256sums=('${SHA256}')

prepare() {
  cd "display-tui-${PKGVER}"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked
}

build() {
  cd "display-tui-${PKGVER}"
  export RUSTUP_TOOLCHAIN=stable
  cargo build --frozen --release
}

package() {
  cd "display-tui-${PKGVER}"
  install -Dm755 target/release/display-tui "\${pkgdir}/usr/bin/display-tui"
  install -Dm644 assets/display-tui.desktop "\${pkgdir}/usr/share/applications/display-tui.desktop"
  install -Dm644 assets/desktop-icon.png "\${pkgdir}/usr/share/pixmaps/display-tui.png"
}
EOF

echo "PKGBUILD generated for display-tui ${PKGVER}"
