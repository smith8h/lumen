#!/bin/sh
# Lumen launcher. Installed as `lumen` next to the real Electron binary (`lumen-bin`).
#  - `lumen --toggle` talks to a running Lumen over D-Bus (instant), or starts it.
#  - Turns Chromium's sandbox off only where it can't work: AppImages on distros that
#    restrict unprivileged user namespaces (Ubuntu 24.04+). The .deb ships an AppArmor
#    profile instead, so the sandbox stays on there.
HERE="$(dirname "$(readlink -f "$0")")"
NAME=io.github.lumenlauncher.Lumen
OBJ=/io/github/lumenlauncher/Lumen

if [ "$1" = "--toggle" ] && command -v gdbus >/dev/null 2>&1; then
  gdbus call --session --dest "$NAME" --object-path "$OBJ" --method "$NAME.Toggle" >/dev/null 2>&1 && exit 0
fi

export LUMEN_LAUNCHER="${APPIMAGE:-$(readlink -f "$0")}"

case " $* " in *" --no-sandbox "*) exec "$HERE/lumen-bin" "$@" ;; esac

if [ -n "$APPIMAGE" ]; then
  if [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null)" = "1" ] || \
     [ "$(cat /proc/sys/kernel/unprivileged_userns_clone 2>/dev/null)" = "0" ]; then
    exec "$HERE/lumen-bin" --no-sandbox "$@"
  fi
elif [ ! -u "$HERE/chrome-sandbox" ] && [ ! -e /etc/apparmor.d/lumen ] && \
     [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null)" = "1" ]; then
  exec "$HERE/lumen-bin" --no-sandbox "$@"
fi
exec "$HERE/lumen-bin" "$@"
