//! Inline command output ("> ls -la").

use gtk::glib;
use gtk::prelude::*;
use regex::Regex;
use std::rc::{Rc, Weak};
use std::sync::LazyLock;
use std::time::Instant;

use super::Panel;
use super::bits::{button, ellipsized, hbox, label, vbox};

pub struct RunState {
    pub pid: u32,
    pub command: String,
    pub done: bool,
    pub exit_code: Option<i32>,
    pub started: Instant,
    pub ended: Option<Instant>,
    pub text: String,
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07|\r").unwrap());

pub fn strip_ansi(s: &str) -> String {
    ANSI.replace_all(&s.replace("\r\n", "\n"), "").into_owned()
}

pub struct OutputView {
    pub root: gtk::Widget,
    status: gtk::Box,
    buttons: gtk::Box,
    view: gtk::TextView,
    scroller: gtk::ScrolledWindow,
    err_tag: gtk::TextTag,
    dim_tag: gtk::TextTag,
    placeholder: std::cell::Cell<bool>,
    panel: Weak<Panel>,
}

impl OutputView {
    pub fn new(panel: &Rc<Panel>, command: &str) -> Rc<OutputView> {
        let root = vbox(10, "output");
        root.set_size_request(-1, super::BODY_MAX);
        let header = hbox(10, "");
        let cmd = hbox(6, "cmd");
        cmd.set_hexpand(true);
        cmd.append(&label("❯", "prompt"));
        cmd.append(&ellipsized(command, ""));
        header.append(&cmd);
        let status = hbox(6, "status-pill");
        status.set_valign(gtk::Align::Center);
        header.append(&status);
        root.append(&header);

        let view = gtk::TextView::new();
        view.set_editable(false);
        view.set_cursor_visible(false);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_monospace(true);
        let buf = view.buffer();
        let err_tag = buf.create_tag(Some("err"), &[("foreground", &"#ff8a96")]).unwrap();
        let dim_tag = buf.create_tag(Some("dim"), &[("foreground-rgba", &gtk::gdk::RGBA::new(0.6, 0.62, 0.7, 0.8))]).unwrap();
        let scroller = gtk::ScrolledWindow::new();
        scroller.add_css_class("console");
        scroller.set_vexpand(true);
        scroller.set_child(Some(&view));
        root.append(&scroller);

        let buttons = hbox(8, "");
        root.append(&buttons);
        let v = Rc::new(OutputView {
            root: root.upcast(),
            status,
            buttons,
            view,
            scroller,
            err_tag,
            dim_tag,
            placeholder: std::cell::Cell::new(true),
            panel: Rc::downgrade(panel),
        });
        let end = buf.end_iter();
        buf.insert_with_tags(&mut end.clone(), "Waiting for output…", &[&v.dim_tag]);
        v
    }

    pub fn has_selection(&self) -> bool {
        self.view.buffer().has_selection()
    }

    /// Follow the output while scrolled to the bottom.
    fn at_bottom(&self) -> bool {
        let adj = self.scroller.vadjustment();
        adj.upper() - adj.value() - adj.page_size() < 24.0
    }

    pub fn append(&self, text: &str, stderr: bool) {
        let buf = self.view.buffer();
        if self.placeholder.replace(false) {
            buf.set_text("");
        }
        let stick = self.at_bottom();
        let mut end = buf.end_iter();
        if stderr {
            buf.insert_with_tags(&mut end, text, &[&self.err_tag]);
        } else {
            buf.insert(&mut end, text);
        }
        if stick {
            let adj = self.scroller.vadjustment();
            glib::idle_add_local_once(move || adj.set_value(adj.upper() - adj.page_size()));
        }
    }

    pub fn refresh(&self, run: &RunState) {
        while let Some(c) = self.status.first_child() {
            self.status.remove(&c);
        }
        let secs = run.ended.unwrap_or_else(Instant::now).duration_since(run.started).as_secs_f64();
        self.status.set_css_classes(&["status-pill"]);
        if !run.done {
            let spin = gtk::Spinner::new();
            spin.start();
            self.status.append(&spin);
            self.status.append(&gtk::Label::new(Some("Running")));
        } else if run.exit_code == Some(0) {
            self.status.add_css_class("ok");
            self.status.append(&gtk::Label::new(Some(&format!("Done · {secs:.1}s"))));
        } else {
            self.status.add_css_class("fail");
            let code = run.exit_code.map_or("killed".to_string(), |c| c.to_string());
            self.status.append(&gtk::Label::new(Some(&format!("Exit {code} · {secs:.1}s"))));
        }
        if run.done && self.placeholder.replace(false) {
            let buf = self.view.buffer();
            buf.set_text("");
            buf.insert_with_tags(&mut buf.end_iter(), "(no output)", &[&self.dim_tag]);
        }

        while let Some(c) = self.buttons.first_child() {
            self.buttons.remove(&c);
        }
        let p = self.panel.clone();
        let main = if run.done {
            let b = button("Run again", Some("rotate-ccw"), Some("Enter"), "small");
            b.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.rerun();
                }
            });
            b
        } else {
            let b = button("Stop", Some("square"), Some("Ctrl+C"), "small");
            b.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.stop_run();
                }
            });
            b
        };
        self.buttons.append(&main);
        let copy = button("Copy output", Some("copy"), None, "small");
        let p = self.panel.clone();
        copy.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                let t = p.output_text();
                if !t.is_empty() {
                    p.copy_text(&t);
                }
            }
        });
        self.buttons.append(&copy);
        let term = button("Open in terminal", Some("square-terminal"), Some("Ctrl+Enter"), "small ghost");
        let p = self.panel.clone();
        term.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.run_in_terminal();
            }
        });
        self.buttons.append(&term);
    }
}
