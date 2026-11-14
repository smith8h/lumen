//! Power, session and quick-setting commands, plus Lumen's own commands.

use crate::constants::APP_NAME;
use crate::env::{is_gnome, run, spawn_detached, which};
use crate::fuzzy::match_fields;
use crate::store::History;
use crate::types::*;

#[derive(Clone, Copy)]
enum Op {
    Sys(SystemOp),
    Lumen(LumenOp),
    ClipClear,
}

struct SysCommand {
    id: &'static str,
    title: String,
    subtitle: String,
    keywords: &'static str,
    glyph: Glyph,
    op: Op,
    confirm: Option<&'static str>,
}

fn commands() -> Vec<SysCommand> {
    use Glyph as G;
    use LumenOp as L;
    use SystemOp as S;
    let c = |id, title: &str, subtitle: &str, keywords, glyph, op, confirm| SysCommand {
        id,
        title: title.to_string(),
        subtitle: subtitle.to_string(),
        keywords,
        glyph,
        op,
        confirm,
    };
    vec![
        c("lock", "Lock Screen", "System", "lock away", G::Lock, Op::Sys(S::Lock), None),
        c("suspend", "Sleep", "Suspend the computer", "suspend sleep", G::Moon, Op::Sys(S::Suspend), None),
        c("logout", "Log Out", "End this session", "logout log out sign out", G::Logout, Op::Sys(S::Logout), Some("Press Enter again to log out")),
        c("restart", "Restart", "Reboot the computer", "restart reboot", G::Restart, Op::Sys(S::Restart), Some("Press Enter again to restart")),
        c(
            "shutdown",
            "Shut Down",
            "Power off the computer",
            "shutdown shut down power off poweroff",
            G::Power,
            Op::Sys(S::Shutdown),
            Some("Press Enter again to shut down"),
        ),
        c("dark", "Dark Mode", "Switch the desktop to dark style", "dark mode theme night appearance", G::Moon, Op::Sys(S::DarkMode), None),
        c("light", "Light Mode", "Switch the desktop to light style", "light mode theme day appearance", G::Sun, Op::Sys(S::LightMode), None),
        c("wifi", "Toggle Wi-Fi", "Turn wireless on or off", "wifi wi-fi wireless network internet", G::Wifi, Op::Sys(S::WifiToggle), None),
        c(
            "dnd",
            "Do Not Disturb",
            "Toggle notification banners",
            "dnd do not disturb notifications focus silence",
            G::Bell,
            Op::Sys(S::DndToggle),
            None,
        ),
        c(
            "trash",
            "Empty Trash",
            "Permanently delete files in the Trash",
            "empty trash bin delete",
            G::Trash,
            Op::Sys(S::EmptyTrash),
            Some("Press Enter again to permanently delete everything in the Trash"),
        ),
        c("settings", "System Settings", "Open desktop settings", "settings control center preferences", G::Settings, Op::Sys(S::Settings), None),
        c(
            "l-settings",
            &format!("{APP_NAME} Settings"),
            "Theme, search engine, files, shortcut",
            "lumen preferences config settings options",
            G::Settings,
            Op::Lumen(L::Settings),
            None,
        ),
        c(
            "l-shortcut",
            &format!("Set Up {APP_NAME} Shortcut"),
            "Open Lumen with Alt+Space",
            "lumen shortcut hotkey keybinding alt space",
            G::Keyboard,
            Op::Lumen(L::Shortcut),
            None,
        ),
        c(
            "l-reindex",
            "Rebuild File Index",
            &format!("Rescan your files for {APP_NAME}"),
            "lumen reindex refresh files index rescan",
            G::Refresh,
            Op::Lumen(L::Reindex),
            None,
        ),
        c(
            "l-clip-clear",
            "Clear Clipboard History",
            "Forget copied text (pinned clips stay)",
            "clipboard history clear forget wipe copied clips",
            G::Clipboard,
            Op::ClipClear,
            Some("Press Enter again to clear clipboard history"),
        ),
        c("l-quit", &format!("Quit {APP_NAME}"), "Stop running in the background", "lumen quit exit close", G::Quit, Op::Lumen(L::Quit), None),
    ]
}

fn to_item(c: &SysCommand, score: f64, matches: Vec<usize>) -> ResultItem {
    let (kind, action, key) = match c.op {
        Op::ClipClear => (ResultKind::Lumen, Action::ClipClear, None),
        Op::Lumen(op) => (ResultKind::Lumen, Action::Lumen(op), Some(format!("sys:{}", c.id))),
        Op::Sys(op) => (ResultKind::System, Action::System(op), Some(format!("sys:{}", c.id))),
    };
    let mut item = ResultItem::new(format!("sys:{}", c.id), kind, c.title.clone(), action);
    item.subtitle = Some(c.subtitle.clone());
    item.icon = Some(IconSpec::Glyph(c.glyph));
    item.score = score;
    item.matches = matches;
    item.confirm = c.confirm.map(String::from);
    item.history_key = key;
    item
}

pub fn system_results(query: &str, history: &History, now: i64) -> Vec<ResultItem> {
    let mut out: Vec<ResultItem> = commands()
        .iter()
        .filter_map(|c| {
            let m = match_fields(query, &c.title, &[c.keywords], 0.85)?;
            (m.score >= 520.0).then(|| to_item(c, m.score * 0.92 + history.frecency(&format!("sys:{}", c.id), now), m.indexes))
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out.truncate(4);
    out
}

fn gsettings_get(schema: &str, key: &str) -> String {
    run("gsettings", &["get", schema, key]).stdout
}

fn gsettings_set(schema: &str, key: &str, value: &str) -> bool {
    run("gsettings", &["set", schema, key, value]).ok
}

fn detach(cmd: &str, args: &[&str]) -> Result<(), String> {
    spawn_detached(cmd, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(), None).map_err(|e| format!("Couldn't run {cmd}: {e}"))
}

/// Run a system command. Returns a short confirmation to show, if any.
pub fn run_system(op: SystemOp) -> Result<Option<String>, String> {
    let gnome = is_gnome() && which("gnome-session-quit").is_some();
    match op {
        SystemOp::Lock => {
            if !run("loginctl", &["lock-session"]).ok {
                detach("xdg-screensaver", &["lock"])?;
            }
            Ok(None)
        }
        SystemOp::Suspend => detach("systemctl", &["suspend"]).map(|_| None),
        SystemOp::Logout => {
            if gnome {
                detach("gnome-session-quit", &["--logout", "--no-prompt"])?;
            } else {
                let id = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "self".into());
                detach("loginctl", &["terminate-session", &id])?;
            }
            Ok(None)
        }
        SystemOp::Restart => detach("systemctl", &["reboot"]).map(|_| None),
        SystemOp::Shutdown => detach("systemctl", &["poweroff"]).map(|_| None),
        SystemOp::DarkMode => {
            gsettings_set("org.gnome.desktop.interface", "color-scheme", "prefer-dark");
            Ok(Some("Dark mode on".into()))
        }
        SystemOp::LightMode => {
            gsettings_set("org.gnome.desktop.interface", "color-scheme", "default");
            Ok(Some("Light mode on".into()))
        }
        SystemOp::WifiToggle => {
            let state = run("nmcli", &["radio", "wifi"]);
            if !state.ok {
                return Err("NetworkManager (nmcli) not available".into());
            }
            let next = if state.stdout == "enabled" { "off" } else { "on" };
            run("nmcli", &["radio", "wifi", next]);
            Ok(Some(format!("Wi-Fi {next}")))
        }
        SystemOp::DndToggle => {
            let cur = gsettings_get("org.gnome.desktop.notifications", "show-banners");
            let next = if cur == "true" { "false" } else { "true" };
            gsettings_set("org.gnome.desktop.notifications", "show-banners", next);
            Ok(Some(if next == "false" { "Do Not Disturb on" } else { "Do Not Disturb off" }.into()))
        }
        SystemOp::EmptyTrash => {
            if !run("gio", &["trash", "--empty"]).ok {
                return Err("Could not empty the Trash".into());
            }
            Ok(Some("Trash emptied".into()))
        }
        SystemOp::Settings => {
            for bin in ["gnome-control-center", "systemsettings", "xfce4-settings-manager", "cinnamon-settings"] {
                if which(bin).is_some() {
                    detach(bin, &[])?;
                    return Ok(None);
                }
            }
            Err("No settings app found".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_power_commands() {
        let h = History::default();
        let r = system_results("shut", &h, 0);
        assert_eq!(r[0].title, "Shut Down");
        assert!(r[0].confirm.is_some());
        let r = system_results("reboot", &h, 0);
        assert_eq!(r[0].title, "Restart");
        assert!(system_results("qqqq", &h, 0).is_empty());
    }
}
