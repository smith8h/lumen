//! Start at login.

use std::fs;

use crate::constants::{APP_ID, APP_NAME};
use crate::env::launcher_path;
use crate::xdg::config_home;

/// A build run straight from the source tree can't be started at login, so it never sets that up.
pub fn is_dev_build() -> bool {
    std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains("/target/"))
}

fn exec_quote(s: &str) -> String {
    if s.chars().any(|c| c.is_whitespace() || "\"'\\$`".contains(c)) {
        let mut out = String::from("\"");
        for c in s.chars() {
            if matches!(c, '"' | '\\' | '$' | '`') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        s.to_string()
    }
}

pub fn set_autostart(enabled: bool) {
    if is_dev_build() {
        return;
    }
    let dir = config_home().join("autostart");
    let file = dir.join("lumen.desktop");
    if !enabled {
        let _ = fs::remove_file(file);
        return;
    }
    let content = [
        "[Desktop Entry]".to_string(),
        "Type=Application".into(),
        format!("Name={APP_NAME}"),
        "Comment=Spotlight-style launcher".into(),
        format!("Exec={} --hidden", exec_quote(&launcher_path())),
        format!("Icon={APP_ID}"),
        "Terminal=false".into(),
        "NoDisplay=true".into(),
        "X-GNOME-Autostart-enabled=true".into(),
        "X-GNOME-Autostart-Delay=2".into(),
        String::new(),
    ]
    .join("\n");
    if fs::read_to_string(&file).ok().as_deref() == Some(content.as_str()) {
        return;
    }
    if fs::create_dir_all(&dir).and_then(|_| fs::write(&file, content)).is_err() {
        eprintln!("[integration] could not write {}", file.display());
    }
}
