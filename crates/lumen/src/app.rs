//! The controller: owns the core, the panel window and the desktop integrations, and runs
//! result actions.

use gtk::prelude::*;
use gtk::{gdk, gio, glib, graphene};
use lumen_core::Core;
use lumen_core::store::Settings;
use lumen_core::types::{Action, LumenOp};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::clipboard::ClipWatch;
use crate::ui::Panel;
use crate::ui::widgets::{PANEL_RADIUS, PanelRect};
use crate::{dbus, desktop, paste, theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Search,
    Settings,
    Shortcut,
}

#[derive(Default)]
pub struct ShowOpts {
    pub query: Option<String>,
    pub view: Option<View>,
}

/// What happened when an action ran, and what the panel should do next.
#[derive(Default)]
pub struct ExecResult {
    pub ok: bool,
    pub error: Option<String>,
    /// Keep the panel open (inline commands, settings, clip edits).
    pub keep_open: bool,
    /// Start an inline run of this command and show its output.
    pub run_command: Option<String>,
    pub view: Option<View>,
    pub toast: Option<String>,
    /// Re-run the current search in place, keeping the selection.
    pub refresh: bool,
}

impl ExecResult {
    fn done() -> Self {
        ExecResult { ok: true, ..Default::default() }
    }
    fn toast(t: impl Into<String>) -> Self {
        ExecResult { ok: true, toast: Some(t.into()), ..Default::default() }
    }
    fn err(e: impl Into<String>) -> Self {
        ExecResult { ok: false, error: Some(e.into()), ..Default::default() }
    }
}

/// Hidden this long, the panel's widgets are destroyed; the next Alt+Space rebuilds them.
fn unload_after() -> Duration {
    std::env::var("LUMEN_UNLOAD_MS").ok().and_then(|v| v.parse().ok()).map(Duration::from_millis).unwrap_or(Duration::from_secs(60))
}

pub struct App {
    pub core: Arc<Core>,
    pub gtk: gtk::Application,
    pub clip: Rc<ClipWatch>,
    panel: RefCell<Option<Rc<Panel>>>,
    css: std::cell::OnceCell<gtk::CssProvider>,
    shell_blur: Cell<bool>,
    system_dark: Cell<bool>,
    last_geometry: Cell<Option<PanelRect>>,
    sent_geometry: Cell<Option<Option<PanelRect>>>,
    previous: RefCell<Option<(String, bool, Instant)>>,
    shown_at: Cell<Option<Instant>>,
    hide_timer: RefCell<Option<glib::SourceId>>,
    unload_timer: RefCell<Option<glib::SourceId>>,
    apps_timer: RefCell<Option<glib::SourceId>>,
    interface: RefCell<Option<gio::Settings>>,
    pub debug: bool,
    started: Cell<bool>,
}

fn cancel(slot: &RefCell<Option<glib::SourceId>>) {
    if let Some(id) = slot.take() {
        id.remove();
    }
}

impl App {
    pub fn new(gtk_app: &gtk::Application) -> Rc<App> {
        let core = Arc::new(Core::load());
        Rc::new(App {
            clip: ClipWatch::new(core.clone()),
            core,
            gtk: gtk_app.clone(),
            panel: RefCell::new(None),
            css: std::cell::OnceCell::new(),
            shell_blur: Cell::new(false),
            system_dark: Cell::new(true),
            last_geometry: Cell::new(None),
            sent_geometry: Cell::new(None),
            previous: RefCell::new(None),
            shown_at: Cell::new(None),
            hide_timer: RefCell::new(None),
            unload_timer: RefCell::new(None),
            apps_timer: RefCell::new(None),
            interface: RefCell::new(None),
            debug: std::env::var_os("LUMEN_DEBUG").is_some(),
            started: Cell::new(false),
        })
    }

    /// Once per process, after GTK is up.
    pub fn startup(self: &Rc<Self>) {
        if self.started.replace(true) {
            return;
        }
        let css = self.css.get_or_init(gtk::CssProvider::new);
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        }
        if std::env::var_os("LUMEN_DEBUG_CSS").is_some() {
            css.connect_parsing_error(|_, section, err| eprintln!("[css] {}: {}", section.to_str(), err.message()));
        }
        self.watch_color_scheme();
        self.apply_theme();

        let t = Instant::now();
        self.core.set_apps(desktop::load_apps());
        eprintln!("[apps] {} apps in {}ms", self.core.apps().len(), t.elapsed().as_millis());
        let weak = Rc::downgrade(self);
        gio::AppInfoMonitor::get().connect_changed(move |_| {
            let Some(app) = weak.upgrade() else { return };
            // Installs come in bursts; rescan once they settle.
            cancel(&app.apps_timer);
            let w = Rc::downgrade(&app);
            let id = glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                if let Some(app) = w.upgrade() {
                    app.apps_timer.replace(None);
                    app.core.set_apps(desktop::load_apps());
                }
            });
            app.apps_timer.replace(Some(id));
        });

        self.core.reindex();
        let core = self.core.clone();
        glib::timeout_add_seconds_local(15 * 60, move || {
            core.reindex();
            glib::ControlFlow::Continue
        });
        if self.core.settings().currency_conversion {
            let core = self.core.clone();
            std::thread::spawn(move || {
                core.rates.get();
            });
        }
        self.clip.apply(&self.core.settings());
        let s = self.core.settings();
        lumen_core::integration::set_autostart(s.launch_at_login);
        if !lumen_core::integration::is_dev_build() {
            lumen_core::shortcut::refresh_command();
        }
    }

    // ── Theme ───────────────────────────────────────────────────────────────

    fn watch_color_scheme(self: &Rc<Self>) {
        let schema = "org.gnome.desktop.interface";
        let has = gio::SettingsSchemaSource::default().and_then(|src| src.lookup(schema, true)).is_some_and(|s| s.has_key("color-scheme"));
        if has {
            let gs = gio::Settings::new(schema);
            self.system_dark.set(gs.string("color-scheme") == "prefer-dark");
            let weak = Rc::downgrade(self);
            gs.connect_changed(Some("color-scheme"), move |gs, _| {
                if let Some(app) = weak.upgrade() {
                    app.system_dark.set(gs.string("color-scheme") == "prefer-dark");
                    app.apply_theme();
                }
            });
            self.interface.replace(Some(gs));
        } else if let Some(s) = gtk::Settings::default() {
            self.system_dark.set(s.is_gtk_application_prefer_dark_theme());
        }
    }

    pub fn apply_theme(&self) {
        let s = self.core.settings();
        let dark = theme::resolve_dark(s.theme, self.system_dark.get());
        if let Some(gs) = gtk::Settings::default() {
            gs.set_gtk_application_prefer_dark_theme(dark);
        }
        if let Some(css) = self.css.get() {
            css.load_from_string(&theme::stylesheet(dark, &s.accent, self.shell_blur.get()));
        }
    }

    pub fn is_shell_blur(&self) -> bool {
        self.shell_blur.get()
    }

    // ── Settings ────────────────────────────────────────────────────────────

    pub fn update_settings(self: &Rc<Self>, f: impl FnOnce(&mut Settings)) -> Settings {
        let (old, new) = self.core.update_settings(f);
        if old.theme != new.theme || old.accent != new.accent {
            self.apply_theme();
        }
        self.clip.apply(&new);
        let files = |s: &Settings| (s.file_roots.clone(), s.exclude_dirs.clone(), s.include_hidden, s.file_search);
        if files(&old) != files(&new) {
            self.core.reindex();
        }
        if old.launch_at_login != new.launch_at_login {
            lumen_core::integration::set_autostart(new.launch_at_login);
        }
        if new.currency_conversion && !old.currency_conversion {
            let core = self.core.clone();
            std::thread::spawn(move || {
                core.rates.get();
            });
        }
        if let Some(p) = self.panel() {
            p.on_settings(&new);
        }
        new
    }

    // ── The panel ───────────────────────────────────────────────────────────

    pub fn panel(&self) -> Option<Rc<Panel>> {
        self.panel.borrow().clone()
    }

    fn ensure_panel(self: &Rc<Self>) -> Rc<Panel> {
        if let Some(p) = self.panel() {
            return p;
        }
        let t = Instant::now();
        let p = Panel::new(self);
        let weak = Rc::downgrade(self);
        p.stage().connect_geometry(move |rect| {
            if let Some(app) = weak.upgrade() {
                app.last_geometry.set(Some(rect));
                if app.panel().is_some_and(|p| p.is_open()) {
                    app.emit_geometry(Some(rect));
                }
            }
        });
        self.panel.replace(Some(p.clone()));
        if self.started.get() {
            eprintln!("[panel] built in {}ms", t.elapsed().as_millis());
        }
        p
    }

    pub fn show(self: &Rc<Self>, opts: ShowOpts) {
        cancel(&self.unload_timer);
        cancel(&self.hide_timer);
        let p = self.ensure_panel();
        p.window.set_opacity(1.0);
        p.on_show(opts);
        self.shown_at.set(Some(Instant::now()));
        p.window.present();
        if let Some(r) = self.last_geometry.get() {
            self.emit_geometry(Some(r));
        }
    }

    /// Hide the panel. `animated` lets it fade out first.
    pub fn hide(self: &Rc<Self>, animated: bool) {
        let Some(p) = self.panel() else { return };
        if !p.window.is_visible() || !p.is_open() {
            return;
        }
        p.on_hide();
        self.emit_geometry(None);
        cancel(&self.hide_timer);
        if animated {
            let weak = Rc::downgrade(self);
            let id = glib::timeout_add_local_once(Duration::from_millis(120), move || {
                if let Some(app) = weak.upgrade() {
                    app.hide_timer.replace(None);
                    if let Some(p) = app.panel()
                        && !p.is_open()
                    {
                        p.window.set_visible(false);
                    }
                }
            });
            self.hide_timer.replace(Some(id));
        } else {
            p.window.set_visible(false);
        }
        self.schedule_unload();
    }

    /// GTK on Wayland only starts receiving other apps' clipboard changes once one of its windows
    /// has been on screen. After a `--hidden` start at login nothing has been, so show the panel
    /// for a moment at 1% opacity (a fully transparent window never draws a frame, which isn't
    /// enough); afterwards it keeps working with no windows at all.
    pub fn prime_clipboard(self: &Rc<Self>) {
        let s = self.core.settings();
        if !lumen_core::env::is_wayland() || !s.clipboard_enabled || self.shown_at.get().is_some() {
            return;
        }
        self.show(ShowOpts::default());
        self.shown_at.set(None);
        let Some(p) = self.panel() else { return };
        p.window.set_opacity(0.01);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(300), move || {
            if let Some(app) = weak.upgrade()
                && let Some(p) = app.panel()
                && p.window.opacity() < 1.0
            {
                app.hide(false);
                p.window.set_opacity(1.0);
            }
        });
    }

    pub fn toggle(self: &Rc<Self>) {
        match self.panel() {
            Some(p) if p.window.is_visible() && p.is_open() && p.window.is_active() => self.hide(true),
            _ => self.show(ShowOpts::default()),
        }
    }

    fn schedule_unload(self: &Rc<Self>) {
        cancel(&self.unload_timer);
        if !self.core.settings().free_memory_when_idle {
            return;
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(unload_after(), move || {
            if let Some(app) = weak.upgrade() {
                app.unload_timer.replace(None);
                app.unload_panel();
            }
        });
        self.unload_timer.replace(Some(id));
    }

    /// Destroy the panel's widgets; the next show rebuilds them.
    fn unload_panel(&self) {
        let Some(p) = self.panel() else { return };
        if p.window.is_visible() {
            return;
        }
        self.panel.replace(None);
        p.window.destroy();
        crate::ui::bits::clear_thumbnails();
    }

    /// Called by the panel when the window loses focus.
    pub fn focus_lost(self: &Rc<Self>) {
        if self.debug || self.shown_at.get().is_some_and(|t| t.elapsed() < Duration::from_millis(250)) {
            return;
        }
        self.hide(false);
    }

    fn emit_geometry(&self, rect: Option<PanelRect>) {
        if self.sent_geometry.get() == Some(rect) {
            return;
        }
        self.sent_geometry.set(Some(rect));
        dbus::emit_geometry(rect.map(|r| (r.x, r.y, r.width, r.height, PANEL_RADIUS)));
    }

    pub fn quit(self: &Rc<Self>) {
        self.core.flush();
        self.gtk.quit();
    }

    // ── Paste ───────────────────────────────────────────────────────────────

    /// The window the GNOME extension says had focus before the panel (only if reported since it opened).
    fn previous_window(&self) -> Option<(String, bool)> {
        let shown = self.shown_at.get()?;
        let prev = self.previous.borrow().clone()?;
        (prev.2 + Duration::from_secs(1) >= shown).then_some((prev.0, prev.1))
    }

    fn paste(self: &Rc<Self>, text: &str, terminal: Option<bool>) -> ExecResult {
        self.clip.write(text);
        let prev = self.previous_window();
        let Some(backend) = paste::backend_for(prev.as_ref().is_some_and(|p| p.1)) else {
            // Nothing can press Ctrl+V for us: leave it on the clipboard and say so.
            return ExecResult::toast("Copied — press Ctrl+V");
        };
        let terminal = terminal.unwrap_or_else(|| prev.as_ref().is_some_and(|p| paste::is_terminal_class(&p.0)));
        self.hide(true);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            // Panel fade-out plus time for the compositor to hand focus back.
            glib::timeout_future(Duration::from_millis(240)).await;
            let ok = gio::spawn_blocking(move || paste::inject(backend, terminal)).await.unwrap_or(false);
            if !ok {
                paste::forget_backend(backend);
                eprintln!("[paste] key injection failed; the text is on the clipboard");
                if let Some(app) = weak.upgrade() {
                    let n = gio::Notification::new(lumen_core::constants::APP_NAME);
                    n.set_body(Some("Copied — press Ctrl+V"));
                    app.gtk.send_notification(Some("paste"), &n);
                }
            }
        });
        ExecResult { ok: true, keep_open: true, ..Default::default() }
    }

    // ── Actions ─────────────────────────────────────────────────────────────

    pub fn execute(self: &Rc<Self>, action: &Action, history_key: Option<&str>) -> ExecResult {
        let remember = || {
            if let Some(k) = history_key {
                self.core.record_use(k);
            }
        };
        let s = self.core.settings();
        let done = |r: Result<(), String>| match r {
            Ok(()) => {
                remember();
                ExecResult::done()
            }
            Err(e) => ExecResult::err(e),
        };
        match action {
            Action::LaunchApp { desktop_id, action_id } => match self.core.app(desktop_id) {
                Some(app) => done(desktop::launch_app(&app, action_id.as_deref(), &s)),
                None => ExecResult::err("App no longer installed"),
            },
            Action::OpenPath(path) => done(desktop::open_path(path)),
            Action::RevealPath(path) => {
                desktop::reveal_path(path);
                remember();
                ExecResult::done()
            }
            Action::OpenTerminalAt(path) => {
                let cwd = std::path::Path::new(path).is_dir().then_some(path.as_str());
                match lumen_core::terminal::open_in_terminal(&s, None, cwd) {
                    Ok(()) => ExecResult::done(),
                    Err(e) => ExecResult::err(e),
                }
            }
            Action::Copy(text) => {
                self.clip.write(text);
                remember();
                ExecResult::toast("Copied to clipboard")
            }
            Action::OpenUrl(url) => done(desktop::open_url(url)),
            Action::RunCommand { command, mode } => {
                let command = command.trim().to_string();
                if command.is_empty() {
                    return ExecResult::err("Empty command");
                }
                self.core.record_command(&command);
                match mode {
                    lumen_core::types::RunMode::Terminal => match lumen_core::terminal::open_in_terminal(&s, Some(&command), None) {
                        Ok(()) => ExecResult::done(),
                        Err(e) => ExecResult::err(e),
                    },
                    lumen_core::types::RunMode::Inline => ExecResult { ok: true, keep_open: true, run_command: Some(command), ..Default::default() },
                }
            }
            Action::System(op) => {
                remember();
                match lumen_core::system::run_system(*op) {
                    Ok(Some(t)) => ExecResult::toast(t),
                    Ok(None) => ExecResult::done(),
                    Err(e) => ExecResult::err(e),
                }
            }
            Action::Lumen(op) => match op {
                LumenOp::Settings => ExecResult { ok: true, keep_open: true, view: Some(View::Settings), ..Default::default() },
                LumenOp::Shortcut => ExecResult { ok: true, keep_open: true, view: Some(View::Shortcut), ..Default::default() },
                LumenOp::Reindex => {
                    self.core.reindex();
                    ExecResult::toast("Rebuilding file index…")
                }
                LumenOp::Quit => {
                    let weak = Rc::downgrade(self);
                    glib::idle_add_local_once(move || {
                        if let Some(app) = weak.upgrade() {
                            app.quit();
                        }
                    });
                    ExecResult::done()
                }
            },
            Action::ClipPaste { id, terminal } => match self.core.clip(id) {
                Some(clip) => {
                    self.core.touch_clip(&clip.id);
                    self.paste(&clip.text, *terminal)
                }
                None => ExecResult::err("That clip is gone"),
            },
            Action::ClipCopy(id) => match self.core.clip(id) {
                Some(clip) => {
                    self.clip.write(&clip.text);
                    self.core.touch_clip(&clip.id);
                    ExecResult::toast("Copied to clipboard")
                }
                None => ExecResult::err("That clip is gone"),
            },
            Action::ClipPin { id, pinned } => {
                if !self.core.pin_clip(id, *pinned) {
                    return ExecResult::err("That clip is gone");
                }
                ExecResult {
                    ok: true,
                    keep_open: true,
                    refresh: true,
                    toast: Some(if *pinned { "Pinned" } else { "Unpinned" }.into()),
                    ..Default::default()
                }
            }
            Action::ClipDelete(id) => {
                if !self.core.delete_clip(id) {
                    return ExecResult::err("That clip is gone");
                }
                ExecResult { ok: true, keep_open: true, refresh: true, toast: Some("Deleted".into()), ..Default::default() }
            }
            Action::ClipClear => {
                let n = self.core.clear_clips();
                ExecResult::toast(if n == 0 {
                    "Clipboard history is already empty".to_string()
                } else {
                    format!("Cleared {}", lumen_core::format::plural(n, "clip", "clips"))
                })
            }
        }
    }
}

// ── D-Bus ──────────────────────────────────────────────────────────────────

pub struct BusHandler(pub std::rc::Weak<App>);

impl dbus::Handler for BusHandler {
    fn toggle(&self) {
        if let Some(a) = self.0.upgrade() {
            a.toggle();
        }
    }
    fn show_query(&self, query: String) {
        if let Some(a) = self.0.upgrade() {
            a.show(ShowOpts { query: Some(query), view: None });
        }
    }
    fn hide(&self) {
        if let Some(a) = self.0.upgrade() {
            a.hide(true);
        }
    }
    fn quit(&self) {
        if let Some(a) = self.0.upgrade() {
            a.quit();
        }
    }
    fn previous_window(&self, wm_class: String, x11: bool) {
        if let Some(a) = self.0.upgrade() {
            a.previous.replace(Some((wm_class, x11, Instant::now())));
        }
    }
    fn clipboard_secret(&self) {
        if let Some(a) = self.0.upgrade() {
            a.clip.mark_secret();
        }
    }
    fn shell_blur(&self) -> bool {
        self.0.upgrade().is_some_and(|a| a.shell_blur.get())
    }
    fn set_shell_blur(&self, on: bool) {
        let Some(a) = self.0.upgrade() else { return };
        if a.shell_blur.replace(on) != on {
            a.apply_theme();
            if let Some(p) = a.panel() {
                p.on_shell_blur(on);
            }
        }
        // The extension (re)connected: give it the current panel straight away.
        if on {
            a.sent_geometry.set(None);
            let open = a.panel().is_some_and(|p| p.is_open() && p.window.is_visible());
            a.emit_geometry(if open { a.last_geometry.get() } else { None });
        }
    }
    fn capture(&self, path: String) -> Result<(), String> {
        if !lumen_core::integration::is_dev_build() {
            return Err("Capture is only available in development builds".into());
        }
        let a = self.0.upgrade().ok_or("gone")?;
        let p = a.panel().ok_or("panel not built")?;
        let w = &p.window;
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(w)).snapshot(&snapshot, w.width() as f64, w.height() as f64);
        let node = snapshot.to_node().ok_or("nothing drawn")?;
        let renderer = w.renderer().ok_or("no renderer")?;
        let r = p.stage().panel_rect();
        let pad = 24.0;
        let area = graphene::Rect::new(r.x as f32 - pad, r.y as f32 - pad, r.width as f32 + 2.0 * pad, r.height as f32 + 2.0 * pad);
        let tex = renderer.render_texture(&node, Some(&area));
        tex.save_to_png(&path).map_err(|e| e.to_string())
    }
    fn key(&self, combo: String) -> Result<(), String> {
        if !lumen_core::integration::is_dev_build() {
            return Err("Key is only available in development builds".into());
        }
        let a = self.0.upgrade().ok_or("gone")?;
        let p = a.panel().ok_or("panel not built")?;
        p.press(&combo)
    }
}
