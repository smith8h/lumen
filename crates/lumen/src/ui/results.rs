//! The result list: sections of rows over a canvas that slides the lit selection.

use gtk::glib;
use gtk::prelude::*;
use lumen_core::types::{Glyph, IconSpec, ResultItem, ResultKind, ResultSection, SectionId};
use std::rc::Rc;

use super::bits::{ellipsized, hbox, highlight_markup, icon_slot, keys, label, vbox};
use super::widgets::ListCanvas;

pub fn kind_label(kind: ResultKind) -> &'static str {
    match kind {
        ResultKind::App => "Application",
        ResultKind::File => "File",
        ResultKind::Folder => "Folder",
        ResultKind::Calc => "Copy",
        ResultKind::Url => "Open",
        ResultKind::Web => "Search",
        ResultKind::Command | ResultKind::System => "Run",
        ResultKind::Lumen => "Open",
        ResultKind::Clip => "Paste",
    }
}

fn kind_class(kind: ResultKind) -> &'static str {
    match kind {
        ResultKind::App => "app",
        ResultKind::File => "file",
        ResultKind::Folder => "folder",
        ResultKind::Calc => "calc",
        ResultKind::Url => "url",
        ResultKind::Web => "web",
        ResultKind::Command => "command",
        ResultKind::System => "system",
        ResultKind::Lumen => "lumen",
        ResultKind::Clip => "clip",
    }
}

/// Whether the pointer really moved over a widget, given pointer coordinates inside it.
pub type MovedFn = Rc<dyn Fn(&gtk::Widget, f64, f64) -> bool>;

/// Row callbacks: hover (only after real pointer movement) and click. Indexes are global.
pub struct RowEvents {
    pub hover: Rc<dyn Fn(usize)>,
    pub run: Rc<dyn Fn(usize)>,
    /// Whether the pointer really moved (the panel often opens under a resting cursor).
    pub moved: MovedFn,
}

pub fn attach_pointer(widget: &impl IsA<gtk::Widget>, index: usize, ev: &RowEvents) {
    let motion = gtk::EventControllerMotion::new();
    let (hover, moved) = (ev.hover.clone(), ev.moved.clone());
    let w = widget.clone().upcast::<gtk::Widget>();
    motion.connect_motion(move |_, x, y| {
        if moved(&w, x, y) {
            hover(index);
        }
    });
    widget.add_controller(motion);
    let click = gtk::GestureClick::new();
    let run = ev.run.clone();
    click.connect_released(move |g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        run(index);
    });
    widget.add_controller(click);
}

fn row(item: &ResultItem, top: bool) -> gtk::Box {
    let r = hbox(12, &format!("row {}", kind_class(item.kind)));
    if top {
        r.add_css_class("top");
    }
    let thumb = matches!(item.icon, Some(IconSpec::File(_))) && item.kind == ResultKind::File;
    r.append(&icon_slot(item.icon.as_ref(), if top { 38 } else { 30 }, thumb, Glyph::Sparkles));

    let text = vbox(1, "text");
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    let title = ellipsized("", "title");
    title.set_markup(&highlight_markup(&item.title, &item.matches));
    if !item.matches.is_empty() {
        title.add_css_class("has-matches");
    }
    text.append(&title);
    if let Some(sub) = &item.subtitle {
        text.append(&ellipsized(sub, "subtitle"));
    }
    r.append(&text);

    let trail = hbox(6, "trail");
    trail.set_valign(gtk::Align::Center);
    if item.confirm.is_none() {
        trail.append(&label(kind_label(item.kind), "kind"));
    }
    trail.append(&keys("Enter"));
    r.append(&trail);
    r
}

pub struct ResultList {
    pub root: gtk::Widget,
    canvas: ListCanvas,
    rows: Vec<gtk::Box>,
    offset: usize,
}

impl ResultList {
    /// `scroll_max`: wrap in a scroller this tall at most (0 = no scroller of its own).
    pub fn new(sections: &[ResultSection], offset: usize, scroll_max: i32, ev: &RowEvents) -> ResultList {
        let content = vbox(0, "results-content");
        let mut rows = Vec::new();
        for section in sections {
            content.append(&label(&section.title, "section-title"));
            for item in &section.items {
                let index = offset + rows.len();
                let r = row(item, section.id == SectionId::Top);
                attach_pointer(&r, index, ev);
                content.append(&r);
                rows.push(r);
            }
        }
        let canvas = ListCanvas::new(&content);
        let root: gtk::Widget = if scroll_max > 0 {
            let sw = gtk::ScrolledWindow::new();
            sw.add_css_class("results");
            sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            sw.set_propagate_natural_height(true);
            sw.set_max_content_height(scroll_max);
            sw.set_child(Some(&canvas));
            let adj = sw.vadjustment();
            canvas.connect_target(move |y, h| {
                let adj = adj.clone();
                glib::idle_add_local_once(move || {
                    let (top, bottom) = (y - 30.0, y + h + 8.0);
                    if top < adj.value() {
                        adj.set_value(top.max(0.0));
                    } else if bottom > adj.value() + adj.page_size() {
                        adj.set_value(bottom - adj.page_size());
                    }
                });
            });
            sw.upcast()
        } else {
            canvas.clone().upcast()
        };
        ResultList { root, canvas, rows, offset }
    }

    /// Select by global index; indexes outside this list clear the selection here.
    pub fn set_selected(&self, global: Option<usize>) {
        let local = global.and_then(|g| g.checked_sub(self.offset)).filter(|i| *i < self.rows.len());
        for (i, r) in self.rows.iter().enumerate() {
            if Some(i) == local {
                r.add_css_class("selected");
            } else {
                r.remove_css_class("selected");
            }
        }
        self.canvas.set_selected(local.map(|i| &self.rows[i]));
    }
}
