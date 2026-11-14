#!/usr/bin/env bash
# Build Lumen and install it for the current user (or system-wide with --prefix /usr/local and sudo).
#
#   scripts/install.sh                 # → ~/.local/bin/lumen, menu entry and icons
#   scripts/install.sh --uninstall
#   scripts/install.sh --prefix /usr/local
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PREFIX="$HOME/.local"
UNINSTALL=0
while [[ $# -gt 0 ]]; do
  case $1 in
    --prefix) PREFIX=$2; shift 2 ;;
    --uninstall) UNINSTALL=1; shift ;;
    *) echo "Unknown option $1" >&2; exit 1 ;;
  esac
done
ID=io.github.lumenlauncher.Lumen
SIZES=(16 24 32 48 64 128 256 512)

if [[ $UNINSTALL == 1 ]]; then
  "$PREFIX/bin/lumen" --quit 2>/dev/null || true
  rm -f "$PREFIX/bin/lumen" "$PREFIX/share/applications/$ID.desktop"
  for s in "${SIZES[@]}"; do rm -f "$PREFIX/share/icons/hicolor/${s}x${s}/apps/$ID.png"; done
  rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/lumen.desktop"
  echo "Lumen removed. Settings stay in ~/.config/Lumen; run 'lumen --remove-shortcut' first to undo Alt+Space."
  exit 0
fi

cargo build --release --manifest-path "$ROOT/Cargo.toml"
install -Dm755 "$ROOT/target/release/lumen" "$PREFIX/bin/lumen"
install -Dm644 "$ROOT/packaging/$ID.desktop" "$PREFIX/share/applications/$ID.desktop"
for s in "${SIZES[@]}"; do
  install -Dm644 "$ROOT/packaging/icons/${s}x${s}.png" "$PREFIX/share/icons/hicolor/${s}x${s}/apps/$ID.png"
done
command -v update-desktop-database >/dev/null && update-desktop-database -q "$PREFIX/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$PREFIX/share/icons/hicolor" 2>/dev/null || true
# Restart a running Lumen so the new build takes over.
lumen --quit >/dev/null 2>&1 || true
echo "Installed $PREFIX/bin/lumen. Start it from your app menu, or run: lumen"
