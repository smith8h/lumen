#!/usr/bin/env bash
# Pack gnome-extension/ into an extensions.gnome.org-ready zip in release/.
#   scripts/pack-extension.sh            → release/lumen@lumenlauncher.github.io.shell-extension.zip
#   scripts/pack-extension.sh --install  → also install it for the current user (log out and in to load it)
# Uses `gnome-extensions pack` when available (it validates metadata.json), otherwise plain `zip`.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SRC="$ROOT/gnome-extension"
OUT="$ROOT/release"
UUID=$(sed -n 's/.*"uuid": *"\([^"]*\)".*/\1/p' "$SRC/metadata.json")
ZIP="$OUT/$UUID.shell-extension.zip"
mkdir -p "$OUT"
rm -f "$ZIP"
cd "$SRC"
FILES=(metadata.json *.js)
[[ -f stylesheet.css ]] && FILES+=(stylesheet.css)
if command -v gnome-extensions >/dev/null; then
  EXTRA=()
  for f in *.js; do [[ $f != extension.js && $f != prefs.js ]] && EXTRA+=("--extra-source=$f"); done
  gnome-extensions pack --force --out-dir="$OUT" "${EXTRA[@]}" .
elif command -v zip >/dev/null; then
  zip -q -X "$ZIP" "${FILES[@]}"
else
  echo "Needs either gnome-extensions (GNOME Shell) or zip to pack the extension" >&2; exit 1
fi
echo "Packed ${FILES[*]} → $ZIP"
if [[ ${1:-} == --install ]]; then
  gnome-extensions install --force "$ZIP"
  echo "Installed $UUID. On Wayland GNOME loads new extensions at login: log out and back in, then run"
  echo "  gnome-extensions enable $UUID"
fi
