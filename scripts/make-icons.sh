#!/usr/bin/env bash
# Re-render packaging/icons/*.png from resources/icon.svg (needs rsvg-convert: librsvg2-bin).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
for s in 16 24 32 48 64 128 256 512; do
  rsvg-convert -w "$s" -h "$s" "$ROOT/resources/icon.svg" -o "$ROOT/packaging/icons/${s}x${s}.png"
done
cp "$ROOT/packaging/icons/512x512.png" "$ROOT/resources/icon.png"
echo "Rendered packaging/icons"
