//! Installed applications. The UI layer reads them from the desktop (GIO) into plain
//! `AppEntry` values; ranking, results and previews live here.

use crate::fuzzy::match_fields;
use crate::store::History;
use crate::types::*;

#[derive(Clone, Debug, PartialEq)]
pub struct AppAction {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppEntry {
    /// Desktop file id, e.g. "org.gnome.Nautilus.desktop".
    pub id: String,
    /// Path of the .desktop file.
    pub path: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    /// The Exec line as written.
    pub exec: String,
    pub icon: Option<IconSpec>,
    pub terminal: bool,
    pub actions: Vec<AppAction>,
}

fn to_result(app: &AppEntry, score: f64, matches: Vec<usize>) -> ResultItem {
    let mut item = ResultItem::new(
        format!("app:{}", app.id),
        ResultKind::App,
        app.name.clone(),
        Action::LaunchApp { desktop_id: app.id.clone(), action_id: None },
    );
    item.subtitle = app.comment.clone().or_else(|| app.generic_name.clone());
    item.icon = Some(app.icon.clone().unwrap_or(IconSpec::Glyph(Glyph::Sparkles)));
    item.score = score;
    item.matches = matches;
    item.history_key = Some(format!("app:{}", app.id));
    item.actions = app
        .actions
        .iter()
        .map(|a| SecondaryAction::new(a.name.clone(), Action::LaunchApp { desktop_id: app.id.clone(), action_id: Some(a.id.clone()) }))
        .collect();
    item
}

pub fn search_apps(apps: &[AppEntry], query: &str, limit: usize, history: &History, now: i64) -> Vec<ResultItem> {
    let mut out: Vec<ResultItem> = apps
        .iter()
        .filter_map(|app| {
            let keywords = app.keywords.join(" ");
            let id = app.id.trim_end_matches(".desktop");
            let generic = app.generic_name.as_deref().unwrap_or("");
            let m = match_fields(query, &app.name, &[generic, &keywords, id], 0.72)?;
            (m.score >= 200.0).then(|| to_result(app, m.score + 150.0 + history.frecency(&format!("app:{}", app.id), now), m.indexes))
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out.truncate(limit);
    out
}

pub fn frequent_apps(apps: &[AppEntry], limit: usize, history: &History, now: i64) -> Vec<ResultItem> {
    let mut scored: Vec<(&AppEntry, f64)> =
        apps.iter().map(|a| (a, history.frecency(&format!("app:{}", a.id), now))).filter(|(_, f)| *f > 0.0).collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.into_iter().take(limit).map(|(a, f)| to_result(a, f, Vec::new())).collect()
}

/// A result for one app, without a query (for the start screen).
pub fn app_result(app: &AppEntry) -> ResultItem {
    to_result(app, 0.0, Vec::new())
}

fn strip_field_codes(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' && chars.peek().is_some_and(|n| n.is_ascii_alphabetic()) {
            chars.next();
            while out.ends_with(' ') {
                out.pop();
            }
            continue;
        }
        out.push(c);
    }
    out.trim().to_string()
}

pub fn app_preview(app: &AppEntry) -> Preview {
    let skip = ["GTK", "GNOME", "Qt", "KDE", "X-"];
    Preview::App {
        name: app.name.clone(),
        comment: app.comment.clone().or_else(|| app.generic_name.clone()),
        icon: app.icon.clone(),
        exec: strip_field_codes(&app.exec),
        categories: app.categories.iter().filter(|c| !skip.iter().any(|s| c.starts_with(s))).take(4).cloned().collect(),
        path: app.path.clone(),
    }
}

/// Split an Exec line into argv per the Desktop Entry spec, expanding the field codes we support.
pub fn parse_exec(exec: &str, app: &AppEntry, icon_name: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let (mut in_quote, mut has) = (false, false);
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if in_quote {
            if c == '\\' {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            } else if c == '"' {
                in_quote = false;
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_quote = true;
            has = true;
        } else if c == ' ' || c == '\t' {
            if has || !cur.is_empty() {
                args.push(std::mem::take(&mut cur));
            }
            has = false;
        } else {
            cur.push(c);
        }
    }
    if has || !cur.is_empty() {
        args.push(cur);
    }

    let mut out = Vec::new();
    for arg in args {
        if arg == "%i" {
            if let Some(icon) = icon_name {
                out.push("--icon".into());
                out.push(icon.into());
            }
            continue;
        }
        if arg.len() == 2 && arg.starts_with('%') && "fFuUdDnNvm".contains(&arg[1..]) {
            continue;
        }
        let mut expanded = String::new();
        let mut it = arg.chars().peekable();
        while let Some(c) = it.next() {
            if c != '%' {
                expanded.push(c);
                continue;
            }
            match it.next() {
                Some('%') => expanded.push('%'),
                Some('c') => expanded.push_str(&app.name),
                Some('k') => expanded.push_str(&app.path),
                Some(x) if "fFuUdDnNvm".contains(x) => {}
                Some(x) => {
                    expanded.push('%');
                    expanded.push(x);
                }
                None => expanded.push('%'),
            }
        }
        out.push(expanded);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, id: &str) -> AppEntry {
        AppEntry {
            id: id.into(),
            path: format!("/usr/share/applications/{id}"),
            name: name.into(),
            generic_name: Some("Web Browser".into()),
            comment: None,
            keywords: vec!["internet".into()],
            categories: vec!["Network".into(), "GTK".into()],
            exec: "firefox %u".into(),
            icon: None,
            terminal: false,
            actions: vec![AppAction { id: "new-window".into(), name: "New Window".into() }],
        }
    }

    #[test]
    fn searches_names_and_keywords() {
        let apps = vec![app("Firefox", "firefox.desktop"), app("Files", "org.gnome.Nautilus.desktop")];
        let h = History::default();
        let r = search_apps(&apps, "fire", 5, &h, 0);
        assert_eq!(r[0].title, "Firefox");
        assert_eq!(r[0].actions.len(), 1);
        assert!(!search_apps(&apps, "nautilus", 5, &h, 0).is_empty());
    }

    #[test]
    fn exec_lines_split_like_the_spec() {
        let a = app("My App", "my.desktop");
        assert_eq!(parse_exec(r#"env "A B=1" my-app --name=%c %U"#, &a, None), ["env", "A B=1", "my-app", "--name=My App"]);
        assert_eq!(parse_exec("app %i --x 100%%", &a, Some("ico")), ["app", "--icon", "ico", "--x", "100%"]);
        assert_eq!(parse_exec(r#"sh -c "echo \"hi\"""#, &a, None), ["sh", "-c", "echo \"hi\""]);
    }

    #[test]
    fn preview_hides_toolkit_categories_and_field_codes() {
        let Preview::App { exec, categories, .. } = app_preview(&app("Firefox", "firefox.desktop")) else { panic!() };
        assert_eq!(exec, "firefox");
        assert_eq!(categories, ["Network"]);
    }
}
