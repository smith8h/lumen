//! Settings: a sidebar of pages and the controls on each.

use gtk::prelude::*;
use gtk::{gio, glib};
use lumen_core::constants::{APP_NAME, REPO_URL, VERSION};
use lumen_core::format::{count, plural};
use lumen_core::shortcut::{self, ShortcutStatus};
use lumen_core::store::{SearchEngine, Settings, ThemeMode, tilde};
use lumen_core::terminal::TERMINAL_CHOICES;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::Panel;
use super::bits::{button, ellipsized, glyph, hbox, keys, label, mark, vbox};

/// Curated accents, tuned to read well on both glass themes. The first is Lumen's default.
pub const ACCENTS: &[(&str, &str)] =
    &[("Iris", "#7c6cff"), ("Blue", "#3b82f6"), ("Teal", "#14b8a6"), ("Green", "#22c55e"), ("Amber", "#f59e0b"), ("Rose", "#f43f5e")];
const CLIP_SIZES: &[u32] = &[50, 100, 200, 500, 1000];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    General,
    Appearance,
    Search,
    Files,
    Clipboard,
    About,
}

const PAGES: &[(Page, &str, &str, &str)] = &[
    (Page::General, "General", "settings-2", "How Lumen starts and opens."),
    (Page::Appearance, "Appearance", "palette", "Theme, colour and layout of the panel."),
    (Page::Search, "Search", "search", "The web, calculator and conversions."),
    (Page::Files, "Files", "folder", "Which files and folders Lumen can find."),
    (Page::Clipboard, "Clipboard", "clipboard-list", "Text you copy, searchable with “;”. It never leaves this computer."),
    (Page::About, "About", "info", ""),
];

thread_local! {
    static CURRENT: RefCell<Option<Rc<SettingsView>>> = const { RefCell::new(None) };
}

pub struct SettingsView {
    panel: Weak<Panel>,
    nav_items: RefCell<Vec<(gtk::Button, gtk::Box)>>,
    scroller: gtk::ScrolledWindow,
    page: Cell<usize>,
    shortcut: RefCell<Option<ShortcutStatus>>,
    confirm_clear: Cell<bool>,
    clear_timer: RefCell<Option<glib::SourceId>>,
}

impl SettingsView {
    pub fn build(panel: &Rc<Panel>) -> gtk::Widget {
        let root = hbox(0, "settings");
        root.set_size_request(-1, 452);
        let nav = vbox(2, "settings-nav");
        let header = hbox(4, "");
        header.set_margin_start(4);
        header.set_margin_bottom(12);
        let back = gtk::Button::new();
        back.add_css_class("icon-btn");
        back.set_child(Some(&glyph("arrow-left", 18)));
        back.set_tooltip_text(Some("Back to search (Esc)"));
        let w = Rc::downgrade(panel);
        back.connect_clicked(move |_| {
            if let Some(p) = w.upgrade() {
                p.back_to_search();
            }
        });
        header.append(&back);
        header.append(&label("Settings", "nav-title"));
        nav.append(&header);

        let scroller = gtk::ScrolledWindow::new();
        scroller.add_css_class("scroll");
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_hexpand(true);
        scroller.set_vexpand(true);

        let view = Rc::new(SettingsView {
            panel: Rc::downgrade(panel),
            nav_items: RefCell::new(Vec::new()),
            scroller: scroller.clone(),
            page: Cell::new(0),
            shortcut: RefCell::new(None),
            confirm_clear: Cell::new(false),
            clear_timer: RefCell::new(None),
        });

        for (i, (_, name, icon, _)) in PAGES.iter().enumerate() {
            let b = gtk::Button::new();
            b.add_css_class("nav-item");
            b.set_focus_on_click(false);
            let row = hbox(10, "");
            let light = hbox(0, "light");
            light.set_size_request(3, 15);
            light.set_valign(gtk::Align::Center);
            row.append(&light);
            row.append(&glyph(icon, 16));
            row.append(&gtk::Label::new(Some(name)));
            b.set_child(Some(&row));
            let v = Rc::downgrade(&view);
            b.connect_clicked(move |_| {
                if let Some(v) = v.upgrade() {
                    v.go(i);
                }
            });
            nav.append(&b);
            view.nav_items.borrow_mut().push((b, light));
        }
        let foot = hbox(6, "nav-foot");
        foot.set_vexpand(true);
        foot.set_valign(gtk::Align::End);
        foot.append(&keys("esc"));
        foot.append(&gtk::Label::new(Some("to go back")));
        nav.append(&foot);

        root.append(&nav);
        root.append(&scroller);

        let v = Rc::downgrade(&view);
        glib::spawn_future_local(async move {
            let Ok(s) = gio::spawn_blocking(shortcut::status).await else { return };
            if let Some(v) = v.upgrade() {
                v.shortcut.replace(Some(s));
                if v.page.get() == 0 {
                    v.render(true);
                }
            }
        });
        view.render(false);
        CURRENT.with(|c| c.replace(Some(view)));
        root.upcast()
    }

    /// ↑/↓ (or Ctrl+Tab) move between pages.
    pub fn step_page(_panel: &Rc<Panel>, d: isize) {
        let Some(v) = CURRENT.with(|c| c.borrow().clone()) else { return };
        let n = PAGES.len() as isize;
        let next = ((v.page.get() as isize + d) % n + n) % n;
        v.go(next as usize);
        if let Some((b, _)) = v.nav_items.borrow().get(next as usize) {
            b.grab_focus();
        }
    }

    fn go(self: &Rc<Self>, i: usize) {
        if self.page.get() != i {
            self.page.set(i);
            self.confirm_clear.set(false);
            self.render(false);
        }
    }

    fn settings(&self) -> Settings {
        self.panel.upgrade().and_then(|p| p.app_handle()).map(|a| a.core.settings()).unwrap_or_default()
    }

    /// Change settings, then redraw this page (after the signal that caused it has finished).
    fn change(self: &Rc<Self>, f: impl FnOnce(&mut Settings) + 'static) {
        if let Some(app) = self.panel.upgrade().and_then(|p| p.app_handle()) {
            app.update_settings(f);
        }
        let v = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(v) = v.upgrade() {
                v.render(true);
            }
        });
    }

    fn render(self: &Rc<Self>, keep_scroll: bool) {
        let scroll = self.scroller.vadjustment().value();
        for (i, (b, light)) in self.nav_items.borrow().iter().enumerate() {
            if i == self.page.get() {
                b.add_css_class("on");
                light.set_opacity(1.0);
            } else {
                b.remove_css_class("on");
                light.set_opacity(0.0);
            }
        }
        let (page, name, _, lede) = PAGES[self.page.get()];
        let root = vbox(0, "settings-page");
        if page != Page::About {
            root.append(&label(name, "page-title"));
            let l = label(lede, "lede");
            l.set_wrap(true);
            l.set_margin_top(4);
            l.set_margin_bottom(4);
            root.append(&l);
        }
        let s = self.settings();
        match page {
            Page::General => self.general(&root, &s),
            Page::Appearance => self.appearance(&root, &s),
            Page::Search => self.search(&root, &s),
            Page::Files => self.files(&root, &s),
            Page::Clipboard => self.clipboard(&root, &s),
            Page::About => self.about(&root),
        }
        self.scroller.set_child(Some(&root));
        if keep_scroll {
            let adj = self.scroller.vadjustment();
            glib::idle_add_local_once(move || adj.set_value(scroll));
        } else {
            self.scroller.vadjustment().set_value(0.0);
        }
    }

    // ── Building blocks ─────────────────────────────────────────────────────

    fn group_title(root: &gtk::Box, text: &str) {
        let first = root.first_child().is_none() || root.observe_children().n_items() <= 2;
        let l = label(text, if first { "group-title first" } else { "group-title" });
        root.append(&l);
    }

    fn group() -> gtk::Box {
        vbox(0, "group")
    }

    fn field(group: &gtk::Box, title: &str, hint: Option<&str>, control: Option<&gtk::Widget>, disabled: bool) {
        let f = hbox(14, "field");
        if group.first_child().is_some() {
            f.add_css_class("separated");
        }
        if disabled {
            f.add_css_class("disabled");
        }
        let text = vbox(2, "label-box");
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        text.append(&label(title, "field-title"));
        if let Some(h) = hint.filter(|h| !h.is_empty()) {
            let l = label(h, "field-hint");
            l.set_wrap(true);
            l.set_max_width_chars(40);
            text.append(&l);
        }
        f.append(&text);
        if let Some(c) = control {
            let wrap = hbox(8, "control");
            wrap.set_valign(gtk::Align::Center);
            wrap.append(c);
            if disabled {
                wrap.set_sensitive(false);
            }
            f.append(&wrap);
        }
        group.append(&f);
    }

    fn switch(
        self: &Rc<Self>,
        group: &gtk::Box,
        title: &str,
        hint: Option<&str>,
        on: bool,
        disabled: bool,
        f: impl Fn(&mut Settings, bool) + 'static,
    ) {
        let sw = gtk::Switch::new();
        sw.set_active(on);
        sw.set_valign(gtk::Align::Center);
        let v = Rc::downgrade(self);
        let f = Rc::new(f);
        sw.connect_active_notify(move |sw| {
            let on = sw.is_active();
            let f = f.clone();
            if let Some(v) = v.upgrade() {
                v.change(move |s| f(s, on));
            }
        });
        Self::field(group, title, hint, Some(&sw.upcast()), disabled);
    }

    fn dropdown(self: &Rc<Self>, options: &[String], selected: usize, f: impl Fn(&mut Settings, usize) + 'static) -> gtk::Widget {
        let refs: Vec<&str> = options.iter().map(String::as_str).collect();
        let dd = gtk::DropDown::from_strings(&refs);
        dd.add_css_class("select");
        dd.set_selected(selected as u32);
        let v = Rc::downgrade(self);
        let f = Rc::new(f);
        dd.connect_selected_notify(move |dd| {
            let i = dd.selected() as usize;
            let f = f.clone();
            if let Some(v) = v.upgrade() {
                v.change(move |s| f(s, i));
            }
        });
        dd.upcast()
    }

    fn path_list(
        self: &Rc<Self>,
        items: &[String],
        placeholder: &str,
        icon: &'static str,
        set: impl Fn(&mut Settings, Vec<String>) + 'static,
    ) -> gtk::Widget {
        let list = vbox(0, "path-list");
        let set = Rc::new(set);
        if items.is_empty() {
            list.append(&label("Nothing here yet.", "none"));
        }
        for p in items {
            let row = hbox(10, "path-row");
            row.append(&glyph(icon, 15));
            row.append(&ellipsized(&tilde(p), ""));
            let rm = gtk::Button::new();
            rm.set_child(Some(&glyph("x", 14)));
            rm.set_tooltip_text(Some(&format!("Remove {p}")));
            let (v, all, p, set) = (Rc::downgrade(self), items.to_vec(), p.clone(), set.clone());
            rm.connect_clicked(move |_| {
                let next: Vec<String> = all.iter().filter(|x| **x != p).cloned().collect();
                let set = set.clone();
                if let Some(v) = v.upgrade() {
                    v.change(move |s| set(s, next));
                }
            });
            row.append(&rm);
            list.append(&row);
        }
        let add_row = hbox(8, "");
        add_row.set_margin_top(6);
        add_row.set_margin_start(4);
        add_row.set_margin_end(4);
        add_row.set_margin_bottom(4);
        let entry = gtk::Entry::new();
        entry.add_css_class("text-input");
        entry.add_css_class("mono");
        entry.set_placeholder_text(Some(placeholder));
        entry.set_hexpand(true);
        let add = button("Add", Some("plus"), None, "small");
        add.set_sensitive(false);
        let a = add.clone();
        entry.connect_changed(move |e| a.set_sensitive(!e.text().trim().is_empty()));
        let submit = {
            let (v, all, set, entry) = (Rc::downgrade(self), items.to_vec(), set.clone(), entry.clone());
            Rc::new(move || {
                let text = entry.text().trim().to_string();
                if text.is_empty() || all.contains(&text) {
                    entry.set_text("");
                    return;
                }
                let mut next = all.clone();
                next.push(text);
                let set = set.clone();
                if let Some(v) = v.upgrade() {
                    v.change(move |s| set(s, next));
                }
            })
        };
        let s1 = submit.clone();
        entry.connect_activate(move |_| s1());
        add.connect_clicked(move |_| submit());
        add_row.append(&entry);
        add_row.append(&add);
        list.append(&add_row);
        list.upcast()
    }

    fn info(&self) -> (usize, usize, usize) {
        match self.panel.upgrade().and_then(|p| p.app_handle()) {
            Some(app) => (app.core.files.count(), app.core.apps().len(), app.core.clip_count()),
            None => (0, 0, 0),
        }
    }

    // ── Pages ───────────────────────────────────────────────────────────────

    fn general(self: &Rc<Self>, root: &gtk::Box, s: &Settings) {
        Self::group_title(root, "Shortcut");
        let g = Self::group();
        let status = self.shortcut.borrow().clone();
        let (hint, control): (String, gtk::Widget) = match &status {
            Some(st) if st.configured => {
                let b = shortcut::pretty_binding(&st.binding);
                let c = hbox(8, "");
                c.append(&keys(&b));
                let rm = button("Remove", None, None, "small ghost");
                let v = Rc::downgrade(self);
                rm.connect_clicked(move |_| {
                    let v = v.clone();
                    glib::spawn_future_local(async move {
                        let Ok(st) = gio::spawn_blocking(shortcut::remove).await else { return };
                        if let Some(v) = v.upgrade() {
                            v.shortcut.replace(Some(st));
                            v.render(true);
                        }
                    });
                });
                c.append(&rm);
                (format!("Press {b} anywhere to open Lumen."), c.upcast())
            }
            other => {
                let hint = match other {
                    Some(st) if !st.supported => format!("Bind a key to “{}” in your desktop’s keyboard settings.", st.command),
                    Some(_) => "Not set up yet.".into(),
                    None => String::new(),
                };
                let b = button("Set up", Some("keyboard"), None, "small primary");
                let p = self.panel.clone();
                b.connect_clicked(move |_| {
                    if let Some(p) = p.upgrade() {
                        p.show_shortcut_view();
                    }
                });
                (hint, b.upcast())
            }
        };
        Self::field(&g, "Open Lumen", Some(&hint), Some(&control), false);
        root.append(&g);

        Self::group_title(root, "Startup");
        let g = Self::group();
        self.switch(
            &g,
            "Launch at login",
            Some("Start quietly in the background so the shortcut opens instantly."),
            s.launch_at_login,
            false,
            |s, v| s.launch_at_login = v,
        );
        self.switch(
            &g,
            "Free memory when idle",
            Some("Release the panel after a minute hidden. Opening it again takes a moment longer."),
            s.free_memory_when_idle,
            false,
            |s, v| s.free_memory_when_idle = v,
        );
        root.append(&g);

        Self::group_title(root, "Terminal");
        let g = Self::group();
        let mut choices: Vec<String> = TERMINAL_CHOICES.iter().map(|t| if *t == "auto" { "Automatic".to_string() } else { t.to_string() }).collect();
        let mut values: Vec<String> = TERMINAL_CHOICES.iter().map(|t| t.to_string()).collect();
        if !values.contains(&s.terminal) {
            choices.push(s.terminal.clone());
            values.push(s.terminal.clone());
        }
        let sel = values.iter().position(|t| *t == s.terminal).unwrap_or(0);
        let dd = self.dropdown(&choices, sel, move |s, i| s.terminal = values[i].clone());
        Self::field(&g, "Terminal app", Some("Used for “Run in terminal” and terminal apps."), Some(&dd), false);
        root.append(&g);
    }

    fn appearance(self: &Rc<Self>, root: &gtk::Box, s: &Settings) {
        Self::group_title(root, "Look");
        let g = Self::group();
        let seg = hbox(2, "segmented");
        for (mode, name, icon) in [(ThemeMode::System, "Auto", "monitor"), (ThemeMode::Light, "Light", "sun"), (ThemeMode::Dark, "Dark", "moon")] {
            let b = gtk::Button::new();
            b.set_focus_on_click(false);
            let c = hbox(6, "");
            c.append(&glyph(icon, 14));
            c.append(&gtk::Label::new(Some(name)));
            b.set_child(Some(&c));
            if s.theme == mode {
                b.add_css_class("on");
            }
            let v = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(v) = v.upgrade() {
                    v.change(move |s| s.theme = mode);
                }
            });
            seg.append(&b);
        }
        Self::field(&g, "Theme", (s.theme == ThemeMode::System).then_some("Follows your desktop’s light or dark style."), Some(&seg.upcast()), false);

        let swatches = hbox(10, "swatches");
        for (name, value) in ACCENTS {
            let b = gtk::Button::new();
            b.add_css_class("swatch");
            b.add_css_class(&format!("c-{}", &value[1..]));
            b.set_tooltip_text(Some(name));
            b.set_focus_on_click(false);
            let check = glyph("check", 12).with_stroke(3.0);
            check.set_opacity(if s.accent == *value { 1.0 } else { 0.0 });
            b.set_child(Some(&check));
            if s.accent == *value {
                b.add_css_class("on");
            }
            let v = Rc::downgrade(self);
            let value = value.to_string();
            b.connect_clicked(move |_| {
                let value = value.clone();
                if let Some(v) = v.upgrade() {
                    v.change(move |s| s.accent = value);
                }
            });
            swatches.append(&b);
        }
        let accent_name = ACCENTS.iter().find(|(_, v)| *v == s.accent).map_or("Custom", |(n, _)| n);
        Self::field(&g, "Accent colour", Some(accent_name), Some(&swatches.upcast()), false);

        let blur = self.panel.upgrade().and_then(|p| p.app_handle()).is_some_and(|a| a.is_shell_blur());
        let (hint, control): (&str, gtk::Widget) = if blur {
            let b = hbox(5, "badge success");
            b.set_valign(gtk::Align::Center);
            b.append(&glyph("circle-check", 13));
            b.append(&gtk::Label::new(Some("Active")));
            ("Real blur is on, drawn by the Lumen Glass GNOME extension.", b.upcast())
        } else {
            let b = button("Learn more", None, None, "small ghost");
            if let Some(c) = b.child().and_downcast::<gtk::Box>() {
                c.append(&glyph("external-link", 15));
            }
            let p = self.panel.clone();
            b.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.open_url_and_hide(&format!("{REPO_URL}#gnome-extension-for-real-blur-optional"));
                }
            });
            ("Frosted tint. Install the Lumen Glass GNOME extension for real blur.", b.upcast())
        };
        Self::field(&g, "Background blur", Some(hint), Some(&control), false);
        root.append(&g);

        Self::group_title(root, "Layout");
        let g = Self::group();
        self.switch(&g, "Preview pane", Some("Show details for the selected result beside the list."), s.show_preview, false, |s, v| {
            s.show_preview = v
        });
        root.append(&g);
    }

    fn search(self: &Rc<Self>, root: &gtk::Box, s: &Settings) {
        Self::group_title(root, "Web");
        let g = Self::group();
        let names: Vec<String> = SearchEngine::ALL.iter().map(|e| e.label().to_string()).collect();
        let sel = SearchEngine::ALL.iter().position(|e| *e == s.search_engine).unwrap_or(0);
        let dd = self.dropdown(&names, sel, |s, i| s.search_engine = SearchEngine::ALL[i]);
        Self::field(&g, "Search engine", Some("Used when a query isn’t an app, file or link."), Some(&dd), false);
        if s.search_engine == SearchEngine::Custom {
            let e = gtk::Entry::new();
            e.add_css_class("text-input");
            e.set_text(&s.custom_search_url);
            e.set_width_chars(30);
            let v = Rc::downgrade(self);
            let save = move |e: &gtk::Entry| {
                let url = e.text().to_string();
                if let Some(v) = v.upgrade()
                    && v.settings().custom_search_url != url
                {
                    v.change(move |s| s.custom_search_url = url);
                }
            };
            let save2 = save.clone();
            e.connect_activate(move |e| save2(e));
            let focus = gtk::EventControllerFocus::new();
            let er = e.clone();
            focus.connect_leave(move |_| save(&er));
            e.add_controller(focus);
            Self::field(&g, "Custom URL", Some("Put %s where the search text goes."), Some(&e.upcast()), false);
        }
        root.append(&g);

        Self::group_title(root, "Calculator");
        let g = Self::group();
        self.switch(
            &g,
            "Currency conversion",
            Some("“100 usd to eur”. Downloads exchange rates once a day."),
            s.currency_conversion,
            false,
            |s, v| s.currency_conversion = v,
        );
        root.append(&g);
    }

    fn files(self: &Rc<Self>, root: &gtk::Box, s: &Settings) {
        let (files, _, _) = self.info();
        Self::group_title(root, "Index");
        let g = Self::group();
        let hint = format!("{} items indexed.", count(files));
        self.switch(&g, "Search files and folders", Some(&hint), s.file_search, false, |s, v| s.file_search = v);
        self.switch(&g, "Include hidden files", Some("Dotfiles and folders like ~/.config."), s.include_hidden, !s.file_search, |s, v| {
            s.include_hidden = v
        });
        let rebuild = button("Rebuild", Some("refresh-cw"), None, "small");
        let p = self.panel.clone();
        rebuild.connect_clicked(move |_| {
            if let Some(p) = p.upgrade()
                && let Some(app) = p.app_handle()
            {
                app.core.reindex();
                p.flash("Rebuilding file index…", false);
            }
        });
        Self::field(&g, "Rebuild index", Some("Re-scan now instead of waiting for changes to be noticed."), Some(&rebuild.upcast()), !s.file_search);
        root.append(&g);

        Self::group_title(root, "Folders to search");
        let g = Self::group();
        g.append(&self.path_list(&s.file_roots, "/path/to/folder", "folder", |s, v| s.file_roots = v));
        if !s.file_search {
            g.add_css_class("dimmed");
            g.set_sensitive(false);
        }
        root.append(&g);

        Self::group_title(root, "Skipped folders");
        let g = Self::group();
        g.append(&self.path_list(&s.exclude_dirs, "Folder name or path, e.g. node_modules", "folder-x", |s, v| s.exclude_dirs = v));
        if !s.file_search {
            g.add_css_class("dimmed");
            g.set_sensitive(false);
        }
        root.append(&g);
    }

    fn clipboard(self: &Rc<Self>, root: &gtk::Box, s: &Settings) {
        let (_, _, clips) = self.info();
        Self::group_title(root, "History");
        let g = Self::group();
        let hint = format!("Type “;” to search and paste. {} saved.", plural(clips, "item", "items"));
        self.switch(&g, "Clipboard history", Some(&hint), s.clipboard_enabled, false, |s, v| s.clipboard_enabled = v);
        self.switch(
            &g,
            "Pause recording",
            Some("Stop saving new copies for now. Existing history stays searchable."),
            s.clipboard_paused,
            !s.clipboard_enabled,
            |s, v| s.clipboard_paused = v,
        );
        let mut sizes: Vec<u32> = CLIP_SIZES.to_vec();
        if !sizes.contains(&s.clipboard_max) {
            sizes.push(s.clipboard_max);
            sizes.sort();
        }
        let names: Vec<String> = sizes.iter().map(|n| format!("{} items", count(*n as usize))).collect();
        let sel = sizes.iter().position(|n| *n == s.clipboard_max).unwrap_or(0);
        let dd = self.dropdown(&names, sel, move |s, i| s.clipboard_max = sizes[i]);
        Self::field(&g, "History size", Some("Older clips drop off. Pinned clips are always kept."), Some(&dd), !s.clipboard_enabled);
        root.append(&g);

        Self::group_title(root, "Privacy");
        let g = Self::group();
        self.switch(
            &g,
            "Keep history after restart",
            Some("Saved in Lumen’s settings folder, readable only by you. Off keeps it in memory only."),
            s.clipboard_persist,
            false,
            |s, v| s.clipboard_persist = v,
        );
        self.switch(&g, "Skip passwords", Some("Ignore copies that password managers mark as secret."), s.clipboard_ignore_secrets, false, |s, v| {
            s.clipboard_ignore_secrets = v
        });
        let confirming = self.confirm_clear.get();
        let clear = button(if confirming { "Confirm" } else { "Clear" }, Some("trash-2"), None, if confirming { "small primary" } else { "small" });
        let v = Rc::downgrade(self);
        clear.connect_clicked(move |_| {
            let Some(v) = v.upgrade() else { return };
            if let Some(t) = v.clear_timer.take() {
                t.remove();
            }
            if !v.confirm_clear.get() {
                // The second click must come soon after the first.
                v.confirm_clear.set(true);
                let w = Rc::downgrade(&v);
                let id = glib::timeout_add_local_once(std::time::Duration::from_secs(3), move || {
                    if let Some(v) = w.upgrade() {
                        v.clear_timer.replace(None);
                        v.confirm_clear.set(false);
                        v.render(true);
                    }
                });
                v.clear_timer.replace(Some(id));
                v.render(true);
                return;
            }
            v.confirm_clear.set(false);
            if let Some(p) = v.panel.upgrade()
                && let Some(app) = p.app_handle()
            {
                let r = app.execute(&lumen_core::types::Action::ClipClear, None);
                p.flash(r.toast.as_deref().unwrap_or("Clipboard history cleared"), false);
            }
            v.render(true);
        });
        Self::field(
            &g,
            "Clear history",
            Some(if confirming { "Click again to clear. Pinned clips stay." } else { "Forget everything except pinned clips." }),
            Some(&clear.upcast()),
            false,
        );
        root.append(&g);
    }

    fn about(self: &Rc<Self>, root: &gtk::Box) {
        let (files, apps, _) = self.info();
        let head = vbox(6, "");
        head.set_margin_top(16);
        head.set_margin_bottom(20);
        let m = mark(64);
        m.set_halign(gtk::Align::Center);
        head.append(&m);
        let t = label(APP_NAME, "about-title");
        t.set_xalign(0.5);
        t.set_margin_top(8);
        head.append(&t);
        let v = label(&format!("Version {VERSION}"), "about-text");
        v.set_xalign(0.5);
        head.append(&v);
        root.append(&head);

        let stats = hbox(0, "group stats");
        stats.set_homogeneous(true);
        let desktop = lumen_core::env::desktop_name();
        let desktop = desktop.strip_prefix("ubuntu:").or(desktop.strip_prefix("Ubuntu:")).unwrap_or(&desktop).to_string();
        for (i, (value, name)) in
            [(count(apps), "Apps".to_string()), (count(files), "Files indexed".to_string()), (lumen_core::env::session_type(), desktop)]
                .iter()
                .enumerate()
        {
            let s = vbox(0, "stat");
            if i > 0 {
                s.add_css_class("separated");
            }
            s.append(&label(value, "stat-value"));
            s.append(&ellipsized(name, "stat-label"));
            stats.append(&s);
        }
        root.append(&stats);

        let g = Self::group();
        g.set_margin_top(12);
        let open = button("Open", None, None, "small ghost");
        if let Some(c) = open.child().and_downcast::<gtk::Box>() {
            c.append(&glyph("external-link", 15));
        }
        let p = self.panel.clone();
        open.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.open_url_and_hide(REPO_URL);
            }
        });
        Self::field(&g, "Source code and issues", Some("github.com/smith8h/lumen"), Some(&open.upcast()), false);
        root.append(&g);
    }
}
