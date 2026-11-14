//! The panel: search bar, results, preview, footer, action menu and toasts, plus the
//! settings, welcome and command-output views.

pub mod bits;
mod glyph_paths;
mod menu;
mod output;
mod preview;
mod results;
mod settings;
mod suggestions;
mod welcome;
pub mod widgets;

use gtk::prelude::*;
use gtk::{gdk, gio, glib, graphene};
use lumen_core::store::Settings;
use lumen_core::types::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use crate::app::{App, ShowOpts, View};
use bits::{glyph, hbox, keys, label, mark, vbox};
use output::{OutputView, RunState};
use results::{ResultList, RowEvents};
use suggestions::Suggestions;
use widgets::{AutoHeight, Glyph as GlyphWidget, Stage};

pub fn settings_accents() -> &'static [(&'static str, &'static str)] {
    settings::ACCENTS
}

/// Results never get taller than this before scrolling.
pub const BODY_MAX: i32 = 420;

struct State {
    open: bool,
    view: View,
    query: String,
    response: Option<SearchResponse>,
    suggested: Vec<ResultSection>,
    selected: usize,
    menu: Option<usize>,
    confirm_id: Option<String>,
    run: Option<RunState>,
    seq: u64,
    preview_cache: HashMap<String, Preview>,
    keep_selection: Option<String>,
    settings: Settings,
    /// Ignore the entry's change signal while Lumen sets its text.
    setting_text: bool,
}

pub struct Panel {
    app: Weak<App>,
    pub window: gtk::Window,
    stage: Stage,
    overlay: gtk::Overlay,
    searchbar: gtk::Box,
    lead: GlyphWidget,
    entry: gtk::Text,
    chip: gtk::Box,
    chip_glyph: GlyphWidget,
    chip_label: gtk::Label,
    divider: gtk::Box,
    body: AutoHeight,
    footer: gtk::Box,
    menu_widget: RefCell<Option<gtk::Widget>>,
    toast: RefCell<Option<(gtk::Widget, glib::SourceId)>>,
    st: RefCell<State>,
    list: RefCell<Option<ResultList>>,
    grid: RefCell<Option<Suggestions>>,
    preview_slot: RefCell<Option<gtk::ScrolledWindow>>,
    preview_timer: RefCell<Option<glib::SourceId>>,
    output: RefCell<Option<Rc<OutputView>>>,
    pointer: Cell<Option<(f64, f64)>>,
    run_token: Cell<u64>,
}

fn greeting() -> String {
    let h = glib::DateTime::now_local().map(|d| d.hour()).unwrap_or(12);
    let part = match h {
        0..=4 => "Still up",
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    };
    format!("{part}. What do you need?")
}

/// The label for Enter on a result.
pub fn primary_action(item: &ResultItem) -> SecondaryAction {
    let label = match item.kind {
        ResultKind::App | ResultKind::File => "Open",
        ResultKind::Folder => "Open folder",
        ResultKind::Calc => "Copy result",
        ResultKind::Url => "Open link",
        ResultKind::Web => "Search",
        ResultKind::Command => {
            if matches!(item.action, Action::RunCommand { mode: RunMode::Inline, .. }) {
                "Run and show output"
            } else {
                "Run in terminal"
            }
        }
        ResultKind::System => "Run",
        ResultKind::Lumen => {
            if item.action == Action::ClipClear {
                "Run"
            } else {
                "Open"
            }
        }
        ResultKind::Clip => "Paste",
    };
    let mut a = SecondaryAction::new(label, item.action.clone()).hint("Enter");
    if item.kind == ResultKind::Clip {
        a = a.glyph(Glyph::Paste);
    }
    a
}

/// "Ctrl+Shift+P" for a key press, matching SecondaryAction hints.
fn combo_of(key: gdk::Key, mods: gdk::ModifierType) -> String {
    let name = match key {
        gdk::Key::Return | gdk::Key::KP_Enter => "Enter".to_string(),
        gdk::Key::Escape => "Escape".into(),
        _ => match key.to_unicode() {
            Some(c) if !c.is_control() => c.to_uppercase().to_string(),
            _ => key.name().map(|n| n.to_string()).unwrap_or_default(),
        },
    };
    let mut parts = Vec::new();
    if mods.contains(gdk::ModifierType::CONTROL_MASK) || mods.contains(gdk::ModifierType::META_MASK) {
        parts.push("Ctrl".to_string());
    }
    if mods.contains(gdk::ModifierType::ALT_MASK) {
        parts.push("Alt".into());
    }
    if mods.contains(gdk::ModifierType::SHIFT_MASK) {
        parts.push("Shift".into());
    }
    parts.push(name);
    parts.join("+")
}

impl Panel {
    pub fn new(app: &Rc<App>) -> Rc<Panel> {
        let window = gtk::Window::builder().application(&app.gtk).title(lumen_core::constants::APP_NAME).decorated(false).build();
        window.add_css_class("lumen");
        window.set_icon_name(Some(lumen_core::constants::APP_ID));
        if let Some(mon) = gdk::Display::default().and_then(|d| d.monitors().item(0)).and_then(|m| m.downcast::<gdk::Monitor>().ok()) {
            let g = mon.geometry();
            window.set_default_size(g.width(), g.height());
        }
        window.maximize();

        // Search bar
        let searchbar = hbox(12, "searchbar");
        let lead = glyph("search", 22);
        lead.add_css_class("lead");
        lead.set_valign(gtk::Align::Center);
        let entry = gtk::Text::new();
        entry.add_css_class("search");
        entry.set_hexpand(true);
        entry.set_placeholder_text(Some(&greeting()));
        entry.set_valign(gtk::Align::Fill);
        let chip = hbox(6, "chip");
        chip.set_valign(gtk::Align::Center);
        let chip_glyph = glyph("search", 13);
        let chip_label = gtk::Label::new(None);
        chip.append(&chip_glyph);
        chip.append(&chip_label);
        chip.set_visible(false);
        let settings_btn = gtk::Button::new();
        settings_btn.add_css_class("icon-btn");
        settings_btn.set_child(Some(&glyph("settings-2", 18)));
        settings_btn.set_valign(gtk::Align::Center);
        settings_btn.set_tooltip_text(Some("Settings (Ctrl+,)"));
        settings_btn.set_focus_on_click(false);
        searchbar.append(&lead);
        searchbar.append(&entry);
        searchbar.append(&chip);
        searchbar.append(&settings_btn);

        let divider = hbox(0, "divider hidden");
        let body = AutoHeight::new();
        let footer = hbox(16, "footer");

        let main = vbox(0, "");
        main.append(&searchbar);
        main.append(&divider);
        main.append(&body);
        main.append(&footer);

        let overlay = gtk::Overlay::new();
        overlay.add_css_class("panel");
        overlay.set_overflow(gtk::Overflow::Hidden);
        overlay.set_child(Some(&main));

        let stage = Stage::new(&overlay);
        window.set_child(Some(&stage));

        let p = Rc::new(Panel {
            app: Rc::downgrade(app),
            window,
            stage,
            overlay,
            searchbar,
            lead,
            entry,
            chip,
            chip_glyph,
            chip_label,
            divider,
            body,
            footer,
            menu_widget: RefCell::new(None),
            toast: RefCell::new(None),
            st: RefCell::new(State {
                open: false,
                view: View::Search,
                query: String::new(),
                response: None,
                suggested: Vec::new(),
                selected: 0,
                menu: None,
                confirm_id: None,
                run: None,
                seq: 0,
                preview_cache: HashMap::new(),
                keep_selection: None,
                settings: app.core.settings(),
                setting_text: false,
            }),
            list: RefCell::new(None),
            grid: RefCell::new(None),
            preview_slot: RefCell::new(None),
            preview_timer: RefCell::new(None),
            output: RefCell::new(None),
            pointer: Cell::new(None),
            run_token: Cell::new(0),
        });
        p.wire(&settings_btn);
        p
    }

    fn wire(self: &Rc<Self>, settings_btn: &gtk::Button) {
        let weak = Rc::downgrade(self);
        self.entry.connect_changed(move |e| {
            let Some(p) = weak.upgrade() else { return };
            if p.st.borrow().setting_text {
                return;
            }
            p.st.borrow_mut().query = e.text().to_string();
            p.run_search();
        });

        let weak = Rc::downgrade(self);
        settings_btn.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.set_view(View::Settings);
            }
        });

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, mods| match weak.upgrade() {
            Some(p) if p.handle_key(key, mods) => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        });
        self.window.add_controller(keys);

        // A click on the transparent area around the panel closes Lumen.
        let click = gtk::GestureClick::new();
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |_, _, x, y| {
            let Some(p) = weak.upgrade() else { return };
            let r = p.stage.panel_rect();
            let inside = x >= r.x as f64 && x <= (r.x + r.width) as f64 && y >= r.y as f64 && y <= (r.y + r.height) as f64;
            if !inside {
                if let Some(app) = p.app.upgrade() {
                    app.hide(true);
                }
            } else if p.st.borrow().menu.is_some() && !p.point_in_menu(x, y) {
                p.close_menu();
            }
        });
        self.stage.add_controller(click);

        let weak = Rc::downgrade(self);
        self.window.connect_is_active_notify(move |w| {
            if w.is_active() {
                return;
            }
            if let Some(p) = weak.upgrade()
                && p.is_open()
                && let Some(app) = p.app.upgrade()
            {
                app.focus_lost();
            }
        });

        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            // Closing (e.g. Alt+F4) just hides; Lumen keeps running for the next Alt+Space.
            if let Some(p) = weak.upgrade()
                && let Some(app) = p.app.upgrade()
            {
                app.hide(false);
            }
            glib::Propagation::Stop
        });
    }

    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    pub fn is_open(&self) -> bool {
        self.st.borrow().open
    }

    fn app(&self) -> Option<Rc<App>> {
        self.app.upgrade()
    }

    /// Whether the pointer really moved since the last check (the panel often opens under a
    /// resting cursor, and that must not steal the keyboard selection).
    pub fn pointer_moved(&self, w: &gtk::Widget, x: f64, y: f64) -> bool {
        let Some(pt) = w.compute_point(&self.window, &graphene::Point::new(x as f32, y as f32)) else { return false };
        let cur = (pt.x() as f64, pt.y() as f64);
        let moved = self.pointer.get().is_some_and(|last| (last.0 - cur.0).abs() > 0.5 || (last.1 - cur.1).abs() > 0.5);
        self.pointer.set(Some(cur));
        moved
    }

    fn row_events(self: &Rc<Self>) -> RowEvents {
        let (a, b, c) = (Rc::downgrade(self), Rc::downgrade(self), Rc::downgrade(self));
        RowEvents {
            hover: Rc::new(move |i| {
                if let Some(p) = a.upgrade()
                    && p.st.borrow().selected != i
                {
                    p.select(i);
                }
            }),
            run: Rc::new(move |i| {
                if let Some(p) = b.upgrade() {
                    p.select(i);
                    if let Some(item) = p.items().get(i).cloned() {
                        p.perform(&item, None);
                    }
                }
            }),
            moved: Rc::new(move |w, x, y| c.upgrade().is_some_and(|p| p.pointer_moved(w, x, y))),
        }
    }

    // ── Show / hide ─────────────────────────────────────────────────────────

    pub fn on_show(self: &Rc<Self>, opts: ShowOpts) {
        self.body.set_animate(false);
        {
            let mut st = self.st.borrow_mut();
            st.open = true;
            st.menu = None;
            st.confirm_id = None;
            st.preview_cache.clear();
            // Alt+Space always lands on search, like Spotlight.
            st.view = opts.view.unwrap_or(View::Search);
            if st.run.as_ref().is_some_and(|r| r.done) {
                st.run = None;
            }
        }
        self.pointer.set(None);
        self.close_menu_widget();
        self.entry.set_placeholder_text(Some(&greeting()));
        if let Some(q) = opts.query {
            self.set_query(&q);
        }
        self.overlay.add_css_class("open");
        self.load_suggestions();
        // Re-run a kept query: results like clipboard history may have changed while hidden.
        self.run_search();
        self.render_chrome();
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(p) = weak.upgrade() {
                p.body.set_animate(true);
            }
        });
    }

    pub fn on_hide(&self) {
        let mut st = self.st.borrow_mut();
        st.open = false;
        st.menu = None;
        drop(st);
        self.overlay.remove_css_class("open");
        self.close_menu_widget();
        self.clear_toast();
    }

    pub fn on_settings(self: &Rc<Self>, s: &Settings) {
        let preview_changed = self.st.borrow().settings.show_preview != s.show_preview;
        self.st.borrow_mut().settings = s.clone();
        if preview_changed && self.st.borrow().view == View::Search {
            self.render_body();
        }
    }

    pub fn on_shell_blur(&self, _on: bool) {}

    fn set_query(&self, q: &str) {
        self.st.borrow_mut().setting_text = true;
        self.entry.set_text(q);
        self.entry.set_position(-1);
        let mut st = self.st.borrow_mut();
        st.setting_text = false;
        st.query = q.to_string();
    }

    fn set_view(self: &Rc<Self>, view: View) {
        self.close_menu();
        self.st.borrow_mut().view = view;
        self.render_body();
        self.render_chrome();
        if view == View::Search {
            self.entry.grab_focus();
        }
    }

    fn load_suggestions(self: &Rc<Self>) {
        let Some(app) = self.app() else { return };
        let core = app.core.clone();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(s) = gio::spawn_blocking(move || core.suggestions()).await else { return };
            let Some(p) = weak.upgrade() else { return };
            let changed = p.st.borrow().suggested != s;
            p.st.borrow_mut().suggested = s;
            if changed && p.st.borrow().query.trim().is_empty() && p.st.borrow().view == View::Search {
                p.render_body();
            }
        });
    }

    // ── Search ──────────────────────────────────────────────────────────────

    fn run_search(self: &Rc<Self>) {
        let (seq, query, keep) = {
            let mut st = self.st.borrow_mut();
            st.seq += 1;
            st.confirm_id = None;
            st.menu = None;
            (st.seq, st.query.clone(), st.keep_selection.take())
        };
        self.close_menu_widget();
        if query.trim().is_empty() {
            {
                let mut st = self.st.borrow_mut();
                st.response = None;
                st.selected = 0;
            }
            self.render_body();
            self.render_chrome();
            return;
        }
        let Some(app) = self.app() else { return };
        let core = app.core.clone();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let q = query.clone();
            let Ok(r) = gio::spawn_blocking(move || core.search(&q, seq)).await else { return };
            let Some(p) = weak.upgrade() else { return };
            if p.st.borrow().seq != r.seq {
                return;
            }
            {
                let mut st = p.st.borrow_mut();
                let all: Vec<&ResultItem> = r.sections.iter().flat_map(|s| s.items.iter()).collect();
                let same = keep.as_ref().and_then(|k| all.iter().position(|i| &i.id == k));
                st.selected = match (same, &keep) {
                    (Some(i), _) => i,
                    (None, Some(_)) => st.selected.min(all.len().saturating_sub(1)),
                    _ => 0,
                };
                st.response = Some(r);
            }
            p.render_body();
            p.render_chrome();
        });
    }

    fn sections(&self) -> Vec<ResultSection> {
        let st = self.st.borrow();
        if st.query.trim().is_empty() { st.suggested.clone() } else { st.response.as_ref().map(|r| r.sections.clone()).unwrap_or_default() }
    }

    fn items(&self) -> Vec<ResultItem> {
        self.sections().into_iter().flat_map(|s| s.items).collect()
    }

    fn current(&self) -> Option<ResultItem> {
        let sel = self.st.borrow().selected;
        self.items().into_iter().nth(sel)
    }

    fn grid_count(&self) -> usize {
        let st = self.st.borrow();
        if st.query.trim().is_empty() && st.suggested.first().is_some_and(|s| s.id == SectionId::Apps) { st.suggested[0].items.len() } else { 0 }
    }

    fn show_preview(&self) -> bool {
        let st = self.st.borrow();
        st.settings.show_preview
            && !st.query.trim().is_empty()
            && st.view == View::Search
            && st.run.is_none()
            && st.response.as_ref().is_some_and(|r| r.sections.iter().any(|s| !s.items.is_empty()))
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    fn render_body(self: &Rc<Self>) {
        self.list.replace(None);
        self.grid.replace(None);
        self.preview_slot.replace(None);
        if let Some(t) = self.preview_timer.take() {
            t.remove();
        }
        let (view, has_run, query, has_response) = {
            let st = self.st.borrow();
            (st.view, st.run.is_some(), st.query.trim().to_string(), st.response.is_some())
        };
        if view != View::Search || !has_run {
            self.output.replace(None);
        }
        let widget: Option<gtk::Widget> = match view {
            View::Settings => Some(settings::SettingsView::build(self)),
            View::Shortcut => Some(welcome::build(self)),
            View::Search if has_run => self.output.borrow().as_ref().map(|o| o.root.clone()),
            View::Search if query.is_empty() => {
                let sections = self.st.borrow().suggested.clone();
                let s = Suggestions::new(&sections, &self.row_events(), Rc::downgrade(self));
                let root = s.root.clone();
                self.grid.replace(Some(s));
                Some(root)
            }
            View::Search => {
                let sections = self.sections();
                if sections.iter().any(|s| !s.items.is_empty()) {
                    let split = hbox(0, "split");
                    let list = ResultList::new(&sections, 0, BODY_MAX, &self.row_events());
                    list.root.set_hexpand(true);
                    split.append(&list.root);
                    if self.show_preview() {
                        list.root.set_hexpand(false);
                        list.root.set_size_request(436, -1);
                        let slot = gtk::ScrolledWindow::new();
                        slot.set_policy(gtk::PolicyType::Never, gtk::PolicyType::External);
                        slot.set_propagate_natural_height(true);
                        slot.set_max_content_height(BODY_MAX);
                        slot.set_hexpand(true);
                        slot.set_size_request(-1, -1);
                        split.append(&slot);
                        self.preview_slot.replace(Some(slot));
                    }
                    self.list.replace(Some(list));
                    Some(split.upcast())
                } else if has_response {
                    let empty = vbox(6, "empty");
                    let g = glyph("search", 28);
                    g.set_opacity(0.6);
                    g.set_halign(gtk::Align::Center);
                    empty.append(&g);
                    let l = label(&format!("No results for “{query}”"), "");
                    l.set_xalign(0.5);
                    empty.append(&l);
                    Some(empty.upcast())
                } else {
                    None
                }
            }
        };
        self.divider.set_css_classes(if widget.is_some() { &["divider"] } else { &["divider", "hidden"] });
        self.body.set_child(widget.as_ref());
        self.update_selection();
    }

    /// Search bar, mode chip and footer.
    fn render_chrome(self: &Rc<Self>) {
        let (view, query, mode, has_run) = {
            let st = self.st.borrow();
            let mode = if st.query.trim().is_empty() { None } else { st.response.as_ref().and_then(|r| r.mode.clone()) };
            (st.view, st.query.clone(), mode, st.run.is_some())
        };
        let search = view == View::Search;
        self.searchbar.set_visible(search);
        if !search {
            self.divider.set_visible(false);
        } else {
            self.divider.set_visible(true);
        }
        match &mode {
            Some(m) => {
                let g = bits::lucide(m.glyph);
                self.lead.set_name(g);
                self.lead.add_css_class("active");
                self.chip_glyph.set_name(g);
                if self.chip_label.text() != m.label || !self.chip.is_visible() {
                    self.chip_label.set_text(&m.label);
                    // Restart the pop animation.
                    self.chip.set_visible(false);
                    self.chip.set_visible(true);
                }
            }
            None => {
                self.lead.set_name("search");
                self.lead.remove_css_class("active");
                self.chip.set_visible(false);
            }
        }
        let _ = (query, has_run);
        self.render_footer();
    }

    fn render_footer(self: &Rc<Self>) {
        while let Some(c) = self.footer.first_child() {
            self.footer.remove(&c);
        }
        let current = self.current();
        let st = self.st.borrow();
        let has_body = self.body.first_child().is_some();
        let visible = st.view == View::Search && (has_body || current.is_some());
        self.footer.set_visible(visible);
        if !visible {
            return;
        }
        let brand = hbox(8, "brand");
        brand.set_hexpand(true);
        let confirm = current.as_ref().filter(|c| st.confirm_id.as_deref() == Some(c.id.as_str())).and_then(|c| c.confirm.clone());
        if let Some(text) = confirm {
            let bar = hbox(8, "confirm-bar");
            bar.append(&glyph("triangle-alert", 15));
            bar.append(&gtk::Label::new(Some(&text)));
            brand.append(&bar);
        } else {
            brand.append(&mark(16));
            let q = st.query.trim();
            if !q.is_empty() && st.run.is_none() {
                if let Some(r) = &st.response {
                    let n: usize = r.sections.iter().map(|s| s.items.len()).sum();
                    let l = bits::ellipsized(&format!("{} in {} ms", lumen_core::format::plural(n, "result", "results"), r.took_ms), "status");
                    brand.append(&l);
                }
            } else if q.is_empty()
                && let Some(c) = &current
            {
                brand.append(&bits::ellipsized(&c.title, "status current"));
            } else {
                brand.append(&gtk::Label::new(Some(lumen_core::constants::APP_NAME)));
            }
        }
        self.footer.append(&brand);
        let hints: Vec<(&str, String)> = if st.run.is_some() {
            vec![]
        } else {
            match &current {
                None => vec![("esc", "Close".into())],
                Some(c) => match c.kind {
                    ResultKind::File | ResultKind::Folder => {
                        vec![("Enter", "Open".into()), ("Ctrl+Enter", "Show in Files".into()), ("Ctrl+K", "Actions".into())]
                    }
                    ResultKind::Command => vec![("Enter", "Run".into()), ("Ctrl+K", "Actions".into())],
                    ResultKind::Calc => vec![("Enter", "Copy".into())],
                    ResultKind::Clip => vec![("Enter", "Paste".into()), ("Ctrl+Enter", "Copy".into()), ("Ctrl+K", "Actions".into())],
                    _ if !c.actions.is_empty() => vec![("Enter", primary_action(c).label), ("Ctrl+K", "Actions".into())],
                    _ => vec![("Enter", primary_action(c).label)],
                },
            }
        };
        for (k, text) in hints {
            let h = hbox(6, "hint");
            h.append(&gtk::Label::new(Some(&text)));
            h.append(&keys(k));
            self.footer.append(&h);
        }
    }

    // ── Selection and preview ───────────────────────────────────────────────

    fn select(self: &Rc<Self>, i: usize) {
        {
            let mut st = self.st.borrow_mut();
            st.selected = i;
            st.confirm_id = None;
        }
        self.update_selection();
        self.render_footer();
    }

    fn update_selection(self: &Rc<Self>) {
        let sel = self.st.borrow().selected;
        if let Some(l) = &*self.list.borrow() {
            l.set_selected(Some(sel));
        }
        if let Some(g) = &*self.grid.borrow() {
            g.set_selected(sel);
        }
        self.schedule_preview();
    }

    fn schedule_preview(self: &Rc<Self>) {
        if let Some(t) = self.preview_timer.take() {
            t.remove();
        }
        let Some(slot) = self.preview_slot.borrow().clone() else { return };
        let Some(item) = self.current() else { return };
        let cached = self.st.borrow().preview_cache.get(&item.id).cloned();
        if let Some(p) = cached {
            slot.set_child(Some(&preview::pane(&item, Some(&p))));
            return;
        }
        // Show the generic pane right away only if nothing is there yet, to avoid flicker.
        if slot.child().is_none() {
            slot.set_child(Some(&preview::pane(&item, None)));
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(40), move || {
            let Some(p) = weak.upgrade() else { return };
            p.preview_timer.replace(None);
            let Some(app) = p.app() else { return };
            let core = app.core.clone();
            let it = item.clone();
            glib::spawn_future_local(async move {
                let Ok(data) = gio::spawn_blocking(move || core.preview(&it)).await else { return };
                let Some(p) = weak.upgrade() else { return };
                p.st.borrow_mut().preview_cache.insert(item.id.clone(), data.clone());
                if p.current().is_some_and(|c| c.id == item.id)
                    && let Some(slot) = p.preview_slot.borrow().clone()
                {
                    slot.set_child(Some(&preview::pane(&item, Some(&data))));
                }
            });
        });
        self.preview_timer.replace(Some(id));
    }

    // ── Actions ─────────────────────────────────────────────────────────────

    fn perform(self: &Rc<Self>, item: &ResultItem, action: Option<&Action>) {
        let action = action.cloned().unwrap_or_else(|| item.action.clone());
        let primary = action == item.action;
        if item.confirm.is_some() && primary && self.st.borrow().confirm_id.as_deref() != Some(item.id.as_str()) {
            self.st.borrow_mut().confirm_id = Some(item.id.clone());
            self.render_footer();
            return;
        }
        self.st.borrow_mut().confirm_id = None;
        self.close_menu();
        let Some(app) = self.app() else { return };
        let res = app.execute(&action, item.history_key.as_deref());
        if !res.ok {
            self.flash(res.error.as_deref().unwrap_or("Something went wrong"), true);
            return;
        }
        if let Some(v) = res.view {
            self.set_view(v);
            return;
        }
        if res.refresh {
            let mut st = self.st.borrow_mut();
            st.preview_cache.clear();
            st.keep_selection = Some(item.id.clone());
            drop(st);
            self.run_search();
        }
        if let Some(cmd) = res.run_command {
            self.start_run(cmd);
            return;
        }
        if let Some(t) = res.toast {
            self.flash(&t, false);
            if !res.keep_open {
                let weak = self.app.clone();
                glib::timeout_add_local_once(Duration::from_millis(650), move || {
                    if let Some(a) = weak.upgrade() {
                        a.hide(true);
                    }
                });
            }
            return;
        }
        if !res.keep_open {
            app.hide(true);
        }
    }

    fn secondary(self: &Rc<Self>) {
        let Some(c) = self.current() else { return };
        if let Some(alt) = c.actions.iter().find(|a| a.hint.as_deref() == Some("Ctrl+Enter")) {
            let a = alt.action.clone();
            self.perform(&c, Some(&a));
        }
    }

    pub fn copy_text(self: &Rc<Self>, text: &str) {
        if let Some(app) = self.app() {
            app.clip.write(text);
            self.flash("Copied to clipboard", false);
        }
    }

    pub fn open_url_and_hide(self: &Rc<Self>, url: &str) {
        if let Some(app) = self.app() {
            let r = app.execute(&Action::OpenUrl(url.to_string()), None);
            if r.ok {
                app.hide(true);
            } else {
                self.flash(r.error.as_deref().unwrap_or("Couldn't open the link"), true);
            }
        }
    }

    pub fn flash(self: &Rc<Self>, text: &str, error: bool) {
        self.clear_toast();
        let t = hbox(8, "toast");
        t.set_halign(gtk::Align::Center);
        t.set_valign(gtk::Align::End);
        t.set_can_target(false);
        let g = glyph(if error { "circle-alert" } else { "circle-check" }, 16);
        g.add_css_class(if error { "err" } else { "ok" });
        t.append(&g);
        t.append(&gtk::Label::new(Some(text)));
        self.overlay.add_overlay(&t);
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(if error { 2600 } else { 1400 }), move || {
            if let Some(p) = weak.upgrade()
                && let Some((w, _)) = p.toast.take()
            {
                p.overlay.remove_overlay(&w);
            }
        });
        self.toast.replace(Some((t.upcast(), id)));
    }

    fn clear_toast(&self) {
        if let Some((w, id)) = self.toast.take() {
            id.remove();
            self.overlay.remove_overlay(&w);
        }
    }

    // ── Action menu ─────────────────────────────────────────────────────────

    fn menu_actions(&self) -> Vec<SecondaryAction> {
        match self.current() {
            Some(c) => std::iter::once(primary_action(&c)).chain(c.actions.iter().cloned()).collect(),
            None => Vec::new(),
        }
    }

    fn open_menu(self: &Rc<Self>, selected: usize) {
        let Some(current) = self.current() else { return };
        self.st.borrow_mut().menu = Some(selected);
        self.close_menu_widget();
        let actions = self.menu_actions();
        let weak = Rc::downgrade(self);
        let on_hover: Rc<dyn Fn(usize)> = Rc::new({
            let weak = weak.clone();
            move |i| {
                if let Some(p) = weak.upgrade()
                    && p.st.borrow().menu != Some(i)
                {
                    p.open_menu(i);
                }
            }
        });
        let on_run: Rc<dyn Fn(usize)> = Rc::new(move |i| {
            if let Some(p) = weak.upgrade()
                && let Some(c) = p.current()
                && let Some(a) = p.menu_actions().get(i).cloned()
            {
                p.perform(&c, Some(&a.action));
            }
        });
        let moved: results::MovedFn = {
            let weak = Rc::downgrade(self);
            Rc::new(move |w, x, y| weak.upgrade().is_some_and(|p| p.pointer_moved(w, x, y)))
        };
        let m = menu::build(&current.title, &actions, selected, on_hover, on_run, moved);
        self.overlay.add_overlay(&m);
        self.menu_widget.replace(Some(m));
    }

    fn close_menu_widget(&self) {
        if let Some(m) = self.menu_widget.take() {
            self.overlay.remove_overlay(&m);
        }
    }

    fn close_menu(&self) {
        self.st.borrow_mut().menu = None;
        self.close_menu_widget();
    }

    fn point_in_menu(&self, x: f64, y: f64) -> bool {
        let Some(m) = &*self.menu_widget.borrow() else { return false };
        m.compute_bounds(&self.stage).is_some_and(|b| b.contains_point(&graphene::Point::new(x as f32, y as f32)))
    }

    // ── Inline command runs ─────────────────────────────────────────────────

    fn start_run(self: &Rc<Self>, command: String) {
        let token = self.run_token.get() + 1;
        self.run_token.set(token);
        let (tx, rx) = async_channel::unbounded::<lumen_core::terminal::RunEvent>();
        let pid = match lumen_core::terminal::run_inline(&command, move |e| {
            let _ = tx.send_blocking(e);
        }) {
            Ok(pid) => pid,
            Err(e) => {
                self.flash(&format!("Couldn't run it: {e}"), true);
                return;
            }
        };
        let view = OutputView::new(self, &command);
        self.output.replace(Some(view.clone()));
        self.st.borrow_mut().run =
            Some(RunState { pid, command, done: false, exit_code: None, started: Instant::now(), ended: None, text: String::new() });
        self.render_body();
        self.render_chrome();
        view.refresh(self.st.borrow().run.as_ref().unwrap());
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(ev) = rx.recv().await {
                let Some(p) = weak.upgrade() else { return };
                if p.run_token.get() != token {
                    return;
                }
                let Some(view) = p.output.borrow().clone() else { return };
                match ev {
                    lumen_core::terminal::RunEvent::Output { text, stderr } => {
                        let clean = output::strip_ansi(&text);
                        if let Some(r) = p.st.borrow_mut().run.as_mut() {
                            r.text.push_str(&clean);
                        }
                        view.append(&clean, stderr);
                    }
                    lumen_core::terminal::RunEvent::Done { exit_code } => {
                        if let Some(r) = p.st.borrow_mut().run.as_mut() {
                            r.done = true;
                            r.exit_code = exit_code;
                            r.ended = Some(Instant::now());
                        }
                        if let Some(r) = p.st.borrow().run.as_ref() {
                            view.refresh(r);
                        }
                        return;
                    }
                }
            }
        });
    }

    pub fn stop_run(&self) {
        if let Some(r) = self.st.borrow().run.as_ref()
            && !r.done
        {
            lumen_core::terminal::kill_run(r.pid);
        }
    }

    pub fn rerun(self: &Rc<Self>) {
        let Some(cmd) = self.st.borrow().run.as_ref().map(|r| r.command.clone()) else { return };
        let item = ResultItem::new("rerun", ResultKind::Command, cmd.clone(), Action::RunCommand { command: cmd, mode: RunMode::Inline });
        self.perform(&item, None);
    }

    pub fn run_in_terminal(self: &Rc<Self>) {
        let Some(cmd) = self.st.borrow().run.as_ref().map(|r| r.command.clone()) else { return };
        if let Some(app) = self.app() {
            app.execute(&Action::RunCommand { command: cmd, mode: RunMode::Terminal }, None);
            app.hide(true);
        }
    }

    pub fn output_text(&self) -> String {
        self.st.borrow().run.as_ref().map(|r| r.text.clone()).unwrap_or_default()
    }

    fn close_run(self: &Rc<Self>) {
        self.stop_run();
        self.st.borrow_mut().run = None;
        self.output.replace(None);
        self.render_body();
        self.render_chrome();
        self.entry.grab_focus();
    }

    // ── Settings and welcome ────────────────────────────────────────────────

    pub fn finish_welcome(self: &Rc<Self>) {
        if let Some(app) = self.app() {
            app.update_settings(|s| s.first_run_done = true);
        }
        self.set_view(View::Search);
    }

    pub fn back_to_search(self: &Rc<Self>) {
        self.set_view(View::Search);
    }

    pub fn show_shortcut_view(self: &Rc<Self>) {
        self.set_view(View::Shortcut);
    }

    pub fn tip(self: &Rc<Self>, prefix: &str) {
        self.set_query(prefix);
        self.run_search();
        self.entry.grab_focus();
        self.entry.set_position(-1);
    }

    pub fn app_handle(&self) -> Option<Rc<App>> {
        self.app()
    }

    // ── Keyboard ────────────────────────────────────────────────────────────

    /// Development hook: press a key combo like "Down", "Enter" or "ctrl+K".
    pub fn press(self: &Rc<Self>, combo: &str) -> Result<(), String> {
        let mut mods = gdk::ModifierType::empty();
        let parts: Vec<&str> = combo.split('+').collect();
        let (last, rest) = parts.split_last().ok_or("empty combo")?;
        for m in rest {
            match m.to_lowercase().as_str() {
                "ctrl" | "control" => mods |= gdk::ModifierType::CONTROL_MASK,
                "shift" => mods |= gdk::ModifierType::SHIFT_MASK,
                "alt" => mods |= gdk::ModifierType::ALT_MASK,
                other => return Err(format!("unknown modifier {other}")),
            }
        }
        let name = match last.to_lowercase().as_str() {
            "enter" => "Return".to_string(),
            "esc" => "Escape".into(),
            "pagedown" => "Page_Down".into(),
            "pageup" => "Page_Up".into(),
            _ => last.to_string(),
        };
        let key = gdk::Key::from_name(&name).ok_or_else(|| format!("unknown key {last}"))?;
        if !self.handle_key(key, mods)
            && let Some(c) = key.to_unicode().filter(|c| !c.is_control())
        {
            let pos = self.entry.position();
            let mut p = pos;
            self.entry.insert_text(&c.to_string(), &mut p);
            self.entry.set_position(p);
        }
        Ok(())
    }

    fn focus_in_text_field(&self) -> bool {
        let Some(f) = gtk::prelude::GtkWindowExt::focus(&self.window) else { return false };
        if f == self.entry.clone().upcast::<gtk::Widget>() {
            return false;
        }
        f.is::<gtk::Text>()
            || f.is::<gtk::Entry>()
            || f.ancestor(gtk::Entry::static_type()).is_some()
            || f.ancestor(gtk::DropDown::static_type()).is_some()
    }

    fn handle_key(self: &Rc<Self>, key: gdk::Key, mods: gdk::ModifierType) -> bool {
        use gdk::Key as K;
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK) || mods.contains(gdk::ModifierType::META_MASK);
        let shift = mods.contains(gdk::ModifierType::SHIFT_MASK);
        let enter = matches!(key, K::Return | K::KP_Enter);
        let view = self.st.borrow().view;

        if view == View::Settings {
            if key == K::Escape {
                if self.focus_in_text_field() {
                    gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
                } else {
                    self.back_to_search();
                }
                return true;
            }
            if self.focus_in_text_field() {
                return false;
            }
            let d = match key {
                K::Down => 1,
                K::Up => -1,
                K::Tab if ctrl => 1,
                K::ISO_Left_Tab if ctrl => -1,
                _ => 0,
            };
            if d != 0 {
                settings::SettingsView::step_page(self, d);
                return true;
            }
            return false;
        }
        if view == View::Shortcut {
            if key == K::Escape {
                self.finish_welcome();
                return true;
            }
            return false;
        }

        let run = self.st.borrow().run.as_ref().map(|r| (r.done, r.command.clone()));
        if let Some((done, _)) = run {
            if key == K::Escape {
                self.close_run();
                return true;
            }
            if ctrl && key == K::c && !done {
                let has_selection = self.output.borrow().as_ref().is_some_and(|o| o.has_selection());
                if !has_selection {
                    self.stop_run();
                    return true;
                }
                return false;
            }
            if enter && ctrl {
                self.run_in_terminal();
                return true;
            }
            if enter && done {
                self.rerun();
                return true;
            }
            return false;
        }

        let menu = self.st.borrow().menu;
        if let Some(sel) = menu {
            let n = self.menu_actions().len().max(1);
            match key {
                K::Down | K::Up => {
                    let d = if key == K::Down { 1 } else { n - 1 };
                    self.open_menu((sel + d) % n);
                }
                _ if enter => {
                    if let Some(c) = self.current()
                        && let Some(a) = self.menu_actions().get(sel).cloned()
                    {
                        self.perform(&c, Some(&a.action));
                    }
                }
                K::Escape | K::Tab | K::ISO_Left_Tab => self.close_menu(),
                K::k if ctrl => self.close_menu(),
                _ => return false,
            }
            return true;
        }

        let current = self.current();
        // Result-specific shortcuts, e.g. Ctrl+Shift+P to pin a clip (Ctrl+Enter is handled below).
        if ctrl && let Some(c) = &current {
            let combo = combo_of(key, mods);
            if let Some(hit) = c.actions.iter().find(|a| a.hint.as_deref().is_some_and(|h| h != "Ctrl+Enter" && h == combo)) {
                let a = hit.action.clone();
                self.perform(c, Some(&a));
                return true;
            }
        }

        let n = self.items().len();
        let selected = self.st.borrow().selected;
        let go_to = |p: &Rc<Panel>, i: isize| {
            let i = i.clamp(0, n.saturating_sub(1) as isize) as usize;
            p.select(i);
        };
        // Before typing, the first section is a row of app tiles: ←/→ move inside it, ↑/↓ jump in and out.
        let grid = self.grid_count();
        let in_grid = selected < grid;
        let s = selected as isize;
        match key {
            K::Down => go_to(self, if in_grid { if grid < n { grid as isize } else { s } } else { s + 1 }),
            K::n | K::j if ctrl => go_to(self, if in_grid { if grid < n { grid as isize } else { s } } else { s + 1 }),
            K::Up => go_to(
                self,
                if in_grid {
                    s
                } else if selected == grid {
                    0
                } else {
                    s - 1
                },
            ),
            K::p if ctrl => go_to(
                self,
                if in_grid {
                    s
                } else if selected == grid {
                    0
                } else {
                    s - 1
                },
            ),
            K::Right | K::Left if in_grid => go_to(self, s + if key == K::Right { 1 } else { -1 }),
            K::Page_Down => go_to(self, s + 5),
            K::Page_Up => go_to(self, s - 5),
            _ if enter => {
                if let Some(c) = current {
                    if ctrl {
                        self.secondary();
                    } else {
                        self.perform(&c, None);
                    }
                }
            }
            K::k if ctrl => {
                if self.menu_actions().len() > 1 {
                    self.open_menu(0);
                }
            }
            K::Tab | K::ISO_Left_Tab if current.is_some() && !shift => {
                if self.menu_actions().len() > 1 {
                    self.open_menu(0);
                }
            }
            K::comma if ctrl => self.set_view(View::Settings),
            K::Escape => {
                let (confirm, query) = {
                    let st = self.st.borrow();
                    (st.confirm_id.is_some(), !st.query.is_empty())
                };
                if confirm {
                    self.st.borrow_mut().confirm_id = None;
                    self.render_footer();
                } else if query {
                    self.set_query("");
                    self.run_search();
                } else if let Some(app) = self.app() {
                    app.hide(true);
                }
            }
            _ => return false,
        }
        true
    }
}
