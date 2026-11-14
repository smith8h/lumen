//! First-run screen: explains Alt+Space and sets the shortcut up with one click.

use gtk::prelude::*;
use gtk::{gio, glib};
use lumen_core::shortcut::{self, ShortcutStatus};
use std::rc::Rc;

use super::Panel;
use super::bits::{button, glyph, hbox, label, mark, vbox};

fn centered(l: &gtk::Label) {
    l.set_xalign(0.5);
    l.set_justify(gtk::Justification::Center);
    l.set_wrap(true);
    l.set_max_width_chars(60);
}

pub fn build(panel: &Rc<Panel>) -> gtk::Widget {
    let root = vbox(14, "welcome");
    let m = mark(72);
    m.set_halign(gtk::Align::Center);
    root.append(&m);
    let t = label("Welcome to Lumen", "welcome-title");
    centered(&t);
    root.append(&t);
    let p = label("Apps, files, calculations, the web and your terminal — one keystroke away.", "welcome-text");
    centered(&p);
    root.append(&p);
    let keys = hbox(8, "");
    keys.set_halign(gtk::Align::Center);
    keys.set_margin_top(6);
    keys.set_margin_bottom(6);
    let alt = label("Alt", "bigkey");
    alt.set_xalign(0.5);
    keys.append(&alt);
    keys.append(&label("+", "plus"));
    let space = label("Space", "bigkey space");
    space.set_xalign(0.5);
    keys.append(&space);
    root.append(&keys);

    let area = vbox(14, "");
    root.append(&area);
    let weak = Rc::downgrade(panel);
    let area_ref = area.clone();
    glib::spawn_future_local(async move {
        let Ok(status) = gio::spawn_blocking(shortcut::status).await else { return };
        if let Some(p) = weak.upgrade() {
            fill(&p, &area_ref, &status);
        }
    });
    root.upcast()
}

fn fill(panel: &Rc<Panel>, area: &gtk::Box, status: &ShortcutStatus) {
    while let Some(c) = area.first_child() {
        area.remove(&c);
    }
    let buttons = hbox(10, "");
    buttons.set_halign(gtk::Align::Center);
    buttons.set_margin_top(4);
    let start = |text: &str| {
        let b = button(text, None, None, "primary");
        let w = Rc::downgrade(panel);
        b.connect_clicked(move |_| {
            if let Some(p) = w.upgrade() {
                p.finish_welcome();
            }
        });
        b
    };
    if status.configured {
        let line = hbox(8, "status-line");
        line.set_halign(gtk::Align::Center);
        line.append(&glyph("check", 16));
        line.append(&gtk::Label::new(Some(&format!("Shortcut ready — press {} anywhere", shortcut::pretty_binding(&status.binding)))));
        area.append(&line);
        let b = start("Start searching");
        buttons.append(&b);
        area.append(&buttons);
        b.grab_focus();
    } else if status.supported {
        let desktop = status.desktop.strip_prefix("ubuntu:").or(status.desktop.strip_prefix("Ubuntu:")).unwrap_or(&status.desktop);
        let extra = if status.conflicts.is_empty() { "" } else { " and moves GNOME’s own Alt+Space (the window menu) out of the way" };
        let note = label(&format!("Lumen adds a keyboard shortcut to {desktop}{extra}. You can undo this anytime in Settings."), "note");
        centered(&note);
        area.append(&note);
        let enable = button("Enable Alt+Space", Some("keyboard"), None, "primary");
        let w = Rc::downgrade(panel);
        let area_ref = area.clone();
        enable.connect_clicked(move |b| {
            b.set_sensitive(false);
            let w = w.clone();
            let area_ref = area_ref.clone();
            glib::spawn_future_local(async move {
                let Ok(s) = gio::spawn_blocking(shortcut::setup).await else { return };
                if let Some(p) = w.upgrade() {
                    fill(&p, &area_ref, &s);
                }
            });
        });
        buttons.append(&enable);
        let later = button("Not now", None, None, "ghost");
        let w = Rc::downgrade(panel);
        later.connect_clicked(move |_| {
            if let Some(p) = w.upgrade() {
                p.finish_welcome();
            }
        });
        buttons.append(&later);
        area.append(&buttons);
        enable.grab_focus();
    } else {
        let note = label("Add a custom shortcut in your desktop’s keyboard settings that runs this command, bound to Alt+Space:", "note");
        centered(&note);
        area.append(&note);
        let manual = hbox(8, "");
        manual.set_halign(gtk::Align::Center);
        let code = label(&status.command, "manual-code");
        code.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        code.set_max_width_chars(60);
        code.set_selectable(true);
        manual.append(&code);
        let copy = button("Copy", Some("copy"), None, "small");
        let w = Rc::downgrade(panel);
        let cmd = status.command.clone();
        copy.connect_clicked(move |_| {
            if let Some(p) = w.upgrade() {
                p.copy_text(&cmd);
            }
        });
        manual.append(&copy);
        area.append(&manual);
        let b = start("Start searching");
        buttons.append(&b);
        area.append(&buttons);
        b.grab_focus();
    }
}
