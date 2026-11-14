//! XDG base directories.

use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn env_dir(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => home().join(fallback),
    }
}

pub fn config_home() -> PathBuf {
    env_dir("XDG_CONFIG_HOME", ".config")
}

pub fn data_home() -> PathBuf {
    env_dir("XDG_DATA_HOME", ".local/share")
}
