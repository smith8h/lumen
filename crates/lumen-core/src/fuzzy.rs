//! Small, fast fuzzy scorer tuned for launcher-style matching.
//! Prefers, in order: exact > prefix > word prefix > word initials ("vsc" → "Visual Studio Code")
//! > substring > subsequence. Indexes are character (not byte) positions.

#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// 0..1000
    pub score: f64,
    pub indexes: Vec<usize>,
}

fn is_break(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '_' | '.' | '(' | ')' | '/' | '[' | ']' | ':')
}

#[inline]
pub(crate) fn lower(c: char) -> char {
    if c.is_ascii() { c.to_ascii_lowercase() } else { c.to_lowercase().next().unwrap_or(c) }
}

fn is_word_start(text: &[char], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let (prev, cur) = (text[i - 1], text[i]);
    if is_break(prev) {
        return true;
    }
    // camelCase boundary: "gnomeTerminal" → T
    !prev.is_uppercase() && cur.is_uppercase()
}

fn range(start: usize, len: usize) -> Vec<usize> {
    (start..start + len).collect()
}

fn starts_with_at(t: &[char], q: &[char], i: usize) -> bool {
    i + q.len() <= t.len() && t[i..i + q.len()] == *q
}

fn find(t: &[char], q: &[char]) -> Option<usize> {
    if q.is_empty() {
        return Some(0);
    }
    (0..=t.len().saturating_sub(q.len())).find(|&i| starts_with_at(t, q, i))
}

/// Score `query` against `text`. Returns None when there is no match.
pub fn fuzzy_match(query: &str, text: &str) -> Option<Match> {
    if query.is_empty() {
        return Some(Match { score: 0.0, indexes: Vec::new() });
    }
    let q: Vec<char> = query.chars().map(lower).collect();
    let orig: Vec<char> = text.chars().collect();
    let t: Vec<char> = orig.iter().map(|&c| lower(c)).collect();
    if q.len() > t.len() {
        return None;
    }
    let (ql, tl) = (q.len() as f64, t.len() as f64);
    if t == q {
        return Some(Match { score: 1000.0, indexes: range(0, q.len()) });
    }
    if starts_with_at(&t, &q, 0) {
        return Some(Match { score: 900.0 - (tl - ql).min(60.0), indexes: range(0, q.len()) });
    }

    // Word prefix: "code" in "Visual Studio Code".
    for i in 1..t.len() {
        if is_word_start(&orig, i) && starts_with_at(&t, &q, i) {
            let score = 820.0 - (i as f64).min(40.0) - (tl - ql).min(40.0) / 2.0;
            return Some(Match { score, indexes: range(i, q.len()) });
        }
    }

    let q_no_space: Vec<char> = q.iter().copied().filter(|c| !c.is_whitespace()).collect();

    // Word initials: "vsc", "lo w" → LibreOffice Writer.
    if let Some(idx) = initials(&q_no_space, &orig, &t) {
        return Some(Match { score: 780.0 - tl.min(60.0) / 3.0, indexes: idx });
    }

    if let Some(sub) = find(&t, &q) {
        let score = 650.0 - (sub as f64).min(50.0) - (tl - ql).min(50.0) / 2.0;
        return Some(Match { score, indexes: range(sub, q.len()) });
    }

    subsequence(&q_no_space, &orig, &t)
}

fn initials(q: &[char], orig: &[char], t: &[char]) -> Option<Vec<usize>> {
    let mut idx = Vec::new();
    let mut qi = 0;
    for (i, &c) in t.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if is_word_start(orig, i) && c == q[qi] {
            idx.push(i);
            qi += 1;
        }
    }
    (qi == q.len() && q.len() >= 2).then_some(idx)
}

fn subsequence(q: &[char], orig: &[char], t: &[char]) -> Option<Match> {
    if q.len() < 2 {
        return None;
    }
    let mut idx = Vec::with_capacity(q.len());
    let (mut qi, mut score, mut run, mut last) = (0usize, 0.0f64, 0.0f64, -2isize);
    for (i, &c) in t.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if c != q[qi] {
            continue;
        }
        idx.push(i);
        run = if i as isize == last + 1 { run + 1.0 } else { 1.0 };
        score += 10.0 + run * 6.0 + if is_word_start(orig, i) { 14.0 } else { 0.0 };
        last = i as isize;
        qi += 1;
    }
    if qi < q.len() {
        return None;
    }
    let spread = (idx[idx.len() - 1] - idx[0] + 1) as f64;
    let density = q.len() as f64 / spread;
    // Very scattered matches are noise.
    if density < 0.25 && q.len() < 5 {
        return None;
    }
    let score = (200.0 + score * density - (idx[0] as f64).min(30.0)).min(560.0);
    Some(Match { score, indexes: idx })
}

/// Best match of the query against several fields; the title's indexes are used for highlights.
pub fn match_fields(query: &str, title: &str, extra: &[&str], extra_weight: f64) -> Option<Match> {
    let main = fuzzy_match(query, title);
    let mut best = main.clone();
    for field in extra {
        if field.is_empty() {
            continue;
        }
        if let Some(m) = fuzzy_match(query, field) {
            let weighted = m.score * extra_weight;
            if weighted > best.as_ref().map_or(0.0, |b| b.score) {
                best = Some(Match { score: weighted, indexes: main.as_ref().map(|m| m.indexes.clone()).unwrap_or_default() });
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_match_types() {
        let exact = fuzzy_match("firefox", "Firefox").unwrap().score;
        let prefix = fuzzy_match("fire", "Firefox").unwrap().score;
        let word = fuzzy_match("code", "Visual Studio Code").unwrap().score;
        let initials = fuzzy_match("vsc", "Visual Studio Code").unwrap();
        let sub = fuzzy_match("efo", "Firefox").unwrap().score;
        assert!(exact > prefix && prefix > word && word > initials.score && initials.score > sub);
        assert_eq!(initials.indexes, vec![0, 7, 14]);
    }

    #[test]
    fn subsequence_and_misses() {
        assert!(fuzzy_match("projprop", "Project Proposal.docx").is_some());
        assert!(fuzzy_match("ppd", "Project Proposal.docx").is_some());
        assert!(fuzzy_match("xyz", "Firefox").is_none());
        assert!(fuzzy_match("toolongquery", "short").is_none());
    }

    #[test]
    fn camel_case_boundaries() {
        let m = fuzzy_match("term", "gnomeTerminal").unwrap();
        assert_eq!(m.indexes, vec![5, 6, 7, 8]);
    }

    #[test]
    fn extra_fields_are_weighted() {
        let m = match_fields("browser", "Firefox", &["Web Browser"], 0.75).unwrap();
        assert!(m.score < 820.0 && m.score > 400.0);
        assert!(m.indexes.is_empty());
    }
}
