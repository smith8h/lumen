//! Clipboard history results for the ";" mode, and clip previews.

use crate::cliphistory::{ClipEntry, ClipHistory, clip_title};
use crate::format::{ago, plural};
use crate::store::Settings;
use crate::types::*;

const RESULT_LIMIT: usize = 30;
const PREVIEW_CHARS: usize = 4000;

fn clip_item(e: &ClipEntry, score: f64, matches: Vec<usize>, now: i64) -> ResultItem {
    let mut item =
        ResultItem::new(format!("clip:{}", e.id), ResultKind::Clip, clip_title(&e.text), Action::ClipPaste { id: e.id.clone(), terminal: None });
    item.subtitle =
        Some(format!("{}{} · {}", if e.pinned { "Pinned · " } else { "" }, ago(e.last_at, now), plural(e.text.chars().count(), "char", "chars")));
    item.icon = Some(IconSpec::Glyph(if e.pinned { Glyph::Pin } else { Glyph::Clipboard }));
    item.score = score;
    item.matches = matches;
    item.actions = vec![
        SecondaryAction::new("Copy to clipboard", Action::ClipCopy(e.id.clone())).hint("Ctrl+Enter").glyph(Glyph::Copy),
        SecondaryAction::new("Paste into terminal (Ctrl+Shift+V)", Action::ClipPaste { id: e.id.clone(), terminal: Some(true) })
            .glyph(Glyph::Terminal),
        SecondaryAction::new(if e.pinned { "Unpin" } else { "Pin" }, Action::ClipPin { id: e.id.clone(), pinned: !e.pinned })
            .hint("Ctrl+Shift+P")
            .glyph(Glyph::Pin),
        SecondaryAction::new("Delete", Action::ClipDelete(e.id.clone())).hint("Ctrl+D").glyph(Glyph::Trash),
    ];
    item
}

fn notice(id: &str, title: &str, subtitle: &str) -> ResultItem {
    let mut item = ResultItem::new(id, ResultKind::Lumen, title, Action::Lumen(LumenOp::Settings));
    item.subtitle = Some(subtitle.into());
    item.icon = Some(IconSpec::Glyph(Glyph::Clipboard));
    item.score = 1000.0;
    item
}

/// Results for ";query". Never part of normal searches.
pub fn clip_results(history: &ClipHistory, query: &str, settings: &Settings, now: i64) -> Vec<ResultItem> {
    if !settings.clipboard_enabled {
        return vec![notice("clip:off", "Clipboard history is off", "Turn it on in Settings → Clipboard")];
    }
    let q = query.trim();
    let ranked = history.search(q, RESULT_LIMIT, now);
    if ranked.is_empty() && q.is_empty() {
        let sub = if settings.clipboard_paused { "Recording is paused in Settings → Clipboard" } else { "Text you copy in any app shows up here" };
        return vec![notice("clip:empty", "Nothing copied yet", sub)];
    }
    // Keep the list order (pinned, then newest) when there's no query.
    ranked
        .into_iter()
        .enumerate()
        .map(|(i, r)| clip_item(&r.entry, if q.is_empty() { 2000.0 - i as f64 } else { 1000.0 + r.score }, r.matches, now))
        .collect()
}

pub fn clip_preview(history: &ClipHistory, id: &str) -> Preview {
    let Some(e) = history.get(id) else { return Preview::None };
    let length = e.text.chars().count();
    let truncated = length > PREVIEW_CHARS;
    Preview::Clip {
        text: if truncated { e.text.chars().take(PREVIEW_CHARS).collect() } else { e.text.clone() },
        truncated,
        length,
        lines: e.text.split('\n').count(),
        first_at: e.first_at,
        last_at: e.last_at,
        pinned: e.pinned,
    }
}
