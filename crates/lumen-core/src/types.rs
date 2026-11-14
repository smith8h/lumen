//! Data shared by the search providers and the UI.

/// What a result is. Decides its section, its primary action label and its footer hints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResultKind {
    App,
    File,
    Folder,
    Calc,
    Url,
    Web,
    Command,
    System,
    Lumen,
    Clip,
}

/// Built-in icons, drawn as colourful tiles (Lucide line icons).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    Calculator,
    Globe,
    Search,
    Terminal,
    Lock,
    Logout,
    Moon,
    Sun,
    Power,
    Restart,
    Settings,
    Folder,
    File,
    Trash,
    Wifi,
    Bell,
    Refresh,
    Quit,
    Sparkles,
    Copy,
    Keyboard,
    Clipboard,
    Pin,
    Paste,
}

/// How an icon should be drawn. The UI resolves names and file types with the user's icon theme.
#[derive(Clone, Debug, PartialEq)]
pub enum IconSpec {
    /// An icon theme name, e.g. "firefox" or "org.gnome.Nautilus".
    Named(String),
    /// An image file drawn as-is: absolute app icons and image thumbnails.
    File(String),
    /// The icon the file manager would show for this file's type.
    FileType {
        path: String,
        dir: bool,
    },
    Glyph(Glyph),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    Terminal,
    Inline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemOp {
    Lock,
    Logout,
    Suspend,
    Restart,
    Shutdown,
    DarkMode,
    LightMode,
    WifiToggle,
    DndToggle,
    Settings,
    EmptyTrash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LumenOp {
    Settings,
    Quit,
    Reindex,
    Shortcut,
}

/// Everything Lumen can do on behalf of a result.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    LaunchApp {
        desktop_id: String,
        action_id: Option<String>,
    },
    OpenPath(String),
    RevealPath(String),
    OpenTerminalAt(String),
    Copy(String),
    OpenUrl(String),
    RunCommand {
        command: String,
        mode: RunMode,
    },
    System(SystemOp),
    Lumen(LumenOp),
    /// Paste a clip into the previous app. `terminal` sends Ctrl+Shift+V instead of Ctrl+V.
    ClipPaste {
        id: String,
        terminal: Option<bool>,
    },
    ClipCopy(String),
    ClipPin {
        id: String,
        pinned: bool,
    },
    ClipDelete(String),
    ClipClear,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryAction {
    pub label: String,
    /// Keyboard hint rendered as keycaps, e.g. "Ctrl+Enter".
    pub hint: Option<String>,
    pub action: Action,
    pub glyph: Option<Glyph>,
}

impl SecondaryAction {
    pub fn new(label: impl Into<String>, action: Action) -> Self {
        Self { label: label.into(), hint: None, action, glyph: None }
    }
    pub fn hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }
    pub fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultItem {
    pub id: String,
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<IconSpec>,
    /// Ranking score; higher is better.
    pub score: f64,
    /// Character indexes into `title` that matched the query, for highlighting.
    pub matches: Vec<usize>,
    pub action: Action,
    pub actions: Vec<SecondaryAction>,
    /// Ask for a second Enter before running (power actions).
    pub confirm: Option<String>,
    /// Key used for frecency ranking.
    pub history_key: Option<String>,
}

impl ResultItem {
    pub fn new(id: impl Into<String>, kind: ResultKind, title: impl Into<String>, action: Action) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            subtitle: None,
            icon: None,
            score: 0.0,
            matches: Vec::new(),
            action,
            actions: Vec::new(),
            confirm: None,
            history_key: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SectionId {
    Top,
    Calc,
    Clipboard,
    Apps,
    Commands,
    Files,
    Folders,
    Web,
    System,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultSection {
    pub id: SectionId,
    pub title: String,
    pub items: Vec<ResultItem>,
}

/// What mode a query triggered, shown as a chip in the search bar.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchMode {
    pub label: String,
    pub glyph: Glyph,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResponse {
    pub seq: u64,
    pub query: String,
    pub sections: Vec<ResultSection>,
    pub mode: Option<SearchMode>,
    pub took_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FolderEntry {
    pub name: String,
    pub dir: bool,
}

/// Details for the preview pane. Times are milliseconds since the epoch.
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    App { name: String, comment: Option<String>, icon: Option<IconSpec>, exec: String, categories: Vec<String>, path: String },
    Image { path: String, name: String, size: u64, modified: i64 },
    Text { path: String, name: String, size: u64, modified: i64, content: String },
    Folder { path: String, name: String, modified: i64, entries: Vec<FolderEntry>, total: usize },
    File { path: String, name: String, size: u64, modified: i64, icon: IconSpec },
    Calc { expression: String, result: String, detail: Option<String> },
    Clip { text: String, truncated: bool, length: usize, lines: usize, first_at: i64, last_at: i64, pinned: bool },
    None,
}
