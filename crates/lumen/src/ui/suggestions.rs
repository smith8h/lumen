//! What you see before typing: your most used apps, recent files and a few tips.

use gtk::prelude::*;
use lumen_core::types::{Glyph, ResultSection, SectionId};
use std::rc::Weak;

use super::Panel;
use super::bits::{ellipsized, hbox, icon, label, vbox};
use super::results::{ResultList, RowEvents, attach_pointer};

const TIPS: &[(&str, &str)] = &[("=", "calculate"), (">", "run a command"), ("?", "search the web"), (";", "clipboard"), ("~/", "browse files")];

pub struct Suggestions {
    pub root: gtk::Widget,
    tiles: Vec<(gtk::Box, gtk::Box)>,
    list: Option<ResultList>,
}

impl Suggestions {
    pub fn new(sections: &[ResultSection], ev: &RowEvents, panel: Weak<Panel>) -> Suggestions {
        let root = vbox(0, "suggest");
        let (apps, rest): (Option<&ResultSection>, &[ResultSection]) = match sections.first() {
            Some(s) if s.id == SectionId::Apps => (Some(s), &sections[1..]),
            _ => (None, sections),
        };
        let mut tiles = Vec::new();
        if let Some(apps) = apps {
            root.append(&label(&apps.title, "section-title"));
            let grid = gtk::Grid::new();
            grid.set_column_homogeneous(true);
            grid.set_column_spacing(4);
            grid.set_margin_start(4);
            grid.set_margin_end(4);
            grid.set_margin_bottom(8);
            // At least six columns, so a short list doesn't stretch into giant tiles.
            let cols = apps.items.len().max(6) as i32;
            for (i, item) in apps.items.iter().enumerate() {
                let tile = vbox(8, "tile-app");
                let ic = icon(item.icon.as_ref(), 46, false, Glyph::Sparkles);
                ic.set_halign(gtk::Align::Center);
                tile.append(&ic);
                let name = ellipsized(&item.title, "");
                name.set_xalign(0.5);
                tile.append(&name);
                tile.set_tooltip_text(Some(&match &item.subtitle {
                    Some(s) => format!("{}, {s}", item.title),
                    None => item.title.clone(),
                }));
                // The light sits along the bottom edge of a selected tile.
                let light = hbox(0, "light bottom");
                light.set_size_request(22, 3);
                light.set_halign(gtk::Align::Center);
                light.set_valign(gtk::Align::End);
                light.set_opacity(0.0);
                let ov = gtk::Overlay::new();
                ov.set_child(Some(&tile));
                ov.add_overlay(&light);
                attach_pointer(&ov, i, ev);
                grid.attach(&ov, i as i32 % cols, 0, 1, 1);
                tiles.push((tile, light));
            }
            for i in apps.items.len() as i32..cols {
                let filler = gtk::Box::new(gtk::Orientation::Vertical, 0);
                grid.attach(&filler, i, 0, 1, 1);
            }
            root.append(&grid);
        }
        let offset = tiles.len();
        let list = (!rest.is_empty()).then(|| {
            let l = ResultList::new(rest, offset, 0, ev);
            root.append(&l.root);
            l
        });

        let tips = hbox(4, "tips");
        let lead = label("Start with", "lead-in");
        lead.set_margin_start(4);
        lead.set_margin_end(6);
        tips.append(&lead);
        for (code, text) in TIPS {
            let b = gtk::Button::new();
            b.add_css_class("tip");
            b.set_focus_on_click(false);
            let content = hbox(6, "");
            content.append(&label(code, "code"));
            content.append(&gtk::Label::new(Some(text)));
            b.set_child(Some(&content));
            let prefix = if *code == "~/" { "~/".to_string() } else { format!("{code} ") };
            let p = panel.clone();
            b.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.tip(&prefix);
                }
            });
            tips.append(&b);
        }
        root.append(&tips);

        let sw = gtk::ScrolledWindow::new();
        sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        sw.set_propagate_natural_height(true);
        sw.set_max_content_height(470);
        sw.set_child(Some(&root));
        Suggestions { root: sw.upcast(), tiles, list }
    }

    pub fn set_selected(&self, i: usize) {
        for (n, (tile, light)) in self.tiles.iter().enumerate() {
            if n == i {
                tile.add_css_class("selected");
                light.set_opacity(1.0);
            } else {
                tile.remove_css_class("selected");
                light.set_opacity(0.0);
            }
        }
        if let Some(l) = &self.list {
            l.set_selected(Some(i));
        }
    }
}
