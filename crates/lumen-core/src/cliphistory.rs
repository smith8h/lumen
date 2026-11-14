//! Clipboard history as plain data: dedupe, size cap, pinning and ranking.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

use crate::fuzzy::match_fields;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClipEntry {
    pub id: String,
    pub text: String,
    /// When this text was first copied (ms since the epoch).
    pub first_at: i64,
    /// When it was last copied or used.
    pub last_at: i64,
    pub pinned: bool,
}

/// Bigger copies (log dumps, whole files) are skipped.
pub const MAX_CLIP_BYTES: usize = 100 * 1024;
pub const TITLE_CHARS: usize = 80;
/// How much of a clip the fuzzy matcher looks at beyond its first line.
const SEARCH_CHARS: usize = 2000;

pub fn acceptable(text: &str) -> bool {
    text.chars().any(|c| !c.is_whitespace()) && text.len() <= MAX_CLIP_BYTES
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// First non-blank line, whitespace collapsed, about 80 characters.
pub fn clip_title(text: &str) -> String {
    let line = text.split('\n').find(|l| l.chars().any(|c| !c.is_whitespace())).unwrap_or("");
    let flat = collapse_ws(line);
    if flat.chars().count() > TITLE_CHARS {
        let cut: String = flat.chars().take(TITLE_CHARS - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        flat
    }
}

/// Small bonus so recent clips win ties: 0..60.
pub fn recency_boost(last_at: i64, now: i64) -> f64 {
    let hours = (now - last_at) as f64 / 3_600_000.0;
    if hours < 1.0 {
        60.0
    } else if hours < 24.0 {
        40.0
    } else if hours < 24.0 * 7.0 {
        20.0
    } else {
        0.0
    }
}

#[derive(Clone, Debug)]
pub struct RankedClip {
    pub entry: ClipEntry,
    pub score: f64,
    /// Matched character indexes in clip_title(entry.text).
    pub matches: Vec<usize>,
}

pub fn random_id() -> String {
    let mut b = [0u8; 6];
    if getrandom::fill(&mut b).is_err() {
        let n = crate::store::now_ms() as u64 ^ (std::process::id() as u64) << 20;
        b.copy_from_slice(&n.to_le_bytes()[..6]);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub struct ClipHistory {
    /// Newest first.
    entries: Vec<ClipEntry>,
    max: usize,
    make_id: Box<dyn FnMut() -> String + Send>,
}

impl Default for ClipHistory {
    fn default() -> Self {
        Self::new(200)
    }
}

impl ClipHistory {
    pub fn new(max: usize) -> Self {
        Self::with_ids(max, Box::new(random_id))
    }

    pub fn with_ids(max: usize, make_id: Box<dyn FnMut() -> String + Send>) -> Self {
        Self { entries: Vec::new(), max: max.max(1), make_id }
    }

    /// Replace everything, e.g. from disk. Bad or oversized entries are dropped.
    pub fn load(&mut self, raw: &Value) {
        let mut seen = HashSet::new();
        let mut ids = HashSet::new();
        self.entries.clear();
        for e in raw.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let Some(text) = e.get("text").and_then(Value::as_str) else { continue };
            if !acceptable(text) || seen.contains(text) {
                continue;
            }
            let id = match e.get("id").and_then(Value::as_str) {
                Some(id) if valid_id(id) && !ids.contains(id) => id.to_string(),
                _ => (self.make_id)(),
            };
            let num = |k: &str| e.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).map(|v| v as i64);
            let last_at = num("lastAt").unwrap_or(0);
            let first_at = num("firstAt").unwrap_or(last_at);
            seen.insert(text.to_string());
            ids.insert(id.clone());
            self.entries.push(ClipEntry { id, text: text.to_string(), first_at, last_at, pinned: e.get("pinned") == Some(&Value::Bool(true)) });
        }
        self.entries.sort_by_key(|e| std::cmp::Reverse(e.last_at));
        self.evict();
    }

    /// Record a copy. The same text moves to the top. Returns None when the text is skipped.
    pub fn add(&mut self, text: &str, now: i64) -> Option<ClipEntry> {
        if !acceptable(text) {
            return None;
        }
        let entry = match self.entries.iter().position(|e| e.text == text) {
            Some(i) => {
                let mut e = self.entries.remove(i);
                e.last_at = now;
                e
            }
            None => ClipEntry { id: (self.make_id)(), text: text.to_string(), first_at: now, last_at: now, pinned: false },
        };
        self.entries.insert(0, entry.clone());
        self.evict();
        Some(entry)
    }

    pub fn set_max(&mut self, max: usize) {
        self.max = max.max(1);
        self.evict();
    }

    /// Drop the oldest unpinned clips beyond the cap. Pinned clips are never dropped and don't use up the cap.
    fn evict(&mut self) {
        let mut over = self.entries.iter().filter(|e| !e.pinned).count() as isize - self.max as isize;
        let mut i = self.entries.len();
        while i > 0 && over > 0 {
            i -= 1;
            if self.entries[i].pinned {
                continue;
            }
            self.entries.remove(i);
            over -= 1;
        }
    }

    pub fn get(&self, id: &str) -> Option<&ClipEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Mark a clip as just used, so it moves to the top.
    pub fn touch(&mut self, id: &str, now: i64) -> bool {
        let Some(i) = self.entries.iter().position(|e| e.id == id) else { return false };
        let mut e = self.entries.remove(i);
        e.last_at = now;
        self.entries.insert(0, e);
        true
    }

    pub fn pin(&mut self, id: &str, pinned: bool) -> bool {
        let Some(e) = self.entries.iter_mut().find(|e| e.id == id) else { return false };
        e.pinned = pinned;
        self.evict();
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    /// Forget everything except pinned clips. Returns how many were removed.
    pub fn clear(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| e.pinned);
        before - self.entries.len()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn all(&self) -> &[ClipEntry] {
        &self.entries
    }

    /// What `;` shows before typing: pinned first, then newest.
    pub fn recent(&self, limit: usize) -> Vec<ClipEntry> {
        self.entries.iter().filter(|e| e.pinned).chain(self.entries.iter().filter(|e| !e.pinned)).take(limit).cloned().collect()
    }

    /// Fuzzy search over the first line (highlighted) and the start of the text, with a small recency boost.
    pub fn search(&self, query: &str, limit: usize, now: i64) -> Vec<RankedClip> {
        let q = query.trim();
        if q.is_empty() {
            return self.recent(limit).into_iter().map(|entry| RankedClip { entry, score: 0.0, matches: Vec::new() }).collect();
        }
        let mut out = Vec::new();
        for entry in &self.entries {
            let title = clip_title(&entry.text);
            let body: String =
                if entry.text.chars().count() > SEARCH_CHARS { entry.text.chars().take(SEARCH_CHARS).collect() } else { entry.text.clone() };
            let flat = collapse_ws(&body);
            let Some(m) = match_fields(q, &title, &[&flat], 0.85) else { continue };
            if m.score < 300.0 {
                continue;
            }
            let score = m.score + recency_boost(entry.last_at, now) + if entry.pinned { 15.0 } else { 0.0 };
            out.push(RankedClip { entry: entry.clone(), score, matches: m.indexes });
        }
        out.sort_by(|a, b| b.score.total_cmp(&a.score).then(b.entry.last_at.cmp(&a.entry.last_at)));
        out.truncate(limit);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ids() -> Box<dyn FnMut() -> String + Send> {
        let mut n = 0;
        Box::new(move || {
            n += 1;
            format!("id{n}")
        })
    }
    fn texts(h: &ClipHistory) -> Vec<&str> {
        h.all().iter().map(|e| e.text.as_str()).collect()
    }

    #[test]
    fn skips_empty_whitespace_and_oversized_text() {
        let mut h = ClipHistory::with_ids(10, ids());
        assert!(h.add("", 0).is_none());
        assert!(h.add("  \n\t ", 0).is_none());
        assert!(h.add(&"x".repeat(MAX_CLIP_BYTES + 1), 0).is_none());
        assert!(h.add(&"x".repeat(MAX_CLIP_BYTES), 0).is_some());
        assert_eq!(h.len(), 1);
        // The limit is in bytes, not characters.
        assert!(!acceptable(&"é".repeat(MAX_CLIP_BYTES / 2 + 1)));
    }

    #[test]
    fn same_text_moves_to_top() {
        let mut h = ClipHistory::with_ids(10, ids());
        let a = h.add("alpha", 1).unwrap();
        h.add("beta", 2);
        let again = h.add("alpha", 3).unwrap();
        assert_eq!(again.id, a.id);
        assert_eq!(again.first_at, 1);
        assert_eq!(again.last_at, 3);
        assert_eq!(texts(&h), ["alpha", "beta"]);
    }

    #[test]
    fn caps_history_dropping_oldest() {
        let mut h = ClipHistory::with_ids(3, ids());
        for (i, t) in ["a", "b", "c", "d", "e"].iter().enumerate() {
            h.add(t, i as i64);
        }
        assert_eq!(texts(&h), ["e", "d", "c"]);
        h.set_max(2);
        assert_eq!(texts(&h), ["e", "d"]);
    }

    #[test]
    fn pinned_clips_are_kept_and_do_not_use_the_cap() {
        let mut h = ClipHistory::with_ids(2, ids());
        let a = h.add("a", 1).unwrap();
        h.pin(&a.id, true);
        h.add("b", 2);
        h.add("c", 3);
        h.add("d", 4);
        assert_eq!(texts(&h), ["d", "c", "a"]);
        assert_eq!(h.clear(), 2);
        assert_eq!(texts(&h), ["a"]);
    }

    #[test]
    fn recent_lists_pinned_first() {
        let mut h = ClipHistory::with_ids(10, ids());
        let a = h.add("a", 1).unwrap();
        h.add("b", 2);
        h.pin(&a.id, true);
        let r: Vec<String> = h.recent(10).into_iter().map(|e| e.text).collect();
        assert_eq!(r, ["a", "b"]);
    }

    #[test]
    fn touch_moves_to_top() {
        let mut h = ClipHistory::with_ids(10, ids());
        let a = h.add("a", 1).unwrap();
        h.add("b", 2);
        assert!(h.touch(&a.id, 3));
        assert_eq!(texts(&h), ["a", "b"]);
        assert!(!h.touch("nope", 4));
    }

    #[test]
    fn load_drops_bad_entries_and_duplicates() {
        let mut h = ClipHistory::with_ids(10, ids());
        h.load(&json!([
            {"id": "x1", "text": "one", "firstAt": 1, "lastAt": 5, "pinned": true},
            {"id": "x1", "text": "two", "lastAt": 9},
            {"id": "bad id!", "text": "three", "lastAt": 2},
            {"text": "one", "lastAt": 7},
            {"text": "   "},
            {"id": 5},
            "nonsense"
        ]));
        assert_eq!(texts(&h), ["two", "one", "three"]);
        assert_eq!(h.get("x1").unwrap().text, "one");
        assert!(h.get("x1").unwrap().pinned);
        let two = h.all().iter().find(|e| e.text == "two").unwrap();
        assert_eq!(two.first_at, 9);
        assert_ne!(two.id, "x1");
    }

    #[test]
    fn titles_use_first_non_blank_line() {
        assert_eq!(clip_title("\n\n   hello   world  \nsecond"), "hello world");
        let long = "a".repeat(200);
        let t = clip_title(&long);
        assert_eq!(t.chars().count(), TITLE_CHARS);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn search_ranks_matches_and_recency() {
        let mut h = ClipHistory::with_ids(10, ids());
        let now = 100 * 3_600_000;
        h.add("git push origin main", now - 48 * 3_600_000);
        h.add("cargo build --release", now - 1000);
        h.add("git status", now - 10);
        let r = h.search("git", 10, now);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].entry.text, "git status");
        assert_eq!(r[0].matches, vec![0, 1, 2]);
        assert!(h.search("zzzz", 10, now).is_empty());
        assert_eq!(h.search("", 10, now).len(), 3);
    }

    #[test]
    fn recency_boost_steps_down() {
        let h = 3_600_000;
        assert_eq!(recency_boost(0, h / 2), 60.0);
        assert_eq!(recency_boost(0, 2 * h), 40.0);
        assert_eq!(recency_boost(0, 48 * h), 20.0);
        assert_eq!(recency_boost(0, 24 * 8 * h), 0.0);
    }
}
