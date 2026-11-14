//! Paste a clip into the app that was focused before Lumen opened.
//!
//! Order matters: put the text on the clipboard while Lumen still has focus (Wayland only lets
//! the focused app set it), hide the panel so focus goes back, wait for that to settle, then
//! press Ctrl+V with ydotool (Wayland, any window) or xdotool (X11 / XWayland windows). The text
//! itself never touches a command line.

use lumen_core::env::{is_wayland, run_timeout, which};
use std::cell::Cell;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Ydotool,
    Xdotool,
}

thread_local! {
    static CACHED: Cell<Option<Backend>> = const { Cell::new(None) };
}

fn ydotool_socket() -> Option<String> {
    let path = std::env::var("YDOTOOL_SOCKET")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("XDG_RUNTIME_DIR").ok().map(|d| format!("{d}/.ydotool_socket")))?;
    std::path::Path::new(&path).exists().then_some(path)
}

/// The best way to press keys in other apps right now (None = none). Only a found backend is cached.
pub fn backend_for(x11_target: bool) -> Option<Backend> {
    let wayland = is_wayland();
    match CACHED.get() {
        Some(Backend::Ydotool) => return Some(Backend::Ydotool),
        Some(Backend::Xdotool) if !wayland => return Some(Backend::Xdotool),
        _ => {}
    }
    if which("ydotool").is_some() && ydotool_socket().is_some() {
        CACHED.set(Some(Backend::Ydotool));
        return Some(Backend::Ydotool);
    }
    if !wayland && which("xdotool").is_some() {
        CACHED.set(Some(Backend::Xdotool));
        return Some(Backend::Xdotool);
    }
    // Without ydotool on Wayland, xdotool can still reach an XWayland window.
    (wayland && x11_target && which("xdotool").is_some()).then_some(Backend::Xdotool)
}

pub fn forget_backend(b: Backend) {
    if CACHED.get() == Some(b) {
        CACHED.set(None);
    }
}

// Linux input event codes for ydotool.
const KEY_CTRL: u32 = 29;
const KEY_SHIFT: u32 = 42;
const KEY_V: u32 = 47;

fn args_for(backend: Backend, terminal: bool) -> Vec<String> {
    match backend {
        Backend::Xdotool => vec!["key".into(), "--clearmodifiers".into(), if terminal { "ctrl+shift+v" } else { "ctrl+v" }.into()],
        Backend::Ydotool => {
            let down: Vec<u32> = if terminal { vec![KEY_CTRL, KEY_SHIFT, KEY_V] } else { vec![KEY_CTRL, KEY_V] };
            let mut args = vec!["key".to_string()];
            args.extend(down.iter().map(|k| format!("{k}:1")));
            args.extend(down.iter().rev().map(|k| format!("{k}:0")));
            args
        }
    }
}

/// Press Ctrl+V (or Ctrl+Shift+V). Blocking; run it off the UI thread.
pub fn inject(backend: Backend, terminal: bool) -> bool {
    let bin = match backend {
        Backend::Ydotool => "ydotool",
        Backend::Xdotool => "xdotool",
    };
    let args = args_for(backend, terminal);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_timeout(bin, &refs, Duration::from_secs(2)).ok
}

const TERMINAL_CLASSES: &[&str] =
    &["console", "terminal", "gnome-terminal-server", "terminator", "tilda", "guake", "yakuake", "rio", "contour", "blackbox", "terminology"];

/// "org.gnome.Ptyxis" → ptyxis, "com.mitchellh.ghostty" → ghostty, "org.gnome.Console" → console.
pub fn is_terminal_class(wm_class: &str) -> bool {
    let c = wm_class.to_lowercase();
    let last = c.rsplit('.').next().unwrap_or(&c).to_string();
    let known = |s: &str| TERMINAL_CLASSES.contains(&s) || lumen_core::terminal::known_terminals().any(|t| t == s);
    known(&c) || known(&last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_terminals() {
        assert!(is_terminal_class("org.gnome.Ptyxis"));
        assert!(is_terminal_class("com.mitchellh.ghostty"));
        assert!(is_terminal_class("org.gnome.Console"));
        assert!(is_terminal_class("kitty"));
        assert!(!is_terminal_class("firefox"));
    }

    #[test]
    fn ydotool_presses_and_releases_in_order() {
        assert_eq!(args_for(Backend::Ydotool, false), ["key", "29:1", "47:1", "47:0", "29:0"]);
        assert_eq!(args_for(Backend::Xdotool, true), ["key", "--clearmodifiers", "ctrl+shift+v"]);
    }
}
