//! Lumen: a fast, glassy, Spotlight-style launcher for Linux.

mod app;
mod clipboard;
mod dbus;
mod desktop;
mod paste;
mod theme;
mod ui;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use lumen_core::constants::{APP_ID, APP_NAME, VERSION};
use std::cell::RefCell;
use std::rc::Rc;

use app::{App, BusHandler, ShowOpts, View};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct LumenApplication {
        pub registration: RefCell<Option<gio::RegistrationId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LumenApplication {
        const NAME: &'static str = "LumenApplication";
        type Type = super::LumenApplication;
        type ParentType = gtk::Application;
    }

    impl ObjectImpl for LumenApplication {}

    impl ApplicationImpl for LumenApplication {
        // Export Lumen's interface before the bus name is taken: clients (like the GNOME
        // extension) call in as soon as it appears.
        fn dbus_register(&self, connection: &gio::DBusConnection, object_path: &str) -> Result<(), glib::Error> {
            self.parent_dbus_register(connection, object_path)?;
            match dbus::register(connection) {
                Ok(id) => {
                    self.registration.replace(Some(id));
                }
                Err(e) => eprintln!("[dbus] couldn't export the Lumen interface: {}", e.message()),
            }
            Ok(())
        }

        fn dbus_unregister(&self, connection: &gio::DBusConnection, object_path: &str) {
            if let Some(id) = self.registration.take() {
                dbus::unregister(connection, id);
            }
            self.parent_dbus_unregister(connection, object_path);
        }
    }

    impl GtkApplicationImpl for LumenApplication {}
}

glib::wrapper! {
    pub struct LumenApplication(ObjectSubclass<imp::LumenApplication>)
        @extends gio::Application, gtk::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

const HELP: &str = "Usage: lumen [OPTION]

  --toggle            Open Lumen, or close it if it's open (bind your shortcut to this)
  --hidden            Start in the background without showing the panel
  --setup-shortcut    Set up Alt+Space (GNOME)
  --remove-shortcut   Remove the shortcut and restore GNOME's own Alt+Space
  --quit              Quit the running Lumen
  --version           Print the version
";

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().skip(1).any(|a| a == f);

    // Maintenance flags work without starting (or talking to) the app.
    if has("--help") || has("-h") {
        print!("{HELP}");
        return glib::ExitCode::SUCCESS;
    }
    if has("--version") {
        println!("{APP_NAME} {VERSION}");
        return glib::ExitCode::SUCCESS;
    }
    if has("--setup-shortcut") {
        let s = lumen_core::shortcut::setup();
        let mut settings = lumen_core::store::Settings::load();
        settings.first_run_done = true;
        let _ = settings.save();
        if s.configured {
            println!("Shortcut ready: {}", lumen_core::shortcut::pretty_binding(&s.binding));
        } else {
            println!("Couldn't set up the shortcut on {}. Bind this command yourself:\n{}", s.desktop, s.command);
        }
        return glib::ExitCode::SUCCESS;
    }
    if has("--remove-shortcut") {
        lumen_core::shortcut::remove();
        println!("Shortcut removed");
        return glib::ExitCode::SUCCESS;
    }
    if has("--quit") {
        if !dbus::call_running("Quit") {
            eprintln!("Lumen isn't running");
        }
        return glib::ExitCode::SUCCESS;
    }
    if let Some(unknown) = args.iter().skip(1).find(|a| !["--toggle", "--hidden"].contains(&a.as_str())) {
        eprintln!("Unknown option {unknown}\n\n{HELP}");
        return glib::ExitCode::FAILURE;
    }

    let gtk_app: LumenApplication =
        glib::Object::builder().property("application-id", APP_ID).property("flags", gio::ApplicationFlags::HANDLES_COMMAND_LINE).build();
    glib::set_application_name(APP_NAME);
    let gtk_app: gtk::Application = gtk_app.upcast();

    let app = App::new(&gtk_app);
    let handler: Rc<dyn dbus::Handler> = Rc::new(BusHandler(Rc::downgrade(&app)));
    dbus::set_handler(Rc::downgrade(&handler));

    let a = app.clone();
    gtk_app.connect_startup(move |g| {
        // Stay resident: Lumen lives in the background between Alt+Space presses.
        std::mem::forget(g.hold());
        a.startup();
    });

    let a = app.clone();
    gtk_app.connect_command_line(move |_, cmd| {
        let args: Vec<String> = cmd.arguments().iter().skip(1).map(|s| s.to_string_lossy().into_owned()).collect();
        let has = |f: &str| args.iter().any(|a| a == f);
        if cmd.is_remote() {
            // Another `lumen` was started: it forwards its arguments here and exits.
            if has("--hidden") {
                return glib::ExitCode::SUCCESS;
            }
            if has("--toggle") {
                a.toggle();
            } else {
                a.show(ShowOpts::default());
            }
            return glib::ExitCode::SUCCESS;
        }
        let first = !a.core.settings().first_run_done;
        if first {
            a.show(ShowOpts { query: None, view: Some(View::Shortcut) });
        } else if has("--hidden") {
            a.prime_clipboard();
        } else {
            a.show(ShowOpts::default());
        }
        glib::ExitCode::SUCCESS
    });

    gtk_app.connect_activate(|_| {});
    let a = app.clone();
    gtk_app.connect_shutdown(move |_| a.core.flush());

    // GApplication must not parse Lumen's own flags; they were checked above.
    let code = gtk_app.run_with_args(&args);
    drop(handler);
    code
}
