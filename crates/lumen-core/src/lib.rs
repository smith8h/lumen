//! Lumen's core: search providers, ranking, calculator, clipboard history and storage.
//! Nothing in here depends on the UI toolkit, so it is unit-tested with plain `cargo test`.

pub mod apps;
pub mod calculator;
pub mod cliphistory;
pub mod clips;
pub mod constants;
pub mod debounce;
pub mod env;
pub mod files;
pub mod format;
pub mod fuzzy;
pub mod integration;
pub mod rates;
pub mod search;
pub mod shortcut;
pub mod store;
pub mod system;
pub mod terminal;
pub mod types;
pub mod units;
pub mod web;
pub mod xdg;

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use apps::AppEntry;
use cliphistory::{ClipEntry, ClipHistory};
use files::{FileIndex, IndexOptions};
use rates::Rates;
use store::{History, Settings, now_ms};
use types::*;

const CLIP_FILE: &str = "clipboard.json";

/// Everything Lumen knows, shared between the UI thread and search workers.
pub struct Core {
    settings: Arc<RwLock<Settings>>,
    history: Arc<Mutex<History>>,
    history_saver: debounce::Debouncer,
    apps: RwLock<Arc<Vec<AppEntry>>>,
    clips: Arc<Mutex<ClipHistory>>,
    clip_saver: debounce::Debouncer,
    pub files: Arc<FileIndex>,
    pub rates: Rates,
}

const STARTER_APPS: &[&str] = &[
    "org.gnome.Nautilus.desktop",
    "brave-browser.desktop",
    "brave_brave.desktop",
    "firefox.desktop",
    "firefox_firefox.desktop",
    "google-chrome.desktop",
    "code.desktop",
    "org.gnome.Ptyxis.desktop",
    "org.gnome.Terminal.desktop",
    "org.gnome.Settings.desktop",
    "org.gnome.TextEditor.desktop",
    "org.gnome.Calculator.desktop",
    "telegram-desktop.desktop",
    "spotify.desktop",
    "org.gnome.Software.desktop",
];

impl Default for Core {
    fn default() -> Self {
        Self::load()
    }
}

impl Core {
    /// Load settings, history and saved clips from the config folder.
    pub fn load() -> Core {
        let settings = Arc::new(RwLock::new(Settings::load()));
        let history = Arc::new(Mutex::new(History::load()));
        let s = settings.read().unwrap().clone();
        let mut clip_history = ClipHistory::new(s.clipboard_max as usize);
        if s.clipboard_persist
            && let Some(saved) = store::read_json::<serde_json::Value>(CLIP_FILE)
            && let Some(entries) = saved.get("entries")
        {
            clip_history.load(entries);
        }
        let clips = Arc::new(Mutex::new(clip_history));

        let h = history.clone();
        let history_saver = debounce::Debouncer::new("history", Duration::from_millis(500), move || {
            let data = h.lock().unwrap().clone();
            if let Err(e) = data.save() {
                eprintln!("[store] could not save history: {e}");
            }
        });
        let (c, st) = (clips.clone(), settings.clone());
        let clip_saver = debounce::Debouncer::new("clipboard", Duration::from_millis(1000), move || {
            if !st.read().unwrap().clipboard_persist {
                return;
            }
            let entries = c.lock().unwrap().all().to_vec();
            // Never log clip contents, only that saving failed.
            if store::write_json(CLIP_FILE, &serde_json::json!({ "version": 1, "entries": entries }), Some(0o600)).is_err() {
                eprintln!("[clipboard] could not save history");
            }
        });
        Core {
            settings,
            history,
            history_saver,
            apps: RwLock::new(Arc::new(Vec::new())),
            clips,
            clip_saver,
            files: Arc::new(FileIndex::default()),
            rates: Rates::default(),
        }
    }

    // ── Settings ────────────────────────────────────────────────────────────

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    /// Change settings and save them. Returns (old, new).
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) -> (Settings, Settings) {
        let mut s = self.settings.write().unwrap();
        let old = s.clone();
        f(&mut s);
        *s = s.clone().sanitized();
        if let Err(e) = s.save() {
            eprintln!("[store] could not save settings: {e}");
        }
        let new = s.clone();
        drop(s);
        let mut clips = self.clips.lock().unwrap();
        clips.set_max(new.clipboard_max as usize);
        drop(clips);
        if new.clipboard_persist {
            self.clip_saver.poke();
        } else if old.clipboard_persist {
            store::remove_json(CLIP_FILE);
        }
        (old, new)
    }

    // ── Usage history ───────────────────────────────────────────────────────

    pub fn record_use(&self, key: &str) {
        if key.len() < 4096 {
            self.history.lock().unwrap().record_use(key, now_ms());
            self.history_saver.poke();
        }
    }

    pub fn record_command(&self, cmd: &str) {
        self.history.lock().unwrap().record_command(cmd);
        self.history_saver.poke();
    }

    fn frecency(&self, key: &str) -> f64 {
        self.history.lock().unwrap().frecency(key, now_ms())
    }

    // ── Apps and files ──────────────────────────────────────────────────────

    pub fn set_apps(&self, apps: Vec<AppEntry>) {
        *self.apps.write().unwrap() = Arc::new(apps);
    }

    pub fn apps(&self) -> Arc<Vec<AppEntry>> {
        self.apps.read().unwrap().clone()
    }

    pub fn app(&self, id: &str) -> Option<AppEntry> {
        self.apps().iter().find(|a| a.id == id).cloned()
    }

    /// Rebuild the file index on a background thread.
    pub fn reindex(&self) {
        let s = self.settings();
        if !s.file_search {
            return;
        }
        let roots: Vec<String> = s.file_roots.iter().map(|r| store::expand_tilde(r)).filter(|r| store::exists(r)).collect();
        let opts = IndexOptions { roots, exclude_dirs: s.exclude_dirs.clone(), include_hidden: s.include_hidden };
        let files = self.files.clone();
        std::thread::Builder::new()
            .name("file-index".into())
            .spawn(move || {
                let t = Instant::now();
                let n = files.build(&opts);
                eprintln!("[files] indexed {n} entries in {}ms", t.elapsed().as_millis());
            })
            .ok();
    }

    // ── Clipboard history ───────────────────────────────────────────────────

    pub fn add_clip(&self, text: &str) -> Option<ClipEntry> {
        let e = self.clips.lock().unwrap().add(text, now_ms());
        if e.is_some() {
            self.clip_saver.poke();
        }
        e
    }

    pub fn clip(&self, id: &str) -> Option<ClipEntry> {
        self.clips.lock().unwrap().get(id).cloned()
    }

    pub fn clip_count(&self) -> usize {
        self.clips.lock().unwrap().len()
    }

    fn change_clips(&self, f: impl FnOnce(&mut ClipHistory) -> bool) -> bool {
        let ok = f(&mut self.clips.lock().unwrap());
        if ok {
            self.clip_saver.poke();
        }
        ok
    }

    pub fn touch_clip(&self, id: &str) -> bool {
        self.change_clips(|c| c.touch(id, now_ms()))
    }
    pub fn pin_clip(&self, id: &str, pinned: bool) -> bool {
        self.change_clips(|c| c.pin(id, pinned))
    }
    pub fn delete_clip(&self, id: &str) -> bool {
        self.change_clips(|c| c.remove(id))
    }
    pub fn clear_clips(&self) -> usize {
        let n = self.clips.lock().unwrap().clear();
        self.clip_saver.poke();
        n
    }

    /// Write anything pending (on quit).
    pub fn flush(&self) {
        self.history_saver.flush();
        self.clip_saver.flush();
    }

    // ── Search ──────────────────────────────────────────────────────────────

    fn search_files(&self, q: &str, limit: usize) -> Vec<ResultItem> {
        if !self.settings.read().unwrap().file_search || q.trim().chars().count() < 2 {
            return Vec::new();
        }
        self.files
            .search(q, limit)
            .into_iter()
            .map(|h| {
                let f = self.frecency(&format!("file:{}", h.path));
                files::file_result(&h.path, h.dir, h.score * 0.88, h.indexes, f)
            })
            .collect()
    }

    /// Run a query. Blocking (the file index and exchange rates may take a moment), so the UI
    /// calls it on a worker thread and drops answers whose `seq` is no longer current.
    pub fn search(&self, raw: &str, seq: u64) -> SearchResponse {
        let started = Instant::now();
        let raw: String = raw.chars().take(500).collect();
        let q = raw.trim();
        let s = self.settings();
        let now = now_ms();
        let mode = |label: &str, glyph| Some(SearchMode { label: label.into(), glyph });
        let (mode, items, top_hit) = if q.starts_with('>') || q.starts_with('$') {
            let cmd = q[1..].trim_start();
            let history = self.history.lock().unwrap().commands.clone();
            (mode("Terminal", Glyph::Terminal), terminal::command_results(cmd, &history), false)
        } else if let Some(rest) = q.strip_prefix('?') {
            let mut items = web::web_results(rest, &s);
            items.iter_mut().for_each(|i| i.score += 2000.0);
            (mode(&web::engine_label(&s), Glyph::Globe), items, false)
        } else if let Some(rest) = q.strip_prefix(';') {
            // Clipboard history only ever shows up behind its prefix (privacy, and less noise).
            let label = if s.clipboard_paused { "Clipboard · paused" } else { "Clipboard" };
            let items = clips::clip_results(&self.clips.lock().unwrap(), rest.trim(), &s, now);
            (mode(label, Glyph::Clipboard), items, false)
        } else if q.starts_with("~/") || q.starts_with('/') || q == "~" {
            let items = files::browse(q, &|k| self.frecency(k));
            (mode("Files", Glyph::Folder), items, false)
        } else if !q.is_empty() {
            let is_url = web::as_url(q).is_some();
            let calc = calculator::calculate(q, s.currency_conversion, &self.rates);
            let files = if is_url { Vec::new() } else { self.search_files(q, 10) };
            let apps = {
                let history = self.history.lock().unwrap();
                apps::search_apps(&self.apps(), q, 7, &history, now)
            };
            let system = system::system_results(q, &self.history.lock().unwrap(), now);
            let web = web::web_results(q, &s);
            let strong_app = apps.iter().any(|a| a.score > 700.0);
            let mode = if !calc.is_empty() {
                mode("Calculator", Glyph::Calculator)
            } else if is_url {
                mode("Open URL", Glyph::Globe)
            } else {
                None
            };
            let bare = if !strong_app && calc.is_empty() { terminal::bare_command_result(q) } else { None };
            let mut items: Vec<ResultItem> = calc.into_iter().chain(apps).chain(system).chain(files).chain(web).chain(bare).collect();
            items = search::cap_kinds(
                items,
                &[(ResultKind::File, 6), (ResultKind::Folder, 4), (ResultKind::App, 6), (ResultKind::Command, 2), (ResultKind::Web, 3)],
            );
            (mode, items, true)
        } else {
            (None, Vec::new(), false)
        };
        SearchResponse { seq, query: raw.clone(), sections: search::group(items, top_hit), mode, took_ms: started.elapsed().as_millis() as u64 }
    }

    /// What the start screen shows before typing: favourite apps and recent files.
    pub fn suggestions(&self) -> Vec<ResultSection> {
        let apps = self.apps();
        let now = now_ms();
        let mut items = apps::frequent_apps(&apps, 8, &self.history.lock().unwrap(), now);
        for id in STARTER_APPS {
            if items.len() >= 8 {
                break;
            }
            if let Some(app) = apps.iter().find(|a| a.id == *id)
                && !items.iter().any(|i| i.id == format!("app:{id}"))
            {
                items.push(apps::app_result(app));
            }
        }
        let mut sections = Vec::new();
        if !items.is_empty() {
            sections.push(ResultSection { id: SectionId::Apps, title: "Suggestions".into(), items });
        }
        let recent = files::recent_files(4);
        if !recent.is_empty() {
            sections.push(ResultSection { id: SectionId::Files, title: "Recent Files".into(), items: recent });
        }
        sections
    }

    pub fn preview(&self, item: &ResultItem) -> Preview {
        match &item.action {
            Action::LaunchApp { desktop_id, .. } => self.app(desktop_id).map_or(Preview::None, |a| apps::app_preview(&a)),
            Action::OpenPath(path) => files::file_preview(path),
            Action::ClipPaste { id, .. } => clips::clip_preview(&self.clips.lock().unwrap(), id),
            _ if item.kind == ResultKind::Calc => Preview::Calc {
                expression: item.id.strip_prefix("calc:").map(String::from).unwrap_or_default(),
                result: item.title.clone(),
                detail: item.subtitle.clone(),
            },
            _ => Preview::None,
        }
    }
}
