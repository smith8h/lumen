//! The desktop side of things through GIO: installed apps, launching, opening files and links.

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumen_core::apps::{AppAction, AppEntry, parse_exec};
use lumen_core::types::IconSpec;

/// Read every app the desktop would show into plain entries for the core.
pub fn load_apps() -> Vec<AppEntry> {
    let mut out = Vec::new();
    for info in gio::AppInfo::all() {
        if !info.should_show() {
            continue;
        }
        let Ok(d) = info.downcast::<gio_unix::DesktopAppInfo>() else { continue };
        let Some(id) = d.id().map(|s| s.to_string()) else { continue };
        let Some(path) = d.filename().map(|p| p.to_string_lossy().into_owned()) else { continue };
        let exec = d.string("Exec").map(|s| s.to_string()).unwrap_or_default();
        if exec.is_empty() && !d.boolean("DBusActivatable") {
            continue;
        }
        let icon = d.icon().and_then(|i| icon_spec(&i));
        let actions = d
            .list_actions()
            .iter()
            .map(|a| AppAction { id: a.to_string(), name: d.action_name(a).to_string() })
            .filter(|a| !a.name.is_empty())
            .collect();
        out.push(AppEntry {
            id,
            path,
            name: d.display_name().to_string(),
            generic_name: d.generic_name().map(|s| s.to_string()).filter(|s| !s.is_empty()),
            comment: d.description().map(|s| s.to_string()).filter(|s| !s.is_empty()),
            keywords: d.keywords().iter().map(|k| k.to_string()).collect(),
            categories: d.categories().map(|c| c.split(';').filter(|s| !s.is_empty()).map(String::from).collect()).unwrap_or_default(),
            exec,
            icon,
            terminal: d.boolean("Terminal"),
            actions,
        });
    }
    out
}

fn icon_spec(icon: &gio::Icon) -> Option<IconSpec> {
    if let Some(t) = icon.downcast_ref::<gio::ThemedIcon>() {
        return t.names().first().map(|n| IconSpec::Named(n.to_string()));
    }
    if let Some(f) = icon.downcast_ref::<gio::FileIcon>() {
        return f.file().path().map(|p| IconSpec::File(p.to_string_lossy().into_owned()));
    }
    None
}

/// A launch context that hands the new app an activation token (so it gets focus on Wayland)
/// and strips variables that only describe how Lumen itself was started.
pub fn launch_context() -> gio::AppLaunchContext {
    let ctx: gio::AppLaunchContext = match gdk::Display::default() {
        Some(d) => d.app_launch_context().upcast(),
        None => gio::AppLaunchContext::new(),
    };
    for name in lumen_core::env::dropped_env_names() {
        ctx.unsetenv(name);
    }
    ctx
}

pub fn launch_app(app: &AppEntry, action: Option<&str>, terminal_settings: &lumen_core::store::Settings) -> Result<(), String> {
    let Some(d) = gio_unix::DesktopAppInfo::from_filename(&app.path) else { return Err("App no longer installed".into()) };
    let ctx = launch_context();
    if app.terminal {
        // Terminal apps go to the terminal the user picked, not GLib's guess.
        let exec = action
            .and_then(|a| {
                let kf = glib::KeyFile::new();
                kf.load_from_file(&app.path, glib::KeyFileFlags::NONE).ok()?;
                kf.string(&format!("Desktop Action {a}"), "Exec").ok().map(|s| s.to_string())
            })
            .unwrap_or_else(|| app.exec.clone());
        let icon = match &app.icon {
            Some(IconSpec::Named(n)) => Some(n.as_str()),
            _ => None,
        };
        let argv = parse_exec(&exec, app, icon);
        if argv.is_empty() {
            return Err("Invalid Exec line".into());
        }
        let cmd = argv.iter().map(|a| lumen_core::env::shell_quote(a)).collect::<Vec<_>>().join(" ");
        let cwd = d.string("Path").map(|s| s.to_string()).filter(|p| std::path::Path::new(p).is_dir());
        return lumen_core::terminal::open_in_terminal(terminal_settings, Some(&cmd), cwd.as_deref());
    }
    match action {
        Some(a) => {
            d.launch_action(a, Some(&ctx));
            Ok(())
        }
        None => d.launch(&[], Some(&ctx)).map_err(|e| e.message().to_string()),
    }
}

pub fn open_uri(uri: &str) -> Result<(), String> {
    gio::AppInfo::launch_default_for_uri(uri, Some(&launch_context())).map_err(|e| e.message().to_string())
}

pub fn open_path(path: &str) -> Result<(), String> {
    if !std::path::Path::new(path).exists() {
        return Err("File no longer exists".into());
    }
    open_uri(&gio::File::for_path(path).uri())
}

const SAFE_SCHEMES: &[&str] = &["http:", "https:", "ftp:", "mailto:"];

pub fn open_url(url: &str) -> Result<(), String> {
    if !SAFE_SCHEMES.iter().any(|s| url.to_lowercase().starts_with(s)) {
        return Err("Unsupported link".into());
    }
    open_uri(url)
}

/// Show a file selected in the file manager (org.freedesktop.FileManager1), or open its folder.
pub fn reveal_path(path: &str) {
    let uri = gio::File::for_path(path).uri().to_string();
    let parent = lumen_core::files::dirname(path);
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else {
        let _ = open_path(&parent);
        return;
    };
    bus.call(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        "org.freedesktop.FileManager1",
        "ShowItems",
        Some(&(vec![uri], "").to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        3000,
        None::<&gio::Cancellable>,
        move |res| {
            if res.is_err() {
                let _ = open_path(&parent);
            }
        },
    );
}
