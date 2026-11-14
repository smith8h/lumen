//! File search: an in-memory index of names built on a background thread, path browsing
//! ("~/Doc…"), recent files and previews.

use regex::Regex;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::{Instant, UNIX_EPOCH};

use crate::fuzzy::fuzzy_match;
use crate::store::{expand_tilde, tilde};
use crate::types::*;
use crate::xdg;

const MAX_ENTRIES: usize = 500_000;
const MAX_DEPTH: u8 = 14;
/// Images up to this size are shown as thumbnails and previews.
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;

pub struct IndexOptions {
    pub roots: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub include_hidden: bool,
}

pub struct FileHit {
    pub path: String,
    pub dir: bool,
    pub score: f64,
    pub indexes: Vec<usize>,
}

#[derive(Default)]
struct IndexData {
    names: Vec<Box<str>>,
    lower: Vec<Box<str>>,
    /// Index of the parent entry; negative values are roots (-1 → roots[0]).
    parent: Vec<i32>,
    depth: Vec<u8>,
    is_dir: Vec<bool>,
    roots: Vec<String>,
    recent: HashSet<String>,
}

#[derive(Default)]
pub struct FileIndex {
    data: RwLock<IndexData>,
    count: AtomicUsize,
    /// Bumped by every build; a build that has been overtaken stops early.
    generation: AtomicUsize,
}

static XBEL_HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"href="file://([^"]+)""#).unwrap());
static XBEL_ENTRY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<bookmark href="file://([^"]+)"[^>]*?modified="([^"]+)""#).unwrap());

fn xbel() -> Option<String> {
    fs::read_to_string(xdg::data_home().join("recently-used.xbel")).ok()
}

/// Decode %XX escapes in a file URI path.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn load_recent() -> HashSet<String> {
    xbel().map(|x| XBEL_HREF.captures_iter(&x).map(|c| percent_decode(&c[1])).collect()).unwrap_or_default()
}

const HOME_FOLDERS: &[&str] = &["Desktop", "Documents", "Downloads", "Music", "Pictures", "Videos", "Projects", "Public", "Templates"];

impl FileIndex {
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    /// Walk the roots breadth-first (so a huge deep folder can't starve shallow, more useful files)
    /// and swap the new table in. Returns the number of entries.
    pub fn build(&self, opts: &IndexOptions) -> usize {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut d = IndexData::default();
        let exclude_names: HashSet<&str> = opts.exclude_dirs.iter().filter(|e| !e.contains('/') && !e.starts_with('*')).map(String::as_str).collect();
        let exclude_suffixes: Vec<&str> = opts.exclude_dirs.iter().filter_map(|e| e.strip_prefix('*')).collect();
        let exclude_paths: Vec<&str> = opts.exclude_dirs.iter().filter(|e| e.contains('/')).map(|e| e.trim_end_matches('/')).collect();
        let mut seen = HashSet::new();
        d.roots = opts.roots.iter().filter(|r| seen.insert(r.as_str())).map(|r| r.trim_end_matches('/').to_string()).collect();

        let mut queue: VecDeque<(String, i32, u8, usize)> =
            d.roots.iter().enumerate().map(|(i, abs)| (if abs.is_empty() { "/".into() } else { abs.clone() }, -(i as i32 + 1), 0u8, i)).collect();
        while let Some((abs, id, level, root)) = queue.pop_front() {
            if d.names.len() >= MAX_ENTRIES || self.generation.load(Ordering::SeqCst) != generation {
                break;
            }
            let Ok(entries) = fs::read_dir(&abs) else { continue };
            for e in entries.flatten() {
                if d.names.len() >= MAX_ENTRIES {
                    break;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                if !opts.include_hidden && name.starts_with('.') {
                    continue;
                }
                let Ok(ft) = e.file_type() else { continue };
                let dir = ft.is_dir();
                if !dir && !ft.is_file() {
                    continue;
                }
                if dir && (exclude_names.contains(name.as_str()) || exclude_suffixes.iter().any(|s| name.ends_with(s))) {
                    continue;
                }
                let full = if abs.ends_with('/') { format!("{abs}{name}") } else { format!("{abs}/{name}") };
                if dir && !exclude_paths.is_empty() {
                    let base = &d.roots[root];
                    let rel = full.strip_prefix(base.as_str()).unwrap_or(&full).trim_start_matches('/');
                    if exclude_paths.iter().any(|x| rel == *x || rel.starts_with(&format!("{x}/"))) {
                        continue;
                    }
                }
                let idx = d.names.len() as i32;
                d.lower.push(name.to_lowercase().into_boxed_str());
                d.names.push(name.into_boxed_str());
                d.parent.push(id);
                d.depth.push(level + 1);
                d.is_dir.push(dir);
                if dir && level + 1 < MAX_DEPTH {
                    queue.push_back((full, idx, level + 1, root));
                }
            }
        }
        if self.generation.load(Ordering::SeqCst) != generation {
            return self.count();
        }
        d.recent = load_recent();
        let n = d.names.len();
        *self.data.write().unwrap() = d;
        self.count.store(n, Ordering::Relaxed);
        n
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<FileHit> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let d = self.data.read().unwrap();
        let started = Instant::now();
        let mut scored: Vec<(usize, f64, Vec<usize>)> = Vec::new();
        let consider = |i: usize, base: f64, indexes: Vec<usize>, out: &mut Vec<(usize, f64, Vec<usize>)>| {
            let mut score = base - d.depth[i] as f64 * 14.0;
            if d.is_dir[i] {
                score += 25.0;
            }
            if d.depth[i] == 1 && HOME_FOLDERS.contains(&&*d.names[i]) {
                score += 120.0;
            }
            if d.names[i].starts_with('.') {
                score -= 80.0;
            }
            out.push((i, score, indexes));
        };

        // Pass 1: substring hits (covers most queries).
        for i in 0..d.lower.len() {
            if d.lower[i].contains(q.as_str())
                && let Some(m) = fuzzy_match(&q, &d.names[i])
            {
                consider(i, m.score, m.indexes, &mut scored);
            }
        }
        // Pass 2: initials / subsequence, time-boxed so typing stays instant.
        if scored.len() < limit * 3 && q.chars().count() >= 3 && !q.contains(' ') {
            let first = q.chars().next().unwrap();
            let seen: HashSet<usize> = scored.iter().map(|s| s.0).collect();
            for i in 0..d.lower.len() {
                if i & 4095 == 0 && started.elapsed().as_millis() > 45 {
                    break;
                }
                if seen.contains(&i) || !d.lower[i].contains(first) {
                    continue;
                }
                if let Some(m) = fuzzy_match(&q, &d.names[i])
                    && m.score >= 300.0
                {
                    consider(i, m.score - 60.0, m.indexes, &mut scored);
                }
            }
        }

        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(limit * 4);
        let mut top: Vec<FileHit> = scored
            .into_iter()
            .map(|(i, score, indexes)| {
                let path = path_of(&d, i);
                let bonus = if d.recent.contains(&path) { 160.0 } else { 0.0 };
                FileHit { path, dir: d.is_dir[i], score: score + bonus, indexes }
            })
            .collect();
        top.sort_by(|a, b| b.score.total_cmp(&a.score));
        top.truncate(limit);
        top
    }
}

fn path_of(d: &IndexData, i: usize) -> String {
    let mut parts = Vec::new();
    let mut cur = i as i32;
    while cur >= 0 {
        parts.push(&*d.names[cur as usize]);
        cur = d.parent[cur as usize];
    }
    parts.reverse();
    let root = &d.roots[(-cur - 1) as usize];
    format!("{root}/{}", parts.join("/"))
}

// ── Results ────────────────────────────────────────────────────────────────

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"];
const TEXT_EXT: &[&str] = &[
    "txt",
    "md",
    "markdown",
    "json",
    "js",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "jsx",
    "py",
    "rs",
    "go",
    "c",
    "h",
    "cpp",
    "hpp",
    "java",
    "kt",
    "dart",
    "rb",
    "php",
    "sh",
    "bash",
    "zsh",
    "fish",
    "yml",
    "yaml",
    "toml",
    "ini",
    "conf",
    "cfg",
    "xml",
    "html",
    "css",
    "scss",
    "sql",
    "csv",
    "log",
    "env",
    "lua",
    "swift",
    "vue",
    "svelte",
    "gradle",
    "properties",
    "desktop",
    "service",
    "lock",
];

pub fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rfind('.') {
        Some(0) | None => String::new(),
        Some(i) => name[i + 1..].to_lowercase(),
    }
}

pub fn is_image(path: &str) -> bool {
    IMAGE_EXT.contains(&extension(path).as_str())
}

pub fn basename(path: &str) -> String {
    let p = path.trim_end_matches('/');
    match p.rsplit_once('/') {
        Some((_, name)) if !name.is_empty() => name.to_string(),
        _ => {
            if p.is_empty() {
                "/".into()
            } else {
                p.to_string()
            }
        }
    }
}

pub fn dirname(path: &str) -> String {
    let p = path.trim_end_matches('/');
    match p.rsplit_once('/') {
        Some(("", _)) => "/".into(),
        Some((dir, _)) => dir.to_string(),
        None => ".".into(),
    }
}

fn file_icon(path: &str, dir: bool) -> IconSpec {
    if !dir && is_image(path) && fs::metadata(path).is_ok_and(|m| m.len() < MAX_IMAGE_BYTES) {
        return IconSpec::File(path.to_string());
    }
    IconSpec::FileType { path: path.to_string(), dir }
}

/// A result for a file or folder. `frecency` is the usage bonus for its history key.
pub fn file_result(path: &str, dir: bool, score: f64, indexes: Vec<usize>, frecency: f64) -> ResultItem {
    let key = format!("file:{path}");
    let mut item =
        ResultItem::new(key.clone(), if dir { ResultKind::Folder } else { ResultKind::File }, basename(path), Action::OpenPath(path.into()));
    item.subtitle = Some(tilde(&dirname(path)));
    item.icon = Some(file_icon(path, dir));
    item.score = score + frecency;
    item.matches = indexes;
    item.history_key = Some(key);
    let here = if dir { path.to_string() } else { dirname(path) };
    item.actions = vec![
        SecondaryAction::new(if dir { "Open folder" } else { "Open" }, Action::OpenPath(path.into())).hint("Enter"),
        SecondaryAction::new("Show in Files", Action::RevealPath(path.into())).hint("Ctrl+Enter").glyph(Glyph::Folder),
        SecondaryAction::new("Copy path", Action::Copy(path.into())).hint("Ctrl+C").glyph(Glyph::Copy),
        SecondaryAction::new("Open terminal here", Action::OpenTerminalAt(here)).glyph(Glyph::Terminal),
    ];
    item
}

/// "~/Documents/rep" or "/etc/hos" → entries of that folder matching the last part.
pub fn browse(query: &str, frecency: &dyn Fn(&str) -> f64) -> Vec<ResultItem> {
    let expanded = expand_tilde(query.trim());
    let is_dir_query = expanded.ends_with('/');
    let (folder, part) = if is_dir_query {
        (expanded.clone(), String::new())
    } else {
        let folder = dirname(&expanded);
        let part = expanded.rsplit('/').next().unwrap_or("").to_string();
        (if folder.ends_with('/') { folder } else { format!("{folder}/") }, part)
    };
    let Ok(entries) = fs::read_dir(&folder) else { return Vec::new() };
    let mut hits: Vec<(String, bool, f64, Vec<usize>)> = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !part.starts_with('.') && name.starts_with('.') {
            continue;
        }
        let m = if part.is_empty() { Some(crate::fuzzy::Match { score: 500.0, indexes: vec![] }) } else { fuzzy_match(&part, &name) };
        let Some(m) = m.filter(|m| m.score >= 300.0) else { continue };
        let dir = e.file_type().is_ok_and(|t| t.is_dir()) || (e.file_type().is_ok_and(|t| t.is_symlink()) && e.path().is_dir());
        hits.push((format!("{folder}{name}"), dir, m.score + if dir { 20.0 } else { 0.0 }, m.indexes));
    }
    hits.sort_by(|a, b| b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mut results: Vec<ResultItem> = hits
        .into_iter()
        .take(12)
        .map(|(p, dir, score, idx)| {
            let f = frecency(&format!("file:{p}"));
            file_result(&p, dir, 1800.0 + score / 10.0, idx, f)
        })
        .collect();
    if part.is_empty() && Path::new(&folder).exists() {
        let path = if folder.len() > 1 { folder.trim_end_matches('/').to_string() } else { folder.clone() };
        let mut item = file_result(&path, true, 2400.0, Vec::new(), 0.0);
        item.subtitle = Some("Open this folder".into());
        results.insert(0, item);
    }
    results
}

/// Files from the desktop's recently-used list, newest first.
pub fn recent_files(limit: usize) -> Vec<ResultItem> {
    let Some(x) = xbel() else { return Vec::new() };
    let mut entries: Vec<(String, chrono::DateTime<chrono::FixedOffset>)> =
        XBEL_ENTRY.captures_iter(&x).filter_map(|c| Some((percent_decode(&c[1]), chrono::DateTime::parse_from_rfc3339(&c[2]).ok()?))).collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.1));
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for (path, _) in entries {
        if !seen.insert(path.clone()) {
            continue;
        }
        let Ok(meta) = fs::metadata(&path) else { continue };
        items.push(file_result(&path, meta.is_dir(), 0.0, Vec::new(), 0.0));
        if items.len() >= limit {
            break;
        }
    }
    items
}

fn mtime_ms(meta: &fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn file_preview(path: &str) -> Preview {
    let Ok(meta) = fs::metadata(path) else { return Preview::None };
    let name = basename(path);
    let modified = mtime_ms(&meta);
    if meta.is_dir() {
        let mut entries: Vec<FolderEntry> = fs::read_dir(path)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        (!name.starts_with('.')).then(|| FolderEntry { dir: e.path().is_dir(), name })
                    })
                    .collect()
            })
            .unwrap_or_default();
        entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        let total = entries.len();
        entries.truncate(14);
        return Preview::Folder { path: path.into(), name, modified, entries, total };
    }
    let ext = extension(path);
    if IMAGE_EXT.contains(&ext.as_str()) && meta.len() < MAX_IMAGE_BYTES {
        return Preview::Image { path: path.into(), name, size: meta.len(), modified };
    }
    if meta.len() < 4 * 1024 * 1024 && (TEXT_EXT.contains(&ext.as_str()) || ext.is_empty()) {
        let mut buf = vec![0u8; (meta.len() as usize).min(6000)];
        if let Ok(mut f) = fs::File::open(path) {
            let n = f.read(&mut buf).unwrap_or(0);
            buf.truncate(n);
            if !buf.iter().take(1024).any(|&b| b == 0) {
                // Cut at a character boundary.
                let content = match std::str::from_utf8(&buf) {
                    Ok(s) => s.to_string(),
                    Err(e) => String::from_utf8_lossy(&buf[..e.valid_up_to()]).into_owned(),
                };
                return Preview::Text { path: path.into(), name, size: meta.len(), modified, content };
            }
        }
    }
    Preview::File { path: path.into(), name, size: meta.len(), modified, icon: file_icon(path, false) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for p in ["Documents/Project Proposal.docx", "Documents/notes.md", "node_modules/pkg/index.js", "deep/a/b/c/target.txt", ".hidden/secret.txt"]
        {
            let full = dir.join(p);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, "hello").unwrap();
        }
        dir
    }

    #[test]
    fn indexes_and_finds_files() {
        let dir = tree("index");
        let idx = FileIndex::default();
        let n =
            idx.build(&IndexOptions { roots: vec![dir.to_string_lossy().into()], exclude_dirs: vec!["node_modules".into()], include_hidden: false });
        assert_eq!(n, 8); // Documents, deep, a, b, c and three files
        let hits = idx.search("projprop", 5);
        assert_eq!(basename(&hits[0].path), "Project Proposal.docx");
        assert!(idx.search("index", 5).is_empty());
        assert!(idx.search("secret", 5).is_empty());
        assert_eq!(basename(&idx.search("target", 5)[0].path), "target.txt");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn path_helpers() {
        assert_eq!(basename("/a/b.txt"), "b.txt");
        assert_eq!(dirname("/a/b.txt"), "/a");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(extension("/x/.bashrc"), "");
        assert_eq!(extension("/x/Photo.JPG"), "jpg");
        assert_eq!(percent_decode("/home/me/My%20File.txt"), "/home/me/My File.txt");
    }

    #[test]
    fn previews_text_and_folders() {
        let dir = tree("preview");
        let docs = dir.join("Documents");
        match file_preview(docs.to_str().unwrap()) {
            Preview::Folder { total, entries, .. } => {
                assert_eq!(total, 2);
                assert_eq!(entries[0].name, "notes.md");
            }
            other => panic!("{other:?}"),
        }
        match file_preview(docs.join("notes.md").to_str().unwrap()) {
            Preview::Text { content, .. } => assert_eq!(content, "hello"),
            other => panic!("{other:?}"),
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
