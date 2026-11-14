//! The preview pane beside the results.

use gtk::prelude::*;
use lumen_core::format::{count, plural, size, when};
use lumen_core::store::{now_ms, tilde};
use lumen_core::types::{Action, Glyph, Preview, ResultItem, ResultKind, RunMode};

use super::bits::{ellipsized, glyph, hbox, icon, keys, label, thumbnail, vbox};

fn meta(rows: &[(&str, Option<String>)]) -> gtk::Widget {
    let grid = gtk::Grid::new();
    grid.add_css_class("meta");
    grid.set_row_spacing(5);
    grid.set_column_spacing(12);
    grid.set_valign(gtk::Align::End);
    grid.set_vexpand(true);
    let mut r = 0;
    for (k, v) in rows {
        let Some(v) = v.as_ref().filter(|v| !v.is_empty()) else { continue };
        grid.attach(&label(k, "key"), 0, r, 1, 1);
        let value = ellipsized(v, "value");
        value.set_xalign(1.0);
        value.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        grid.attach(&value, 1, r, 1, 1);
        r += 1;
    }
    grid.upcast()
}

fn hero(big_icon: gtk::Widget, title: &str, text: Option<&str>) -> gtk::Box {
    let h = vbox(10, "hero");
    h.set_margin_top(8);
    big_icon.set_halign(gtk::Align::Center);
    h.append(&big_icon);
    let t = label(title, "hero-title");
    t.set_xalign(0.5);
    t.set_justify(gtk::Justification::Center);
    t.set_wrap(true);
    t.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    t.set_max_width_chars(28);
    h.append(&t);
    if let Some(p) = text.filter(|s| !s.is_empty()) {
        let l = label(p, "hero-text");
        l.set_xalign(0.5);
        l.set_justify(gtk::Justification::Center);
        l.set_wrap(true);
        l.set_max_width_chars(34);
        h.append(&l);
    }
    h
}

fn host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    Some(rest.split(['/', '?', '#']).next()?.to_string())
}

fn code(text: &str, clip: bool) -> gtk::Widget {
    let l = label(text, "");
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_yalign(0.0);
    l.set_selectable(clip);
    l.set_max_width_chars(1);
    l.set_hexpand(true);
    let b = vbox(0, "code");
    b.set_vexpand(true);
    b.append(&l);
    b.set_overflow(gtk::Overflow::Hidden);
    if clip {
        let sw = gtk::ScrolledWindow::new();
        sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        sw.set_vexpand(true);
        sw.set_child(Some(&b));
        sw.upcast()
    } else {
        b.upcast()
    }
}

fn categories(cats: &[String]) -> gtk::Widget {
    let chips = gtk::FlowBox::new();
    chips.add_css_class("chips");
    chips.set_selection_mode(gtk::SelectionMode::None);
    chips.set_halign(gtk::Align::Center);
    chips.set_column_spacing(6);
    chips.set_row_spacing(6);
    chips.set_max_children_per_line(4);
    chips.set_can_target(false);
    for c in cats {
        // "AudioVideo" → "Audio Video"
        let mut pretty = String::new();
        let mut prev_lower = false;
        for ch in c.chars() {
            if ch.is_uppercase() && prev_lower {
                pretty.push(' ');
            }
            prev_lower = ch.is_lowercase();
            pretty.push(ch);
        }
        chips.insert(&gtk::Label::new(Some(&pretty)), -1);
    }
    chips.upcast()
}

/// Build the pane for `item`; `preview` is None while it loads.
pub fn pane(item: &ResultItem, preview: Option<&Preview>) -> gtk::Widget {
    let root = vbox(14, "preview");
    let now = now_ms();
    match preview.unwrap_or(&Preview::None) {
        Preview::None => {
            root.append(&hero(icon(item.icon.as_ref(), 76, false, Glyph::Sparkles), &item.title, item.subtitle.as_deref()));
            if item.kind == ResultKind::Command {
                let inline = matches!(item.action, Action::RunCommand { mode: RunMode::Inline, .. });
                root.append(&meta(&[
                    ("Enter", Some(if inline { "Run & show output" } else { "Run in terminal" }.into())),
                    ("Ctrl+K", Some("More actions".into())),
                ]));
            }
            if let (ResultKind::Web | ResultKind::Url, Action::OpenUrl(url)) = (item.kind, &item.action) {
                root.append(&meta(&[("Opens", host(url))]));
            }
        }
        Preview::App { name, comment, icon: app_icon, exec, categories: cats, path } => {
            let h = hero(icon(app_icon.as_ref(), 76, false, Glyph::Sparkles), name, comment.as_deref());
            if !cats.is_empty() {
                h.append(&categories(cats));
            }
            root.append(&h);
            if !item.actions.is_empty() {
                let list = vbox(4, "actions-list");
                for a in item.actions.iter().take(3) {
                    let b = hbox(0, "action-chip");
                    b.append(&ellipsized(&a.label, ""));
                    list.append(&b);
                }
                root.append(&list);
            }
            root.append(&meta(&[("Command", Some(tilde(exec))), ("Entry", Some(tilde(path)))]));
        }
        Preview::Image { path, name, size: bytes, modified } => {
            let frame = vbox(0, "image");
            frame.set_vexpand(true);
            frame.set_overflow(gtk::Overflow::Hidden);
            let pic = thumbnail(path, 260, root.scale_factor(), false);
            pic.set_vexpand(true);
            frame.append(&pic);
            root.append(&frame);
            let dims = gtk::gdk_pixbuf::Pixbuf::file_info(path).map(|(_, w, h)| format!("{w} × {h}"));
            root.append(&meta(&[
                ("Name", Some(name.clone())),
                ("Dimensions", dims),
                ("Size", Some(size(*bytes))),
                ("Modified", Some(when(*modified, now))),
            ]));
        }
        Preview::Text { name, size: bytes, modified, content, .. } => {
            let text: String = content.lines().take(40).collect::<Vec<_>>().join("\n");
            root.append(&code(&text, false));
            root.append(&meta(&[("Name", Some(name.clone())), ("Size", Some(size(*bytes))), ("Modified", Some(when(*modified, now)))]));
        }
        Preview::Folder { path, name, modified, entries, total } => {
            let head = hbox(12, "");
            head.append(&icon(item.icon.as_ref(), 44, false, Glyph::Folder));
            let t = vbox(2, "");
            t.append(&ellipsized(name, "hero-title"));
            t.append(&label(&if *total == 0 { "Empty folder".to_string() } else { plural(*total, "item", "items") }, "hero-text"));
            head.append(&t);
            root.append(&head);
            let list = vbox(2, "entries");
            list.set_overflow(gtk::Overflow::Hidden);
            for e in entries {
                let r = hbox(8, "entry");
                let g = glyph(if e.dir { "folder" } else { "file" }, 14);
                g.add_css_class("glyph-dim");
                r.append(&g);
                r.append(&ellipsized(&e.name, ""));
                list.append(&r);
            }
            root.append(&list);
            root.append(&meta(&[("Where", Some(tilde(path))), ("Modified", Some(when(*modified, now)))]));
        }
        Preview::File { path, name, size: bytes, modified, icon: file_icon } => {
            root.append(&hero(icon(Some(file_icon), 76, false, Glyph::File), name, None));
            root.append(&meta(&[("Where", Some(tilde(path))), ("Size", Some(size(*bytes))), ("Modified", Some(when(*modified, now)))]));
        }
        Preview::Clip { text, truncated, length, lines, first_at, last_at, pinned } => {
            let mut shown = text.clone();
            if *truncated {
                shown.push_str(&format!("\n… {} more characters", count(length - text.chars().count())));
            }
            root.append(&code(&shown, true));
            root.append(&meta(&[
                ("Copied", Some(when(*last_at, now))),
                ("First copied", (last_at - first_at > 60_000).then(|| when(*first_at, now))),
                ("Length", Some(plural(*length, "character", "characters"))),
                ("Lines", Some(count(*lines))),
                ("Pinned", Some(if *pinned { "Yes" } else { "No" }.into())),
            ]));
        }
        Preview::Calc { expression, result, detail } => {
            let h = vbox(14, "hero");
            h.set_margin_top(34);
            let e = label(expression, "hero-text mono");
            e.set_xalign(0.5);
            e.set_wrap(true);
            h.append(&e);
            let r = label(result, "calc-result");
            r.set_xalign(0.5);
            r.set_wrap(true);
            r.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            r.set_justify(gtk::Justification::Center);
            r.set_selectable(true);
            h.append(&r);
            let hint = hbox(6, "hero-text");
            hint.set_halign(gtk::Align::Center);
            hint.append(&keys("Enter"));
            hint.append(&gtk::Label::new(Some("to copy")));
            h.append(&hint);
            root.append(&h);
            if let Some(d) = detail.as_ref().filter(|d| **d != format!("{expression} =")) {
                let l = label(d, "detail");
                l.set_xalign(0.5);
                l.set_wrap(true);
                l.set_vexpand(true);
                l.set_valign(gtk::Align::End);
                root.append(&l);
            }
        }
    }
    root.upcast()
}
