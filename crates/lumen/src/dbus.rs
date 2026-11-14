//! Lumen's D-Bus interface, on the same name and path as the GTK application, so the
//! keyboard shortcut can toggle Lumen in a few milliseconds (`gdbus call … Toggle`).
//!
//! It also talks to the optional GNOME Shell extension (gnome-extension/):
//!   • signal PanelGeometry(x, y, w, h, radius): where the panel is inside Lumen's window, so
//!     the extension can blur exactly that area. All zeros = panel closed.
//!   • property ShellBlur (read/write bool): the extension sets it while it draws real blur,
//!     and Lumen makes its own tint more see-through.
//!   • method PreviousWindow(wm_class, is_x11): which window had focus before the panel, so
//!     pasting a clip into a terminal uses Ctrl+Shift+V and XWayland windows can be reached.
//!   • method ClipboardSecret(): a copy was tagged as a secret by a password manager, so
//!     clipboard history skips it. Never carries the text.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use lumen_core::constants::{DBUS_IFACE, DBUS_PATH};

pub const INTROSPECTION: &str = r#"
<node>
  <interface name="io.github.lumenlauncher.Lumen">
    <method name="Toggle"/>
    <method name="Show"><arg type="s" name="query" direction="in"/></method>
    <method name="Hide"/>
    <method name="Quit"/>
    <method name="PreviousWindow">
      <arg type="s" name="wm_class" direction="in"/>
      <arg type="b" name="is_x11" direction="in"/>
    </method>
    <method name="ClipboardSecret"/>
    <method name="Capture"><arg type="s" name="path" direction="in"/></method>
    <method name="Key"><arg type="s" name="combo" direction="in"/></method>
    <property name="ShellBlur" type="b" access="readwrite"/>
    <signal name="PanelGeometry">
      <arg type="i" name="x"/><arg type="i" name="y"/><arg type="i" name="width"/>
      <arg type="i" name="height"/><arg type="i" name="radius"/>
    </signal>
  </interface>
</node>"#;

/// What the bus can ask of Lumen.
pub trait Handler {
    fn toggle(&self);
    fn show_query(&self, query: String);
    fn hide(&self);
    fn quit(&self);
    fn previous_window(&self, wm_class: String, x11: bool);
    fn clipboard_secret(&self);
    fn shell_blur(&self) -> bool;
    fn set_shell_blur(&self, on: bool);
    /// Development only: save a PNG of the panel.
    fn capture(&self, path: String) -> Result<(), String>;
    /// Development only: press keys in the panel, e.g. "Down", "Enter", "ctrl+K".
    fn key(&self, combo: String) -> Result<(), String>;
}

thread_local! {
    static HANDLER: std::cell::RefCell<Option<std::rc::Weak<dyn Handler>>> = const { std::cell::RefCell::new(None) };
}

pub fn set_handler(h: std::rc::Weak<dyn Handler>) {
    HANDLER.with(|c| c.replace(Some(h)));
}

fn with<R>(f: impl FnOnce(&dyn Handler) -> R) -> Option<R> {
    let h = HANDLER.with(|c| c.borrow().as_ref().and_then(|w| w.upgrade()))?;
    Some(f(&*h))
}

thread_local! {
    static CONN: std::cell::RefCell<Option<gio::DBusConnection>> = const { std::cell::RefCell::new(None) };
}

pub fn register(conn: &gio::DBusConnection) -> Result<gio::RegistrationId, glib::Error> {
    CONN.with(|c| c.replace(Some(conn.clone())));
    let node = gio::DBusNodeInfo::for_xml(INTROSPECTION)?;
    let iface = node.lookup_interface(DBUS_IFACE).ok_or_else(|| glib::Error::new(gio::IOErrorEnum::Failed, "missing interface"))?;
    conn.register_object(DBUS_PATH, &iface)
        .method_call(|_conn, _sender, _path, _iface, method, params, invocation| {
            let ok = |inv: gio::DBusMethodInvocation| inv.return_value(None);
            match method {
                "Toggle" => {
                    with(|h| h.toggle());
                    ok(invocation)
                }
                "Show" => {
                    let (q,) = params.get::<(String,)>().unwrap_or_default();
                    with(|h| h.show_query(q));
                    ok(invocation)
                }
                "Hide" => {
                    with(|h| h.hide());
                    ok(invocation)
                }
                "Quit" => {
                    ok(invocation);
                    with(|h| h.quit());
                }
                "PreviousWindow" => {
                    if let Some((class, x11)) = params.get::<(String, bool)>()
                        && class.len() <= 256
                    {
                        with(|h| h.previous_window(class, x11));
                    }
                    ok(invocation)
                }
                "ClipboardSecret" => {
                    with(|h| h.clipboard_secret());
                    ok(invocation)
                }
                "Capture" | "Key" => {
                    let (arg,) = params.get::<(String,)>().unwrap_or_default();
                    let res = with(|h| if method == "Capture" { h.capture(arg) } else { h.key(arg) }).unwrap_or(Err("not ready".into()));
                    match res {
                        Ok(()) => ok(invocation),
                        Err(e) => invocation.return_dbus_error("io.github.lumenlauncher.Lumen.Error", &e),
                    }
                }
                _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", "Unknown method"),
            }
        })
        .property(|_conn, _sender, _path, _iface, prop| match prop {
            "ShellBlur" => with(|h| h.shell_blur()).unwrap_or(false).to_variant(),
            _ => false.to_variant(),
        })
        .set_property(|conn, _sender, path, iface, prop, value| {
            if prop != "ShellBlur" {
                return false;
            }
            let Some(on) = value.get::<bool>() else { return false };
            let changed = with(|h| {
                let changed = h.shell_blur() != on;
                h.set_shell_blur(on);
                changed
            })
            .unwrap_or(false);
            if changed {
                let changed_props = glib::VariantDict::new(None);
                changed_props.insert_value("ShellBlur", &on.to_variant());
                let args = (iface, changed_props.end(), Vec::<String>::new()).to_variant();
                let _ = conn.emit_signal(None, path, "org.freedesktop.DBus.Properties", "PropertiesChanged", Some(&args));
            }
            true
        })
        .build()
}

pub fn unregister(conn: &gio::DBusConnection, id: gio::RegistrationId) {
    let _ = conn.unregister_object(id);
    CONN.with(|c| c.replace(None));
}

/// Broadcast the panel's rectangle (None when it's closed) for the GNOME extension.
pub fn emit_geometry(rect: Option<(i32, i32, i32, i32, i32)>) {
    let Some(conn) = CONN.with(|c| c.borrow().clone()) else { return };
    let r = rect.unwrap_or((0, 0, 0, 0, 0));
    let _ = conn.emit_signal(None, DBUS_PATH, DBUS_IFACE, "PanelGeometry", Some(&r.to_variant()));
}

/// Call a method on the running Lumen (for `lumen --quit`). Returns false when none is running.
pub fn call_running(method: &str) -> bool {
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else { return false };
    bus.call_sync(
        Some(lumen_core::constants::DBUS_NAME),
        DBUS_PATH,
        DBUS_IFACE,
        method,
        None,
        None,
        gio::DBusCallFlags::NO_AUTO_START,
        2000,
        None::<&gio::Cancellable>,
    )
    .is_ok()
}
