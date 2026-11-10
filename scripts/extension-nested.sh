#!/usr/bin/env bash
# Try the GNOME extension without logging out: starts a nested GNOME Shell with only the
# Lumen extension enabled, then runs Lumen (hot-reloading dev build) inside it.
#
#   npm run extension:nested              # nested shell in a window (needs mutter-devkit)
#   npm run extension:nested -- --headless   # no window; drive it over D-Bus (CI / scripts)
#
# Everything is isolated: its own D-Bus session, Wayland display, GSettings (a keyfile in a
# temp folder) and Lumen profile, so your real session, settings and shortcut are untouched.
# Restart it to pick up changes to the extension; GNOME Shell can't reload extension code.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
UUID=$(sed -n 's/.*"uuid": *"\([^"]*\)".*/\1/p' "$ROOT/gnome-extension/metadata.json")
DISPLAY_NAME=lumen-nested-0
TMP=$(mktemp -d -t lumen-nested-XXXXXX)
trap 'rm -rf "$TMP"' EXIT

MODE=(--devkit)
if [[ ${1:-} == --headless ]]; then
  MODE=(--headless --virtual-monitor 1920x1080)
elif [[ ! -x /usr/libexec/mutter-devkit ]]; then
  echo "mutter-devkit isn't installed (Ubuntu/Debian: sudo apt install mutter-dev-bin; Fedora: mutter-devel)." >&2
  echo "Run with --headless to start the shell without a window instead." >&2
  exit 1
fi

mkdir -p "$TMP/data/gnome-shell/extensions" "$TMP/config" "$TMP/lumen"
ln -s "$ROOT/gnome-extension" "$TMP/data/gnome-shell/extensions/$UUID"
echo '{"firstRunDone": true}' >"$TMP/lumen/settings.json"

export LUMEN_NESTED_TMP=$TMP LUMEN_NESTED_UUID=$UUID LUMEN_NESTED_DISPLAY=$DISPLAY_NAME LUMEN_ROOT=$ROOT
export LUMEN_REAL_DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share} LUMEN_REAL_CONFIG_HOME=${XDG_CONFIG_HOME:-$HOME/.config}

dbus-run-session -- bash -c '
  set -euo pipefail
  shell_env=(XDG_DATA_HOME="$LUMEN_NESTED_TMP/data" XDG_CONFIG_HOME="$LUMEN_NESTED_TMP/config" GSETTINGS_BACKEND=keyfile)
  env "${shell_env[@]}" gsettings set org.gnome.shell enabled-extensions "[\"$LUMEN_NESTED_UUID\"]"
  env "${shell_env[@]}" gsettings set org.gnome.shell disable-user-extensions false
  env "${shell_env[@]}" gnome-shell --wayland-display "$LUMEN_NESTED_DISPLAY" --no-x11 "$@" &
  shell=$!
  for _ in $(seq 100); do [[ -S "$XDG_RUNTIME_DIR/$LUMEN_NESTED_DISPLAY" ]] && break; sleep 0.1; done

  echo
  echo "Nested GNOME Shell is running with $LUMEN_NESTED_UUID."
  echo "Toggle Lumen inside it with:"
  echo "  DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS gdbus call --session --dest io.github.lumenlauncher.Lumen --object-path /io/github/lumenlauncher/Lumen --method io.github.lumenlauncher.Lumen.Toggle"
  echo

  cd "$LUMEN_ROOT"
  env -u DISPLAY WAYLAND_DISPLAY="$LUMEN_NESTED_DISPLAY" XDG_SESSION_TYPE=wayland \
    XDG_DATA_HOME="$LUMEN_REAL_DATA_HOME" XDG_CONFIG_HOME="$LUMEN_REAL_CONFIG_HOME" \
    LUMEN_USER_DATA="$LUMEN_NESTED_TMP/lumen" npx electron-vite dev &
  lumen=$!
  wait "$shell" || true
  kill "$lumen" 2>/dev/null || true
' bash "${MODE[@]}"
