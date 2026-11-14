//! The actions menu (Ctrl+K).

use gtk::prelude::*;
use lumen_core::types::SecondaryAction;
use std::rc::Rc;

use super::bits::{ellipsized, glyph, hbox, keys, lucide, vbox};

pub fn build(
    title: &str,
    actions: &[SecondaryAction],
    selected: usize,
    on_hover: Rc<dyn Fn(usize)>,
    on_run: Rc<dyn Fn(usize)>,
    moved: super::results::MovedFn,
) -> gtk::Widget {
    let m = vbox(0, "action-menu");
    m.set_halign(gtk::Align::End);
    m.set_valign(gtk::Align::End);
    m.set_size_request(290, -1);
    let t = ellipsized(title, "menu-title");
    m.append(&t);
    for (i, a) in actions.iter().enumerate() {
        let item = hbox(10, "item");
        let light = hbox(0, "light");
        light.set_size_request(3, 15);
        light.set_valign(gtk::Align::Center);
        light.set_opacity(if i == selected { 1.0 } else { 0.0 });
        item.append(&light);
        if i == selected {
            item.add_css_class("selected");
        }
        let g = glyph(a.glyph.map(lucide).unwrap_or("chevron-right"), 15);
        item.append(&g);
        let l = ellipsized(&a.label, "label");
        item.append(&l);
        if let Some(h) = &a.hint {
            item.append(&keys(h));
        }
        let motion = gtk::EventControllerMotion::new();
        let (hover, mv) = (on_hover.clone(), moved.clone());
        let w = item.clone().upcast::<gtk::Widget>();
        motion.connect_motion(move |_, x, y| {
            if mv(&w, x, y) {
                hover(i);
            }
        });
        item.add_controller(motion);
        let click = gtk::GestureClick::new();
        let run = on_run.clone();
        click.connect_released(move |g, _, _, _| {
            g.set_state(gtk::EventSequenceState::Claimed);
            run(i);
        });
        item.add_controller(click);
        m.append(&item);
    }
    m.upcast()
}
