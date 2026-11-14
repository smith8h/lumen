//! Runs every provider for a query, ranks the results and groups them into sections.

use std::collections::HashMap;

use crate::types::*;

fn title(id: SectionId) -> &'static str {
    match id {
        SectionId::Top => "Top Hit",
        SectionId::Calc => "Calculator",
        SectionId::Clipboard => "Clipboard",
        SectionId::Apps => "Applications",
        SectionId::Commands => "Commands",
        SectionId::Files => "Files",
        SectionId::Folders => "Folders",
        SectionId::Web => "Web",
        SectionId::System => "System",
    }
}

const ORDER: [SectionId; 9] = [
    SectionId::Clipboard,
    SectionId::Calc,
    SectionId::Top,
    SectionId::Apps,
    SectionId::System,
    SectionId::Folders,
    SectionId::Files,
    SectionId::Commands,
    SectionId::Web,
];

fn section_of(item: &ResultItem) -> SectionId {
    match item.kind {
        ResultKind::App => SectionId::Apps,
        ResultKind::File => SectionId::Files,
        ResultKind::Folder => SectionId::Folders,
        ResultKind::Calc => SectionId::Calc,
        ResultKind::Clip => SectionId::Clipboard,
        ResultKind::Url | ResultKind::Web => SectionId::Web,
        ResultKind::Command => SectionId::Commands,
        ResultKind::System | ResultKind::Lumen => SectionId::System,
    }
}

pub fn sort_by_score(items: &mut [ResultItem]) {
    items.sort_by(|a, b| b.score.total_cmp(&a.score));
}

pub fn group(mut items: Vec<ResultItem>, with_top_hit: bool) -> Vec<ResultSection> {
    sort_by_score(&mut items);
    let top =
        if with_top_hit { items.iter().position(|i| i.kind != ResultKind::Calc && i.kind != ResultKind::Web && i.score >= 640.0) } else { None };
    let mut buckets: HashMap<SectionId, Vec<ResultItem>> = HashMap::new();
    let mut top_item = None;
    for (i, item) in items.into_iter().enumerate() {
        if Some(i) == top {
            top_item = Some(item);
            continue;
        }
        buckets.entry(section_of(&item)).or_default().push(item);
    }
    if let Some(t) = top_item {
        buckets.insert(SectionId::Top, vec![t]);
    }
    ORDER
        .iter()
        .filter_map(|id| buckets.remove(id).filter(|v| !v.is_empty()).map(|items| ResultSection { id: *id, title: title(*id).into(), items }))
        .collect()
}

/// Keep sections tidy: cap each kind (apps and files get one extra slot when they win).
pub fn cap_kinds(mut items: Vec<ResultItem>, caps: &[(ResultKind, usize)]) -> Vec<ResultItem> {
    sort_by_score(&mut items);
    let mut counts: HashMap<ResultKind, usize> = HashMap::new();
    items
        .into_iter()
        .filter(|i| {
            let n = counts.entry(i.kind).or_default();
            *n += 1;
            let cap = caps.iter().find(|(k, _)| *k == i.kind).map_or(99, |(_, c)| *c);
            *n <= cap + usize::from(matches!(i.kind, ResultKind::App | ResultKind::File))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: ResultKind, score: f64) -> ResultItem {
        let mut i = ResultItem::new(id, kind, id, Action::Copy(String::new()));
        i.score = score;
        i
    }

    #[test]
    fn picks_a_top_hit_and_orders_sections() {
        let s = group(vec![item("web", ResultKind::Web, 120.0), item("app", ResultKind::App, 900.0), item("file", ResultKind::File, 500.0)], true);
        let ids: Vec<SectionId> = s.iter().map(|s| s.id).collect();
        assert_eq!(ids, [SectionId::Top, SectionId::Files, SectionId::Web]);
        assert_eq!(s[0].items[0].id, "app");
    }

    #[test]
    fn caps_kinds() {
        let items = (0..10).map(|i| item(&format!("f{i}"), ResultKind::Folder, i as f64)).collect();
        assert_eq!(cap_kinds(items, &[(ResultKind::Folder, 4)]).len(), 4);
    }
}
