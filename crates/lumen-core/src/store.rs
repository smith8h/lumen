//! Settings, usage history (frecency) and small JSON files in Lumen's config folder
//! (`~/.config/Lumen`, the same place the Electron version used, so nothing is lost).

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::xdg;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Lumen's config folder. `LUMEN_USER_DATA` points it elsewhere, e.g. for screenshots or tests.
pub fn config_dir() -> PathBuf {
    match std::env::var_os("LUMEN_USER_DATA") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => xdg::config_home().join(crate::constants::APP_NAME),
    }
}

pub fn read_json<T: DeserializeOwned>(file: &str) -> Option<T> {
    let text = fs::read_to_string(config_dir().join(file)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write atomically so a crash never leaves half a file behind. `mode` restricts who can read it.
pub fn write_json<T: Serialize + ?Sized>(file: &str, data: &T, mode: Option<u32>) -> std::io::Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let target = dir.join(file);
    let tmp = dir.join(format!("{file}.tmp"));
    let text = serde_json::to_string_pretty(data).map_err(std::io::Error::other)?;
    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        if let Some(m) = mode {
            opts.mode(m);
        }
        let mut f = opts.open(&tmp)?;
        // `mode` only applies when the file is created; a leftover .tmp keeps its old permissions.
        if let Some(m) = mode {
            f.set_permissions(fs::Permissions::from_mode(m))?;
        }
        f.write_all(text.as_bytes())?;
        f.sync_data().ok();
    }
    fs::rename(&tmp, &target)
}

pub fn remove_json(file: &str) {
    let _ = fs::remove_file(config_dir().join(file));
}

pub fn file_path(file: &str) -> PathBuf {
    config_dir().join(file)
}

// ── Settings ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchEngine {
    #[default]
    Google,
    Duckduckgo,
    Brave,
    Bing,
    Startpage,
    Custom,
}

impl SearchEngine {
    pub const ALL: [SearchEngine; 6] = [Self::Google, Self::Duckduckgo, Self::Brave, Self::Bing, Self::Startpage, Self::Custom];
    pub fn id(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Duckduckgo => "duckduckgo",
            Self::Brave => "brave",
            Self::Bing => "bing",
            Self::Startpage => "startpage",
            Self::Custom => "custom",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Custom => "Custom…",
            other => crate::constants::SEARCH_ENGINES.iter().find(|e| e.id == other.id()).map_or("Google", |e| e.name),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: ThemeMode,
    pub accent: String,
    pub search_engine: SearchEngine,
    pub custom_search_url: String,
    pub terminal: String,
    pub file_search: bool,
    pub file_roots: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub include_hidden: bool,
    pub launch_at_login: bool,
    pub show_preview: bool,
    pub currency_conversion: bool,
    /// Destroy the panel's widgets after it has been hidden for a while.
    pub free_memory_when_idle: bool,
    /// Remember text you copy, searchable with ";".
    pub clipboard_enabled: bool,
    /// Keep clipboard history on disk across restarts.
    pub clipboard_persist: bool,
    /// How many clips to keep (pinned clips are never dropped).
    pub clipboard_max: u32,
    /// Skip copies that password managers mark as secret.
    pub clipboard_ignore_secrets: bool,
    /// Temporarily stop recording without turning history off.
    pub clipboard_paused: bool,
    pub first_run_done: bool,
}

pub const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules",
    ".git",
    ".cache",
    ".npm",
    ".cargo",
    ".rustup",
    ".gradle",
    ".m2",
    ".pub-cache",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".nuxt",
    ".dart_tool",
    "build",
    "target",
    "snap",
    ".var",
    "Trash",
    ".local/share/Trash",
    "Android/Sdk",
    ".android",
    ".sdks",
    ".vscode-server",
    ".wine",
    "*.asar.unpacked",
    "site-packages",
    "dist-packages",
    ".flatpak-builder",
];

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            accent: "#7c6cff".into(),
            search_engine: SearchEngine::Google,
            custom_search_url: "https://example.com/search?q=%s".into(),
            terminal: "auto".into(),
            file_search: true,
            file_roots: vec![xdg::home().to_string_lossy().into_owned()],
            exclude_dirs: DEFAULT_EXCLUDES.iter().map(|s| s.to_string()).collect(),
            include_hidden: false,
            launch_at_login: true,
            show_preview: true,
            currency_conversion: true,
            free_memory_when_idle: true,
            clipboard_enabled: true,
            clipboard_persist: true,
            clipboard_max: 200,
            clipboard_ignore_secrets: true,
            clipboard_paused: false,
            first_run_done: false,
        }
    }
}

impl Settings {
    /// Load settings.json. Each known key is taken on its own, so one bad value (or an option
    /// from an older version) never resets the rest.
    pub fn load() -> Settings {
        let Some(Value::Object(saved)) = read_json::<Value>("settings.json") else { return Settings::default() };
        Self::from_map(saved)
    }

    pub fn from_map(saved: Map<String, Value>) -> Settings {
        let Value::Object(mut base) = serde_json::to_value(Settings::default()).unwrap_or(Value::Null) else {
            return Settings::default();
        };
        for (key, value) in saved {
            if !base.contains_key(&key) {
                continue;
            }
            let mut trial = base.clone();
            trial.insert(key.clone(), value.clone());
            if serde_json::from_value::<Settings>(Value::Object(trial)).is_ok() {
                base.insert(key, value);
            }
        }
        serde_json::from_value::<Settings>(Value::Object(base)).unwrap_or_default().sanitized()
    }

    /// Clamp values a hand-edited file (or a bug) could get wrong.
    pub fn sanitized(mut self) -> Settings {
        let hex = self.accent.len() == 7 && self.accent.starts_with('#') && self.accent[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !hex {
            self.accent = Settings::default().accent;
        }
        self.clipboard_max = self.clipboard_max.clamp(10, 5000);
        self
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json("settings.json", self, None)
    }
}

// ── Frecency: things you open often and recently rank higher ──────────────

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct HistoryEntry {
    pub count: u32,
    /// Milliseconds since the epoch.
    pub last: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct History {
    #[serde(default)]
    pub items: HashMap<String, HistoryEntry>,
    #[serde(default)]
    pub commands: Vec<String>,
}

impl History {
    pub fn load() -> History {
        read_json("history.json").unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_json("history.json", self, None)
    }

    pub fn record_use(&mut self, key: &str, now: i64) {
        let e = self.items.entry(key.to_string()).or_insert(HistoryEntry { count: 0, last: 0 });
        e.count += 1;
        e.last = now;
        // Keep the file small.
        if self.items.len() > 2000 {
            let mut keys: Vec<(String, i64)> = self.items.iter().map(|(k, v)| (k.clone(), v.last)).collect();
            keys.sort_by_key(|(_, last)| *last);
            let drop = keys.len() - 1500;
            for (k, _) in keys.into_iter().take(drop) {
                self.items.remove(&k);
            }
        }
    }

    /// 0..~180 bonus based on how often and how recently something was used.
    pub fn frecency(&self, key: &str, now: i64) -> f64 {
        let Some(e) = self.items.get(key) else { return 0.0 };
        let age_days = (now - e.last) as f64 / 86_400_000.0;
        let recency = if age_days < 1.0 {
            1.0
        } else if age_days < 7.0 {
            0.7
        } else if age_days < 30.0 {
            0.45
        } else {
            0.2
        };
        (((e.count + 1) as f64).log2() * 45.0 * recency).min(180.0)
    }

    pub fn record_command(&mut self, cmd: &str) {
        self.commands.retain(|c| c != cmd);
        self.commands.insert(0, cmd.to_string());
        self.commands.truncate(100);
    }
}

/// True when `path` is inside the user's home folder.
pub fn tilde(path: &str) -> String {
    let home = xdg::home();
    let home = home.to_string_lossy();
    let home = home.trim_end_matches('/');
    if !home.is_empty() && (path == home || path.starts_with(&format!("{home}/"))) { format!("~{}", &path[home.len()..]) } else { path.to_string() }
}

/// "~/x" → "/home/me/x".
pub fn expand_tilde(path: &str) -> String {
    if path == "~" || path.starts_with("~/") { format!("{}{}", xdg::home().to_string_lossy(), &path[1..]) } else { path.to_string() }
}

pub fn exists(p: &str) -> bool {
    Path::new(p).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_values_keep_the_rest() {
        let saved: Map<String, Value> = serde_json::from_str(
            r##"{"theme":"dark","accent":"nope","clipboardMax":99999,"searchEngine":"altavista","glassOpacity":0.5,"showPreview":false}"##,
        )
        .unwrap();
        let s = Settings::from_map(saved);
        assert_eq!(s.theme, ThemeMode::Dark);
        assert_eq!(s.accent, "#7c6cff");
        assert_eq!(s.clipboard_max, 5000);
        assert_eq!(s.search_engine, SearchEngine::Google);
        assert!(!s.show_preview);
    }

    #[test]
    fn frecency_grows_with_use() {
        let mut h = History::default();
        h.record_use("app:x", 1000);
        let once = h.frecency("app:x", 1000);
        h.record_use("app:x", 1000);
        assert!(h.frecency("app:x", 1000) > once);
        assert_eq!(h.frecency("app:y", 1000), 0.0);
        assert!(h.frecency("app:x", 1000 + 40 * 86_400_000) < once);
    }
}
