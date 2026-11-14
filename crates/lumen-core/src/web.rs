//! URLs open directly in the default browser; anything else can be searched on the web.

use regex::Regex;
use std::sync::LazyLock;

use crate::constants::{SEARCH_ENGINES, WEB_KEYWORDS};
use crate::store::{SearchEngine, Settings};
use crate::types::*;

static TLD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\.(com|org|net|io|dev|app|ai|co|me|edu|gov|info|xyz|tech|sh|gg|tv|so|ly|to|fm|uk|de|fr|iq|sa|ae|eg|tr|ru|in|jp|cn|br|ca|au|nl|it|es|ch|se|no|pl|us|eu|page|site|online|store|blog|cloud|design|news|wiki|[a-z]{2})(:\d+)?(/|$|\?|#)").unwrap()
});
static SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(https?|ftp)://\S+$").unwrap());
static LOCAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\])(:\d+)?(/\S*)?$").unwrap());
static IPV4: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{1,3}(\.\d{1,3}){3}(:\d+)?(/\S*)?$").unwrap());
static DOMAIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[\w-]+(\.[\w-]+)+(:\d+)?(/\S*)?$").unwrap());
static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+(\.\d+)+$").unwrap());

/// Returns a normalized URL if the query is clearly a web address.
pub fn as_url(query: &str) -> Option<String> {
    let q = query.trim();
    if q.is_empty() || q.chars().any(char::is_whitespace) {
        return None;
    }
    if SCHEME.is_match(q) {
        return Some(q.to_string());
    }
    if LOCAL.is_match(q) || IPV4.is_match(q) {
        return Some(format!("http://{q}"));
    }
    if DOMAIN.is_match(q) && TLD.is_match(&format!("{q}/")) && !VERSION.is_match(q) {
        return Some(format!("https://{q}"));
    }
    None
}

/// encodeURIComponent.
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn fill(template: &str, q: &str) -> String {
    template.replacen("%s", &encode_component(q), 1)
}

/// "https://www.youtube.com/results?…" → "youtube.com"
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].rsplit_once('@').map_or(&rest[..end], |(_, h)| h);
    let host = if host.starts_with('[') { host } else { host.split(':').next().unwrap_or(host) };
    (!host.is_empty()).then(|| host.trim_start_matches("www.").to_lowercase())
}

pub fn engine(settings: &Settings) -> (String, String) {
    if settings.search_engine == SearchEngine::Custom && settings.custom_search_url.contains("%s") {
        let name = host_of(&settings.custom_search_url.replace("%s", "x")).unwrap_or_else(|| "the web".into());
        return (name, settings.custom_search_url.clone());
    }
    let id = if settings.search_engine == SearchEngine::Custom { "google" } else { settings.search_engine.id() };
    let e = SEARCH_ENGINES.iter().find(|e| e.id == id).unwrap_or(&SEARCH_ENGINES[0]);
    (e.name.to_string(), e.url.to_string())
}

/// The engine's name for the "?" mode chip.
pub fn engine_label(settings: &Settings) -> String {
    if settings.search_engine == SearchEngine::Custom { "Web".into() } else { engine(settings).0 }
}

pub fn web_results(query: &str, settings: &Settings) -> Vec<ResultItem> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();

    let url = as_url(q);
    if let Some(url) = &url {
        let title = q.trim_start_matches("https://").trim_start_matches("http://");
        let mut item = ResultItem::new(format!("url:{url}"), ResultKind::Url, title, Action::OpenUrl(url.clone()));
        item.subtitle = Some("Open in your browser".into());
        item.icon = Some(IconSpec::Glyph(Glyph::Globe));
        item.score = 2600.0;
        item.history_key = Some(format!("url:{url}"));
        item.actions = vec![SecondaryAction::new("Copy link", Action::Copy(url.clone())).glyph(Glyph::Copy)];
        out.push(item);
    }

    // Keyword searches: "yt lofi", "gh electron", "wiki baghdad".
    if let Some((kw, rest)) = q.split_once(' ') {
        let rest = rest.trim();
        let kw = kw.to_lowercase();
        if let Some((_, name, template)) = WEB_KEYWORDS.iter().find(|(k, _, _)| *k == kw)
            && !rest.is_empty()
        {
            let mut item = ResultItem::new(
                format!("kw:{name}:{rest}"),
                ResultKind::Web,
                format!("Search {name} for “{rest}”"),
                Action::OpenUrl(fill(template, rest)),
            );
            item.subtitle = host_of(template);
            item.icon = Some(IconSpec::Glyph(Glyph::Search));
            item.score = 2500.0;
            out.push(item);
        }
    }

    let (name, template) = engine(settings);
    let mut item = ResultItem::new(format!("web:{q}"), ResultKind::Web, format!("Search {name} for “{q}”"), Action::OpenUrl(fill(&template, q)));
    item.subtitle = Some("Opens in your default browser".into());
    item.icon = Some(IconSpec::Glyph(Glyph::Search));
    item.score = if url.is_some() { 400.0 } else { 120.0 };
    item.actions = WEB_KEYWORDS
        .iter()
        .filter(|(k, _, _)| ["yt", "gh", "wiki", "maps"].contains(k))
        .map(|(_, name, t)| SecondaryAction::new(format!("Search {name}"), Action::OpenUrl(fill(t, q))).glyph(Glyph::Globe))
        .collect();
    out.push(item);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_urls() {
        assert_eq!(as_url("github.com").as_deref(), Some("https://github.com"));
        assert_eq!(as_url("localhost:3000/x").as_deref(), Some("http://localhost:3000/x"));
        assert_eq!(as_url("192.168.1.1").as_deref(), Some("http://192.168.1.1"));
        assert_eq!(as_url("https://a.b/c?d").as_deref(), Some("https://a.b/c?d"));
        assert_eq!(as_url("1.2.3"), None);
        assert_eq!(as_url("hello world"), None);
        assert_eq!(as_url("readme.md"), Some("https://readme.md".into()));
        assert_eq!(as_url("notes.txt"), None);
    }

    #[test]
    fn keyword_and_engine_results() {
        let s = Settings::default();
        let r = web_results("yt lofi beats", &s);
        assert_eq!(r[0].title, "Search YouTube for “lofi beats”");
        assert_eq!(r[0].action, Action::OpenUrl("https://www.youtube.com/results?search_query=lofi%20beats".into()));
        assert_eq!(r[0].subtitle.as_deref(), Some("youtube.com"));
        assert_eq!(r.last().unwrap().title, "Search Google for “yt lofi beats”");
    }

    #[test]
    fn custom_engine_uses_its_host() {
        let s = Settings { search_engine: SearchEngine::Custom, custom_search_url: "https://www.kagi.com/search?q=%s".into(), ..Default::default() };
        assert_eq!(engine(&s).0, "kagi.com");
        assert_eq!(engine_label(&s), "Web");
    }
}
