//! Registers Alt+Space with the desktop. Wayland doesn't let apps grab global keys, so on
//! GNOME Lumen adds a custom keyboard shortcut (the same thing Settings → Keyboard does) that
//! calls Lumen over D-Bus. GNOME's own Alt+Space (window menu) is moved out of the way and
//! restored if the shortcut is removed.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::constants::{APP_NAME, DBUS_IFACE, DBUS_NAME, DBUS_PATH, DEFAULT_SHORTCUT};
use crate::env::{desktop_name, is_gnome, launcher_path, run, which};
use crate::store::{read_json, write_json};

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const KEY_PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lumen/";
const CONFLICT_SCHEMAS: &[&str] = &["org.gnome.desktop.wm.keybindings", "org.gnome.shell.keybindings", "org.gnome.mutter.keybindings", MEDIA_KEYS];
const BACKUP: &str = "shortcut-backup.json";

fn relocatable() -> String {
    format!("{MEDIA_KEYS}.custom-keybinding:{KEY_PATH}")
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShortcutStatus {
    pub desktop: String,
    pub supported: bool,
    pub configured: bool,
    pub binding: String,
    pub command: String,
    pub conflicts: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Conflict {
    schema: String,
    key: String,
    values: Vec<String>,
}

/// A GVariant string literal.
pub fn gv_string(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

static STRV_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"'((?:[^'\\]|\\.)*)'").unwrap());
static UNESCAPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\(.)").unwrap());

/// Parse a GVariant string array as printed by gsettings.
pub fn parse_strv(v: &str) -> Vec<String> {
    STRV_ITEM.captures_iter(v).map(|c| UNESCAPE.replace_all(&c[1], "$1").into_owned()).collect()
}

pub fn strv(list: &[String]) -> String {
    if list.is_empty() { "@as []".into() } else { format!("[{}]", list.iter().map(|s| gv_string(s)).collect::<Vec<_>>().join(", ")) }
}

fn same(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect::<String>();
    norm(a) == norm(b)
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Shell command that toggles Lumen, starting it if it isn't running yet.
pub fn toggle_command(launcher: &str) -> String {
    format!(
        "gdbus call --session --dest {DBUS_NAME} --object-path {DBUS_PATH} --method {DBUS_IFACE}.Toggle >/dev/null 2>&1 || exec {} --toggle",
        sh_quote(launcher)
    )
}

/// GNOME parses the command with g_shell_parse_argv, so wrap it in double quotes.
fn gv_shell_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        if matches!(c, '"' | '\\' | '$' | '`') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// What the shortcut runs.
pub fn shortcut_command() -> String {
    format!("sh -c {}", gv_shell_quote(&toggle_command(&launcher_path())))
}

fn gsettings_ok() -> bool {
    is_gnome() && which("gsettings").is_some()
}

fn find_conflicts(binding: &str) -> Vec<Conflict> {
    static LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\S+\s+(\S+)\s+(.*)$").unwrap());
    let mut out = Vec::new();
    for schema in CONFLICT_SCHEMAS {
        let r = run("gsettings", &["list-recursively", schema]);
        if !r.ok {
            continue;
        }
        for line in r.stdout.lines() {
            let Some(c) = LINE.captures(line) else { continue };
            let (key, value) = (&c[1], &c[2]);
            if !value.contains('<') {
                continue;
            }
            let values =
                if value.starts_with('[') || value.starts_with("@as") { parse_strv(value) } else { vec![value.trim_matches('\'').to_string()] };
            if values.iter().any(|v| same(v, binding)) {
                out.push(Conflict { schema: schema.to_string(), key: key.to_string(), values });
            }
        }
    }
    out
}

pub fn status() -> ShortcutStatus {
    let desktop = desktop_name();
    let command = shortcut_command();
    if !gsettings_ok() {
        return ShortcutStatus { desktop, supported: false, configured: false, binding: DEFAULT_SHORTCUT.into(), command, conflicts: vec![] };
    }
    let list = parse_strv(&run("gsettings", &["get", MEDIA_KEYS, "custom-keybindings"]).stdout);
    let binding = run("gsettings", &["get", &relocatable(), "binding"]).stdout.trim_matches('\'').to_string();
    let configured = list.iter().any(|p| p == KEY_PATH) && !binding.is_empty();
    let effective = if binding.is_empty() { DEFAULT_SHORTCUT.to_string() } else { binding };
    let conflicts = find_conflicts(&effective)
        .into_iter()
        .filter(|c| !(c.schema == MEDIA_KEYS && c.key == "custom-keybindings"))
        .map(|c| format!("{} › {}", c.schema.rsplit('.').next().unwrap_or(&c.schema), c.key))
        .collect();
    ShortcutStatus { desktop, supported: true, configured, binding: effective, command, conflicts }
}

pub fn setup() -> ShortcutStatus {
    if !gsettings_ok() {
        return status();
    }
    let binding = DEFAULT_SHORTCUT;
    // Free the key from GNOME's own bindings, remembering what was there.
    let mut backup: Vec<Conflict> = read_json(BACKUP).unwrap_or_default();
    for c in find_conflicts(binding) {
        if c.schema == MEDIA_KEYS {
            continue;
        }
        if !backup.iter().any(|b| b.schema == c.schema && b.key == c.key) {
            backup.push(c.clone());
        }
        let kept: Vec<String> = c.values.iter().filter(|v| !same(v, binding)).cloned().collect();
        run("gsettings", &["set", &c.schema, &c.key, &strv(&kept)]);
    }
    let _ = write_json(BACKUP, &backup, None);

    let mut list = parse_strv(&run("gsettings", &["get", MEDIA_KEYS, "custom-keybindings"]).stdout);
    if !list.iter().any(|p| p == KEY_PATH) {
        list.push(KEY_PATH.into());
        run("gsettings", &["set", MEDIA_KEYS, "custom-keybindings", &strv(&list)]);
    }
    let rel = relocatable();
    run("gsettings", &["set", &rel, "name", &gv_string(APP_NAME)]);
    run("gsettings", &["set", &rel, "command", &gv_string(&shortcut_command())]);
    run("gsettings", &["set", &rel, "binding", &gv_string(binding)]);
    status()
}

pub fn remove() -> ShortcutStatus {
    if !gsettings_ok() {
        return status();
    }
    let list: Vec<String> =
        parse_strv(&run("gsettings", &["get", MEDIA_KEYS, "custom-keybindings"]).stdout).into_iter().filter(|p| p != KEY_PATH).collect();
    run("gsettings", &["set", MEDIA_KEYS, "custom-keybindings", &strv(&list)]);
    run("gsettings", &["reset-recursively", &relocatable()]);
    if let Some(backup) = read_json::<Vec<Conflict>>(BACKUP) {
        for b in backup {
            run("gsettings", &["set", &b.schema, &b.key, &strv(&b.values)]);
        }
        let _ = write_json::<Vec<Conflict>>(BACKUP, &vec![], None);
    }
    status()
}

/// If Lumen moved or was updated, point the existing shortcut at the new path.
pub fn refresh_command() {
    if !gsettings_ok() {
        return;
    }
    if !status().configured {
        return;
    }
    let want = gv_string(&shortcut_command());
    let current = run("gsettings", &["get", &relocatable(), "command"]).stdout;
    if current != want {
        run("gsettings", &["set", &relocatable(), "command", &want]);
    }
}

/// "<Alt>space" → "Alt+Space"
pub fn pretty_binding(b: &str) -> String {
    b.replace("<Alt>", "Alt+")
        .replace("<Super>", "Super+")
        .replace("<Control>", "Ctrl+")
        .replace("<Primary>", "Ctrl+")
        .replace("<Shift>", "Shift+")
        .replace("space", "Space")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strv_round_trips() {
        let list = vec!["<Alt>space".to_string(), "it's".to_string(), r"back\slash".to_string()];
        assert_eq!(parse_strv(&strv(&list)), list);
        assert_eq!(parse_strv("@as []"), Vec::<String>::new());
        assert_eq!(strv(&[]), "@as []");
    }

    #[test]
    fn commands_are_quoted_for_gnome() {
        let t = toggle_command("/opt/My Apps/lumen");
        assert!(t.ends_with("exec '/opt/My Apps/lumen' --toggle"));
        let sh = format!("sh -c {}", gv_shell_quote("echo \"$HOME\""));
        assert_eq!(sh, r#"sh -c "echo \"\$HOME\"""#);
        assert_eq!(pretty_binding("<Alt>space"), "Alt+Space");
    }
}
