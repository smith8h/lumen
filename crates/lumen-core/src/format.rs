//! Number, size and date formatting (en-US style, like the rest of Lumen's text).

use chrono::{Datelike, Local, TimeZone};

/// "1234567" → "1,234,567" (keeps a leading minus sign).
pub fn group_thousands(int: &str) -> String {
    let (sign, digits) = int.strip_prefix('-').map_or(("", int), |d| ("-", d));
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{sign}{out}")
}

fn group_decimal(s: &str) -> String {
    match s.split_once('.') {
        Some((int, frac)) => format!("{}.{frac}", group_thousands(int)),
        None => group_thousands(s),
    }
}

/// Round to `digits` significant digits and write it out, mathjs-style: plain decimals with
/// grouped thousands, exponent notation below 1e-9 and from 1e21. None for NaN/∞.
pub fn fmt_number(v: f64, digits: usize) -> Option<String> {
    if !v.is_finite() {
        return None;
    }
    if v == 0.0 {
        return Some("0".into());
    }
    if v.fract() == 0.0 && v.abs() < 1e21 {
        return Some(group_thousands(&format!("{v:.0}")));
    }
    let sci = format!("{:.*e}", digits.saturating_sub(1), v);
    let (mantissa, exp) = sci.split_once('e')?;
    let exp: i32 = exp.parse().ok()?;
    let rounded: f64 = sci.parse().ok()?;
    if rounded.abs() < 1e-9 || rounded.abs() >= 1e21 {
        let m = trim_zeros(mantissa);
        return Some(format!("{m}e{}{}", if exp < 0 { "-" } else { "+" }, exp.abs()));
    }
    let (neg, m) = mantissa.strip_prefix('-').map_or((false, mantissa), |m| (true, m));
    let digits_only: String = m.chars().filter(|c| c.is_ascii_digit()).collect();
    let point = exp + 1; // position of the decimal point within digits_only
    let s = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits_only)
    } else if point as usize >= digits_only.len() {
        format!("{}{}", digits_only, "0".repeat(point as usize - digits_only.len()))
    } else {
        format!("{}.{}", &digits_only[..point as usize], &digits_only[point as usize..])
    };
    let s = trim_zeros(&s);
    let s = group_decimal(&s);
    Some(if neg && s != "0" { format!("-{s}") } else { s })
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
}

/// At most `digits` decimals, trailing zeros dropped, thousands grouped: 1234.5 → "1,234.5".
pub fn fmt_fixed(v: f64, digits: usize) -> String {
    let s = trim_zeros(&format!("{v:.digits$}"));
    let s = if s == "-0" { "0".to_string() } else { s };
    group_decimal(&s)
}

pub fn count(n: usize) -> String {
    group_thousands(&n.to_string())
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", count(n), if n == 1 { one } else { many })
}

fn local(ms: i64) -> Option<chrono::DateTime<Local>> {
    Local.timestamp_millis_opt(ms).single()
}

/// "Nov 15, 2026"
pub fn date(ms: i64) -> String {
    local(ms).map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default()
}

/// "Nov 15, 2026, 3:04 PM"
pub fn date_time(ms: i64) -> String {
    local(ms).map(|d| d.format("%b %-d, %Y, %-I:%M %p").to_string()).unwrap_or_default()
}

/// File times in the preview pane: "Just now", "5 min ago", "Today, 3:04 PM", "Nov 15, 2026".
pub fn when(ms: i64, now: i64) -> String {
    let diff = now - ms;
    if diff < 60_000 {
        return "Just now".into();
    }
    if diff < 3_600_000 {
        return format!("{} min ago", (diff as f64 / 60_000.0).round() as i64);
    }
    if let (Some(d), Some(n)) = (local(ms), local(now))
        && diff < 86_400_000
        && d.day() == n.day()
    {
        return format!("Today, {}", d.format("%-I:%M %p"));
    }
    date(ms)
}

/// Clip ages: "Just now", "5 min ago", "3 h ago", "Yesterday", "4 days ago", then the date.
pub fn ago(ms: i64, now: i64) -> String {
    let s = ((now - ms) as f64 / 1000.0).round().max(0.0) as i64;
    if s < 45 {
        return "Just now".into();
    }
    let m = (s as f64 / 60.0).round() as i64;
    if m < 60 {
        return format!("{m} min ago");
    }
    let h = (m as f64 / 60.0).round() as i64;
    if h < 24 {
        return format!("{h} h ago");
    }
    let d = (h as f64 / 24.0).round() as i64;
    if d < 30 {
        return if d == 1 { "Yesterday".into() } else { format!("{d} days ago") };
    }
    date(ms)
}

/// 1536 → "1.5 KB"
pub fn size(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut v = n as f64 / 1024.0;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v < 10.0 { format!("{v:.1} {}", units[i]) } else { format!("{v:.0} {}", units[i]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(fmt_number(1024.0, 12).unwrap(), "1,024");
        assert_eq!(fmt_number(-1234567.0, 12).unwrap(), "-1,234,567");
        assert_eq!(fmt_number(0.1 + 0.2, 12).unwrap(), "0.3");
        assert_eq!(fmt_number(1.0 / 3.0, 12).unwrap(), "0.333333333333");
        assert_eq!(fmt_number(12345.678, 12).unwrap(), "12,345.678");
        assert_eq!(fmt_number(1.5e-10, 12).unwrap(), "1.5e-10");
        assert_eq!(fmt_number(2e21, 12).unwrap(), "2e+21");
        assert_eq!(fmt_number(0.000123, 12).unwrap(), "0.000123");
        assert!(fmt_number(f64::NAN, 12).is_none());
        assert_eq!(fmt_fixed(1234.5, 2), "1,234.5");
        assert_eq!(fmt_fixed(50.0, 4), "50");
    }

    #[test]
    fn sizes_and_ages() {
        assert_eq!(size(512), "512 B");
        assert_eq!(size(1536), "1.5 KB");
        assert_eq!(size(50 * 1024 * 1024), "50 MB");
        assert_eq!(ago(0, 10_000), "Just now");
        assert_eq!(ago(0, 5 * 60_000), "5 min ago");
        assert_eq!(ago(0, 3 * 3_600_000), "3 h ago");
        assert_eq!(ago(0, 30 * 3_600_000), "Yesterday");
    }
}
