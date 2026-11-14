//! Names and identifiers. Rename the app in one place.

pub const APP_NAME: &str = "Lumen";
/// Also the D-Bus name and the GTK application id.
pub const APP_ID: &str = "io.github.lumenlauncher.Lumen";
pub const DBUS_NAME: &str = APP_ID;
pub const DBUS_PATH: &str = "/io/github/lumenlauncher/Lumen";
pub const DBUS_IFACE: &str = APP_ID;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO_URL: &str = "https://github.com/smith8h/lumen";

pub const DEFAULT_SHORTCUT: &str = "<Alt>space";

pub struct Engine {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
}

pub const SEARCH_ENGINES: &[Engine] = &[
    Engine { id: "google", name: "Google", url: "https://www.google.com/search?q=%s" },
    Engine { id: "duckduckgo", name: "DuckDuckGo", url: "https://duckduckgo.com/?q=%s" },
    Engine { id: "brave", name: "Brave Search", url: "https://search.brave.com/search?q=%s" },
    Engine { id: "bing", name: "Bing", url: "https://www.bing.com/search?q=%s" },
    Engine { id: "startpage", name: "Startpage", url: "https://www.startpage.com/do/search?q=%s" },
];

/// Type `yt cats` to search YouTube for cats, etc.
pub const WEB_KEYWORDS: &[(&str, &str, &str)] = &[
    ("g", "Google", "https://www.google.com/search?q=%s"),
    ("ddg", "DuckDuckGo", "https://duckduckgo.com/?q=%s"),
    ("yt", "YouTube", "https://www.youtube.com/results?search_query=%s"),
    ("gh", "GitHub", "https://github.com/search?q=%s&type=repositories"),
    ("wiki", "Wikipedia", "https://en.wikipedia.org/wiki/Special:Search?search=%s"),
    ("maps", "Google Maps", "https://www.google.com/maps/search/%s"),
    ("npm", "npm", "https://www.npmjs.com/search?q=%s"),
    ("so", "Stack Overflow", "https://stackoverflow.com/search?q=%s"),
    ("tr", "Google Translate", "https://translate.google.com/?sl=auto&tl=en&text=%s"),
    ("img", "Google Images", "https://www.google.com/search?tbm=isch&q=%s"),
    ("reddit", "Reddit", "https://www.reddit.com/search/?q=%s"),
    ("mdn", "MDN", "https://developer.mozilla.org/en-US/search?q=%s"),
];
