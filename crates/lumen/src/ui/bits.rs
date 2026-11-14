//! Building blocks: icons, glyph tiles, keycaps, the Lumen mark and layout helpers.

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumen_core::types::{Glyph, IconSpec};
use std::cell::RefCell;
use std::collections::HashMap;

use super::widgets::Glyph as GlyphWidget;

/// Lucide icon for each built-in glyph.
pub fn lucide(g: Glyph) -> &'static str {
    match g {
        Glyph::Calculator => "calculator",
        Glyph::Globe => "globe",
        Glyph::Search => "search",
        Glyph::Terminal => "square-terminal",
        Glyph::Lock => "lock",
        Glyph::Logout => "log-out",
        Glyph::Moon => "moon",
        Glyph::Sun => "sun",
        Glyph::Power => "power",
        Glyph::Restart => "rotate-ccw",
        Glyph::Settings => "settings",
        Glyph::Folder => "folder",
        Glyph::File => "file",
        Glyph::Trash => "trash-2",
        Glyph::Wifi => "wifi",
        Glyph::Bell => "bell-off",
        Glyph::Refresh => "refresh-cw",
        Glyph::Quit => "circle-x",
        Glyph::Sparkles => "sparkles",
        Glyph::Copy => "copy",
        Glyph::Keyboard => "keyboard",
        Glyph::Clipboard => "clipboard",
        Glyph::Pin => "pin",
        Glyph::Paste => "clipboard-paste",
    }
}

/// CSS tint class for each glyph's tile.
fn tint(g: Glyph) -> &'static str {
    match g {
        Glyph::Calculator => "calculator",
        Glyph::Globe => "globe",
        Glyph::Search => "search",
        Glyph::Terminal => "terminal",
        Glyph::Lock => "lock",
        Glyph::Logout => "logout",
        Glyph::Moon => "moon",
        Glyph::Sun => "sun",
        Glyph::Power => "power",
        Glyph::Restart => "restart",
        Glyph::Settings => "settings",
        Glyph::Folder => "folder",
        Glyph::File => "file",
        Glyph::Trash => "trash",
        Glyph::Wifi => "wifi",
        Glyph::Bell => "bell",
        Glyph::Refresh => "refresh",
        Glyph::Quit => "quit",
        Glyph::Sparkles => "sparkles",
        Glyph::Copy => "copy",
        Glyph::Keyboard => "keyboard",
        Glyph::Clipboard => "clipboard",
        Glyph::Pin => "pin",
        Glyph::Paste => "paste",
    }
}

pub fn glyph(name: &'static str, size: i32) -> GlyphWidget {
    GlyphWidget::new(name, size)
}

/// macOS-style colourful tile with a white line icon.
pub fn glyph_tile(g: Glyph, size: i32) -> gtk::Widget {
    let tile = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tile.add_css_class("tile");
    tile.add_css_class(&format!("tint-{}", tint(g)));
    if size >= 60 {
        tile.add_css_class("big");
    } else if size >= 40 {
        tile.add_css_class("huge");
    }
    tile.set_size_request(size, size);
    tile.set_hexpand(false);
    tile.set_vexpand(false);
    tile.set_halign(gtk::Align::Center);
    tile.set_valign(gtk::Align::Center);
    let inner = GlyphWidget::new(lucide(g), (size as f32 * 0.58).round() as i32).with_stroke(2.1);
    inner.set_hexpand(true);
    inner.set_halign(gtk::Align::Center);
    inner.set_valign(gtk::Align::Center);
    tile.append(&inner);
    tile.upcast()
}

fn icon_theme() -> Option<gtk::IconTheme> {
    gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d))
}

fn image_for_gicon(icon: &gio::Icon, size: i32) -> gtk::Widget {
    let img = gtk::Image::from_gicon(icon);
    img.set_pixel_size(size);
    img.add_css_class("app-icon");
    img.upcast()
}

thread_local! {
    static THUMBS: RefCell<HashMap<(String, i32), gdk::Texture>> = RefCell::new(HashMap::new());
}

/// Forget cached thumbnails (when the panel is unloaded).
pub fn clear_thumbnails() {
    THUMBS.with(|t| t.borrow_mut().clear());
}

/// Load an image file scaled to fit `px` (or cropped to a `px` square), off the UI thread.
fn load_scaled(path: &str, px: i32, square: bool) -> Option<(glib::Bytes, i32, i32, i32, bool)> {
    use gtk::gdk_pixbuf::Pixbuf;
    let pb = if square {
        let (_, w, h) = Pixbuf::file_info(path)?;
        let (w, h) = (w.max(1) as f64, h.max(1) as f64);
        let k = (px as f64 / w).max(px as f64 / h);
        let (sw, sh) = ((w * k).ceil().max(px as f64) as i32, (h * k).ceil().max(px as f64) as i32);
        let full = Pixbuf::from_file_at_scale(path, sw, sh, false).ok()?;
        let (x, y) = ((full.width() - px).max(0) / 2, (full.height() - px).max(0) / 2);
        full.new_subpixbuf(x, y, px.min(full.width()), px.min(full.height())).copy()?
    } else {
        Pixbuf::from_file_at_scale(path, px, px, true).ok()?
    };
    Some((pb.read_pixel_bytes(), pb.width(), pb.height(), pb.rowstride(), pb.has_alpha()))
}

/// An image file scaled down on a worker thread, then shown. `square` crops it to fill a
/// rounded square (result icons); otherwise it fits inside `size` (the preview pane).
pub fn thumbnail(path: &str, size: i32, scale: i32, square: bool) -> gtk::Widget {
    let px = size * scale.max(1);
    let key = (format!("{path}\u{0}{square}"), px);
    type Setter = Box<dyn Fn(&gdk::Texture)>;
    let (widget, set): (gtk::Widget, Setter) = if square {
        let img = gtk::Image::new();
        img.set_pixel_size(size);
        img.add_css_class("thumb");
        img.set_overflow(gtk::Overflow::Hidden);
        let w = img.downgrade();
        (
            img.upcast(),
            Box::new(move |t| {
                if let Some(i) = w.upgrade() {
                    i.set_paintable(Some(t))
                }
            }),
        )
    } else {
        let pic = gtk::Picture::new();
        pic.set_content_fit(gtk::ContentFit::Contain);
        pic.set_can_shrink(true);
        pic.set_size_request(-1, size.min(220));
        let w = pic.downgrade();
        (
            pic.upcast(),
            Box::new(move |t| {
                if let Some(p) = w.upgrade() {
                    p.set_paintable(Some(t))
                }
            }),
        )
    };
    if let Some(tex) = THUMBS.with(|t| t.borrow().get(&key).cloned()) {
        set(&tex);
        return widget;
    }
    let path = path.to_string();
    glib::spawn_future_local(async move {
        let p = path.clone();
        let loaded = gio::spawn_blocking(move || load_scaled(&p, px, square)).await.ok().flatten();
        let Some((bytes, w, h, stride, alpha)) = loaded else { return };
        let format = if alpha { gdk::MemoryFormat::R8g8b8a8 } else { gdk::MemoryFormat::R8g8b8 };
        let tex: gdk::Texture = gdk::MemoryTexture::new(w, h, format, &bytes, stride as usize).upcast();
        THUMBS.with(|t| {
            let mut t = t.borrow_mut();
            if t.len() > 200 {
                t.clear();
            }
            t.insert(key, tex.clone());
        });
        set(&tex);
    });
    widget
}

/// The icon for a result: an app icon from the icon theme, a thumbnail, a file-type icon or a glyph tile.
pub fn icon(spec: Option<&IconSpec>, size: i32, thumb: bool, fallback: Glyph) -> gtk::Widget {
    let tile = |g| glyph_tile(g, size);
    match spec {
        Some(IconSpec::Glyph(g)) => tile(*g),
        Some(IconSpec::Named(name)) => match icon_theme() {
            Some(theme) if theme.has_icon(name) => image_for_gicon(&gio::ThemedIcon::new(name).upcast(), size),
            _ => tile(fallback),
        },
        Some(IconSpec::File(path)) => {
            if thumb {
                thumbnail(path, size, 2, true)
            } else if std::path::Path::new(path).exists() {
                image_for_gicon(&gio::FileIcon::new(&gio::File::for_path(path)).upcast(), size)
            } else {
                tile(fallback)
            }
        }
        Some(IconSpec::FileType { path, dir }) => {
            let ct = if *dir { "inode/directory".to_string() } else { gio::content_type_guess(Some(path), None::<&[u8]>).0.to_string() };
            let gicon = gio::content_type_get_icon(&ct);
            image_for_gicon(&gicon, size)
        }
        None => tile(fallback),
    }
}

/// A fixed-size slot holding an icon, so rows line up.
pub fn icon_slot(spec: Option<&IconSpec>, size: i32, thumb: bool, fallback: Glyph) -> gtk::Widget {
    let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    slot.set_size_request(size, size);
    slot.set_valign(gtk::Align::Center);
    slot.set_hexpand(false);
    let i = icon(spec, size, thumb, fallback);
    i.set_hexpand(true);
    i.set_halign(gtk::Align::Center);
    i.set_valign(gtk::Align::Center);
    slot.append(&i);
    slot.upcast()
}

/// "Ctrl+Enter" as keycaps.
pub fn keys(combo: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    b.set_hexpand(false);
    b.add_css_class("keys");
    b.set_valign(gtk::Align::Center);
    for k in combo.split('+') {
        let cap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        cap.add_css_class("kbd");
        cap.set_valign(gtk::Align::Center);
        let content: gtk::Widget = match k.to_lowercase().as_str() {
            "enter" => glyph("corner-down-left", 12).upcast(),
            "up" => glyph("arrow-up", 12).upcast(),
            "down" => glyph("arrow-down", 12).upcast(),
            other => gtk::Label::new(Some(match other {
                "ctrl" => "Ctrl",
                "alt" => "Alt",
                "shift" => "Shift",
                "esc" => "esc",
                "tab" => "Tab",
                "space" => "Space",
                _ => k,
            }))
            .upcast(),
        };
        content.set_hexpand(true);
        content.set_halign(gtk::Align::Center);
        content.set_valign(gtk::Align::Center);
        cap.set_hexpand(false);
        cap.append(&content);
        b.append(&cap);
    }
    b
}

const MARK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 64 64"><defs><linearGradient id="g" x1="8" y1="6" x2="58" y2="60" gradientUnits="userSpaceOnUse"><stop stop-color="#9d8cff"/><stop offset="0.55" stop-color="#6c5cff"/><stop offset="1" stop-color="#ff6fb5"/></linearGradient><radialGradient id="s" cx="0" cy="0" r="1" gradientTransform="translate(22 16) rotate(50) scale(40)" gradientUnits="userSpaceOnUse"><stop stop-color="white" stop-opacity="0.55"/><stop offset="1" stop-color="white" stop-opacity="0"/></radialGradient></defs><rect x="4" y="4" width="56" height="56" rx="17" fill="url(#g)"/><rect x="4" y="4" width="56" height="56" rx="17" fill="url(#s)"/><rect x="4.5" y="4.5" width="55" height="55" rx="16.5" fill="none" stroke="white" stroke-opacity="0.35"/><path d="M32 15c1.4 9.2 7.8 15.6 17 17-9.2 1.4-15.6 7.8-17 17-1.4-9.2-7.8-15.6-17-17 9.2-1.4 15.6-7.8 17-17Z" fill="white"/><circle cx="45" cy="19" r="2.6" fill="white" fill-opacity="0.85"/></svg>"##;

thread_local! {
    static MARK: RefCell<Option<gdk::Texture>> = const { RefCell::new(None) };
}

/// The Lumen mark: a glass lens with a four-point glint.
pub fn mark(size: i32) -> gtk::Widget {
    let tex = MARK.with(|m| {
        if m.borrow().is_none() {
            *m.borrow_mut() = gdk::Texture::from_bytes(&glib::Bytes::from_static(MARK_SVG.as_bytes())).ok();
        }
        m.borrow().clone()
    });
    let img = match tex {
        Some(t) => gtk::Image::from_paintable(Some(&t)),
        None => gtk::Image::from_icon_name(lumen_core::constants::APP_ID),
    };
    img.set_pixel_size(size);
    img.add_css_class("mark");
    img.upcast()
}

pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    if !class.is_empty() {
        for c in class.split_whitespace() {
            l.add_css_class(c);
        }
    }
    l.set_xalign(0.0);
    l
}

/// A label that ends in "…" when it doesn't fit.
pub fn ellipsized(text: &str, class: &str) -> gtk::Label {
    let l = label(text, class);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    l.set_single_line_mode(true);
    l.set_max_width_chars(1);
    l.set_hexpand(true);
    l
}

pub fn hbox(spacing: i32, class: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, spacing);
    for c in class.split_whitespace() {
        b.add_css_class(c);
    }
    b
}

pub fn vbox(spacing: i32, class: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, spacing);
    for c in class.split_whitespace() {
        b.add_css_class(c);
    }
    b
}

/// A button with an optional leading glyph, a label and optional trailing keycaps.
pub fn button(label_text: &str, lead: Option<&'static str>, trail_keys: Option<&str>, class: &str) -> gtk::Button {
    let b = gtk::Button::new();
    b.add_css_class("btn");
    for c in class.split_whitespace() {
        b.add_css_class(c);
    }
    let content = hbox(8, "");
    content.set_halign(gtk::Align::Center);
    if let Some(g) = lead {
        content.append(&glyph(g, 15));
    }
    if !label_text.is_empty() {
        content.append(&gtk::Label::new(Some(label_text)));
    }
    if let Some(k) = trail_keys {
        content.append(&keys(k));
    }
    b.set_child(Some(&content));
    b.set_focus_on_click(false);
    b
}

/// Pango markup for a title with matched characters at full strength and the rest dimmed.
pub fn highlight_markup(text: &str, indexes: &[usize]) -> String {
    if indexes.is_empty() {
        return glib::markup_escape_text(text).to_string();
    }
    let mut out = String::new();
    let mut run = String::new();
    let mut run_on: Option<bool> = None;
    let flush = |out: &mut String, run: &mut String, on: Option<bool>| {
        if run.is_empty() {
            return;
        }
        let esc = glib::markup_escape_text(run);
        if on == Some(true) {
            out.push_str(&esc);
        } else {
            out.push_str(&format!("<span alpha=\"68%\">{esc}</span>"));
        }
        run.clear();
    };
    for (i, c) in text.chars().enumerate() {
        let on = indexes.contains(&i);
        if run_on != Some(on) {
            flush(&mut out, &mut run, run_on);
            run_on = Some(on);
        }
        run.push(c);
    }
    flush(&mut out, &mut run, run_on);
    out
}
