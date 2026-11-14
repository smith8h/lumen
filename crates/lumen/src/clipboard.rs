//! Clipboard history capture. Watches the system clipboard (signal plus a light poll, since
//! some compositors only report changes to the focused app) and hands new text to the core.
//! Clipboard contents are never logged.

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumen_core::Core;
use lumen_core::store::Settings;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(700);
/// A write Lumen made itself is ignored if it shows up again within this window.
const SELF_WRITE: Duration = Duration::from_secs(5);
/// A "that copy was a secret" report this close to a recorded copy refers to it.
const SECRET_WINDOW: Duration = Duration::from_secs(2);

#[derive(Default)]
pub struct ClipWatch {
    core: RefCell<Option<Arc<Core>>>,
    capturing: Cell<bool>,
    handler: RefCell<Option<(gdk::Clipboard, glib::SignalHandlerId)>>,
    poll: RefCell<Option<glib::SourceId>>,
    last: RefCell<Option<String>>,
    self_write: RefCell<Option<(String, Instant)>>,
    secret_at: Cell<Option<Instant>>,
    last_added: RefCell<Option<(String, Instant)>>,
    /// The read in flight, if any, and when it started (a read can stall; it's cancelled after a while).
    reading: RefCell<Option<(gtk::gio::Cancellable, Instant)>>,
}

const READ_TIMEOUT: Duration = Duration::from_secs(2);

fn trace(msg: &str) {
    if std::env::var_os("LUMEN_DEBUG_CLIPBOARD").is_some() {
        eprintln!("[clipboard] {msg}");
    }
}

fn wanted(s: &Settings) -> bool {
    s.clipboard_enabled && !s.clipboard_paused
}

fn clipboard() -> Option<gdk::Clipboard> {
    gdk::Display::default().map(|d| d.clipboard())
}

impl ClipWatch {
    pub fn new(core: Arc<Core>) -> Rc<ClipWatch> {
        let w = Rc::new(ClipWatch::default());
        w.core.replace(Some(core));
        w
    }

    fn core(&self) -> Arc<Core> {
        self.core.borrow().clone().expect("core set")
    }

    /// Follow settings: start or stop watching.
    pub fn apply(self: &Rc<Self>, s: &Settings) {
        if wanted(s) && !self.capturing.get() {
            self.start();
        } else if !wanted(s) && self.capturing.get() {
            self.stop();
        }
    }

    fn start(self: &Rc<Self>) {
        let Some(cb) = clipboard() else { return };
        self.capturing.set(true);
        let weak = Rc::downgrade(self);
        let id = cb.connect_changed(move |_| {
            if let Some(w) = weak.upgrade() {
                w.check();
            }
        });
        self.handler.replace(Some((cb, id)));
        let weak = Rc::downgrade(self);
        let poll = glib::timeout_add_local(POLL, move || match weak.upgrade() {
            Some(w) => {
                w.check();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
        self.poll.replace(Some(poll));
        // Take a baseline: what's already there isn't a new copy.
        self.check();
    }

    fn stop(&self) {
        self.capturing.set(false);
        if let Some((cb, id)) = self.handler.take() {
            cb.disconnect(id);
        }
        if let Some(p) = self.poll.take() {
            p.remove();
        }
        // Copies made while stopped are never recorded: the next start takes a fresh baseline.
        self.last.replace(None);
    }

    fn check(self: &Rc<Self>) {
        let Some(cb) = clipboard() else { return };
        if !self.capturing.get() {
            return;
        }
        if let Some((c, at)) = &*self.reading.borrow() {
            if at.elapsed() < READ_TIMEOUT {
                return;
            }
            trace("read stalled; cancelling");
            c.cancel();
        }
        self.reading.replace(None);
        if cb.is_local() {
            return; // Lumen's own write, or nothing seen yet
        }
        let formats = cb.formats();
        if !formats.contains_type(glib::Type::STRING)
            && !formats.contain_mime_type("text/plain;charset=utf-8")
            && !formats.contain_mime_type("text/plain")
        {
            return;
        }
        let cancel = gtk::gio::Cancellable::new();
        self.reading.replace(Some((cancel.clone(), Instant::now())));
        let weak = Rc::downgrade(self);
        cb.read_text_async(Some(&cancel), move |res| {
            let Some(w) = weak.upgrade() else { return };
            w.reading.replace(None);
            let text = match res {
                Ok(Some(t)) => t,
                Ok(None) => return,
                Err(e) => {
                    trace(&format!("read failed: {}", e.message()));
                    return;
                }
            };
            trace(&format!("read {} bytes", text.len()));
            let text = text.to_string();
            if !w.capturing.get() || w.last.borrow().as_deref() == Some(text.as_str()) {
                return;
            }
            let first = w.last.borrow().is_none();
            w.last.replace(Some(text.clone()));
            if !first {
                w.on_copy(&text);
            }
        });
    }

    fn on_copy(&self, text: &str) {
        if let Some((t, at)) = &*self.self_write.borrow()
            && at.elapsed() < SELF_WRITE
            && t == text
        {
            return;
        }
        let core = self.core();
        let s = core.settings();
        if !wanted(&s) {
            return;
        }
        if s.clipboard_ignore_secrets && self.secret_at.get().is_some_and(|t| t.elapsed() < SECRET_WINDOW) {
            return;
        }
        if let Some(e) = core.add_clip(text) {
            self.last_added.replace(Some((e.id, Instant::now())));
        }
    }

    /// The GNOME extension saw a copy tagged as a secret by a password manager. It arrives either
    /// just before Lumen notices the copy (then that copy is skipped) or just after (then the clip
    /// it produced is removed again).
    pub fn mark_secret(&self) {
        let core = self.core();
        if !core.settings().clipboard_ignore_secrets {
            return;
        }
        self.secret_at.set(Some(Instant::now()));
        if let Some((id, at)) = self.last_added.take()
            && at.elapsed() < SECRET_WINDOW
        {
            core.delete_clip(&id);
        }
    }

    /// Put text on the clipboard without recording it as a new copy.
    pub fn write(&self, text: &str) {
        self.self_write.replace(Some((text.to_string(), Instant::now())));
        if let Some(cb) = clipboard() {
            cb.set_text(text);
        }
    }
}
