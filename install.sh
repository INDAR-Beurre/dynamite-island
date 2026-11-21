#!/usr/bin/env bash
set -euo pipefail

REPO_URL="${DYNAMITE_REPO:-https://github.com/INDAR-Beurre/dynamite-island.git}"
INSTALL_DIR="${DYNAMITE_DIR:-$HOME/.local/share/dynamite-island}"

echo "==> Dynamite Island installer"

missing=()
for cmd in git cargo cc pkg-config; do
    command -v "$cmd" >/dev/null 2>&1 || missing+=("$cmd")
done

if command -v pkg-config >/dev/null 2>&1; then
    pkg-config --exists xkbcommon || missing+=("libxkbcommon")
    pkg-config --exists alsa || missing+=("alsa-lib")
fi

if (( ${#missing[@]} > 0 )); then
    echo "Missing build dependencies: ${missing[*]}"
    echo "On Arch / Artix, install them with:"
    echo "  sudo pacman -S --needed git rust base-devel libxkbcommon alsa-lib pkgconf"
    exit 1
fi

if [[ -d "$INSTALL_DIR/.git" ]]; then
    echo "==> Updating existing checkout in $INSTALL_DIR"
    git -C "$INSTALL_DIR" pull --ff-only
else
    echo "==> Cloning $REPO_URL -> $INSTALL_DIR"
    mkdir -p "$(dirname "$INSTALL_DIR")"
    git clone --depth 1 "$REPO_URL" "$INSTALL_DIR"
fi

cd "$INSTALL_DIR"
echo "==> Building binaries"
./build.sh

echo "==> Installing MangoWM bindings, layer rules, and autostart"
./deploy/install.sh

echo ""
echo "Done! Start now with:"
echo "  $INSTALL_DIR/target/release/dynamite-rust &"
