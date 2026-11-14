//! "Lit glass" design tokens, computed for the current theme and accent and filled into
//! style.css. GTK 4.14 has no CSS variables or color-mix(), so the mixing happens here.

use lumen_core::store::ThemeMode;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

pub const fn rgba(r: u8, g: u8, b: u8, a: f64) -> Rgba {
    Rgba { r: r as f64, g: g as f64, b: b as f64, a }
}

impl Rgba {
    pub fn hex(s: &str) -> Option<Rgba> {
        let s = s.strip_prefix('#')?;
        if s.len() != 6 {
            return None;
        }
        let n = u32::from_str_radix(s, 16).ok()?;
        Some(rgba((n >> 16) as u8, (n >> 8) as u8, n as u8, 1.0))
    }
    pub fn alpha(self, a: f64) -> Rgba {
        Rgba { a, ..self }
    }
    pub fn css(self) -> String {
        format!("rgba({:.0},{:.0},{:.0},{:.3})", self.r.clamp(0.0, 255.0), self.g.clamp(0.0, 255.0), self.b.clamp(0.0, 255.0), self.a.clamp(0.0, 1.0))
    }
}

/// CSS color-mix(): `t` of `a` and the rest of `b`, interpolated with premultiplied alpha.
pub fn mix(a: Rgba, t: f64, b: Rgba) -> Rgba {
    let alpha = a.a * t + b.a * (1.0 - t);
    if alpha <= 0.0 {
        return rgba(0, 0, 0, 0.0);
    }
    let ch = |x: f64, y: f64| (x * a.a * t + y * b.a * (1.0 - t)) / alpha;
    Rgba { r: ch(a.r, b.r), g: ch(a.g, b.g), b: ch(a.b, b.b), a: alpha }
}

const TRANSPARENT: Rgba = rgba(0, 0, 0, 0.0);
const WHITE: Rgba = rgba(255, 255, 255, 1.0);
const BLACK: Rgba = rgba(0, 0, 0, 1.0);

struct Palette {
    glass_tint: Rgba,
    glass_alpha: f64,
    glass_alpha_blur: f64,
    glass_solid: Rgba,
    glass_edge: Rgba,
    glass_rim: Rgba,
    glass_sheen: Rgba,
    glass_glow: f64,
    text: Rgba,
    text_2: Rgba,
    text_3: Rgba,
    text_4: Rgba,
    fill_1: Rgba,
    fill_2: Rgba,
    fill_3: Rgba,
    fill_inset: Rgba,
    stroke_hairline: Rgba,
    stroke: Rgba,
    stroke_strong: Rgba,
    lit_fill: Rgba,
    lit_edge: Rgba,
    lit_tint: f64,
    lit_glow: f64,
    kbd_fill: Rgba,
    kbd_stroke: Rgba,
    danger: Rgba,
    success: Rgba,
    warning: Rgba,
    shadow: Rgba,
    shadow_strength: f64,
}

fn dark() -> Palette {
    Palette {
        glass_tint: rgba(17, 19, 28, 1.0),
        glass_alpha: 0.82,
        glass_alpha_blur: 0.46,
        glass_solid: rgba(24, 26, 36, 0.97),
        glass_edge: rgba(255, 255, 255, 0.16),
        glass_rim: rgba(255, 255, 255, 0.09),
        glass_sheen: rgba(255, 255, 255, 0.06),
        glass_glow: 0.16,
        text: rgba(246, 247, 252, 0.95),
        text_2: rgba(222, 226, 240, 0.64),
        text_3: rgba(222, 226, 240, 0.42),
        text_4: rgba(222, 226, 240, 0.28),
        fill_1: rgba(255, 255, 255, 0.04),
        fill_2: rgba(255, 255, 255, 0.07),
        fill_3: rgba(255, 255, 255, 0.11),
        fill_inset: rgba(0, 0, 0, 0.26),
        stroke_hairline: rgba(255, 255, 255, 0.07),
        stroke: rgba(255, 255, 255, 0.10),
        stroke_strong: rgba(255, 255, 255, 0.16),
        lit_fill: rgba(255, 255, 255, 0.095),
        lit_edge: rgba(255, 255, 255, 0.16),
        lit_tint: 0.14,
        lit_glow: 0.55,
        kbd_fill: rgba(255, 255, 255, 0.07),
        kbd_stroke: rgba(255, 255, 255, 0.12),
        danger: rgba(0xff, 0x6b, 0x7a, 1.0),
        success: rgba(0x4a, 0xde, 0x80, 1.0),
        warning: rgba(0xfb, 0xbf, 0x24, 1.0),
        shadow: rgba(0, 0, 0, 1.0),
        shadow_strength: 1.0,
    }
}

fn light() -> Palette {
    Palette {
        glass_tint: rgba(248, 249, 253, 1.0),
        glass_alpha: 0.80,
        glass_alpha_blur: 0.54,
        glass_solid: rgba(250, 251, 254, 0.97),
        glass_edge: rgba(255, 255, 255, 0.95),
        glass_rim: rgba(0, 0, 0, 0.08),
        glass_sheen: rgba(255, 255, 255, 0.5),
        glass_glow: 0.11,
        text: rgba(16, 18, 30, 0.94),
        text_2: rgba(36, 40, 62, 0.64),
        text_3: rgba(36, 40, 62, 0.45),
        text_4: rgba(36, 40, 62, 0.30),
        fill_1: rgba(0, 0, 0, 0.03),
        fill_2: rgba(0, 0, 0, 0.05),
        fill_3: rgba(0, 0, 0, 0.08),
        fill_inset: rgba(255, 255, 255, 0.6),
        stroke_hairline: rgba(0, 0, 0, 0.065),
        stroke: rgba(0, 0, 0, 0.09),
        stroke_strong: rgba(0, 0, 0, 0.14),
        lit_fill: rgba(255, 255, 255, 0.72),
        lit_edge: rgba(255, 255, 255, 1.0),
        lit_tint: 0.09,
        lit_glow: 0.40,
        kbd_fill: rgba(0, 0, 0, 0.045),
        kbd_stroke: rgba(0, 0, 0, 0.10),
        danger: rgba(0xe5, 0x48, 0x4d, 1.0),
        success: rgba(0x16, 0xa3, 0x4a, 1.0),
        warning: rgba(0xd9, 0x77, 0x06, 1.0),
        shadow: rgba(30, 34, 70, 1.0),
        shadow_strength: 0.5,
    }
}

/// The finished stylesheet for these settings.
pub fn stylesheet(dark_mode: bool, accent: &str, shell_blur: bool) -> String {
    let p = if dark_mode { dark() } else { light() };
    let accent = Rgba::hex(accent).unwrap_or(rgba(0x7c, 0x6c, 0xff, 1.0));
    let mut t: HashMap<&str, String> = HashMap::new();
    let mut c = |k: &'static str, v: Rgba| {
        t.insert(k, v.css());
    };
    let glass_bg = p.glass_tint.alpha(if shell_blur { p.glass_alpha_blur } else { p.glass_alpha });
    c("glass_bg", glass_bg);
    c("glass_solid", p.glass_solid);
    c("glass_edge", p.glass_edge);
    c("glass_rim", p.glass_rim);
    c("glass_sheen", p.glass_sheen);
    c("glass_tint", p.glass_tint);
    c("glow_1", accent.alpha(p.glass_glow));
    c("glow_2", accent.alpha(p.glass_glow * 0.55));
    c("text", p.text);
    c("text_2", p.text_2);
    c("text_3", p.text_3);
    c("text_4", p.text_4);
    c("text_soft", mix(p.text, 0.88, TRANSPARENT));
    c("on_accent", WHITE);
    c("fill_1", p.fill_1);
    c("fill_2", p.fill_2);
    c("fill_3", p.fill_3);
    c("fill_inset", p.fill_inset);
    c("stroke_hairline", p.stroke_hairline);
    c("stroke", p.stroke);
    c("stroke_strong", p.stroke_strong);
    c("lit_bg", mix(accent, p.lit_tint, p.lit_fill));
    c("lit_edge", p.lit_edge);
    c("kbd_fill", p.kbd_fill);
    c("kbd_stroke", p.kbd_stroke);
    c("danger", p.danger);
    c("danger_soft", p.danger.alpha(0.14));
    c("danger_text", mix(p.danger, 0.8, p.text));
    c("success", p.success);
    c("success_soft", p.success.alpha(0.14));
    c("warning", p.warning);
    c("accent", accent);
    c("accent_soft", accent.alpha(0.18));
    c("accent_stroke", accent.alpha(0.36));
    c("accent_selection", accent.alpha(0.40));
    c("accent_strong", mix(accent, 0.78, p.text));
    c("accent_light", mix(accent, 0.55, WHITE));
    c("accent_glow", accent.alpha(p.lit_glow));
    c("accent_top", mix(accent, 0.88, WHITE));
    c("accent_dark", mix(accent, 0.70, BLACK));
    c("accent_result", mix(accent, 0.45, p.text));
    let s = p.shadow;
    let k = p.shadow_strength;
    c("shadow_1", s.alpha(0.72 * k));
    c("shadow_2", s.alpha(0.5 * k));
    c("shadow_3", s.alpha(if dark_mode { 0.55 } else { 0.12 }));
    c("shadow_4", s.alpha(0.7 * k));
    c("shadow_control", s.alpha(if dark_mode { 0.3 } else { 0.18 }));
    c("shadow_icon", s.alpha(if dark_mode { 0.3 } else { 0.22 }));
    c("shadow_row", s.alpha(0.5 * k));

    let template = include_str!("../resources/style.css");
    let mut out = String::with_capacity(template.len() + 512);
    let mut rest = template;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let len = after.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')).unwrap_or(after.len());
        match t.get(&after[..len]) {
            Some(v) => out.push_str(v),
            None => {
                eprintln!("[theme] unknown token ${}", &after[..len]);
                out.push_str("transparent");
            }
        }
        rest = &after[len..];
    }
    out.push_str(rest);
    out.push_str(&tile_css());
    out.push_str(&swatch_css(p.glass_tint));
    out
}

/// One class per curated accent for the settings swatches.
fn swatch_css(ring: Rgba) -> String {
    let mut s = String::new();
    for (_, hex) in crate::ui::settings_accents() {
        let c = Rgba::hex(hex).unwrap_or(BLACK);
        let key = &hex[1..];
        s.push_str(&format!(
            "button.swatch.c-{key} {{ background-image: radial-gradient(circle at 35% 30%, {}, {} 70%); box-shadow: inset 0 0 0 1px rgba(255,255,255,0.22), 0 2px 6px -2px {}; }}\n\
             button.swatch.c-{key}.on {{ box-shadow: 0 0 0 2px {}, 0 0 0 4px {}; }}\n",
            mix(c, 0.7, WHITE).css(),
            c.css(),
            c.css(),
            ring.css(),
            c.css()
        ));
    }
    s
}

/// macOS-style colourful tiles for built-in result types.
pub const TINTS: &[(&str, &str, &str)] = &[
    ("calculator", "#ffb547", "#ff7a1a"),
    ("globe", "#5ac8fa", "#2a7fff"),
    ("search", "#6ea8ff", "#3b5bff"),
    ("terminal", "#4b5563", "#111827"),
    ("lock", "#8b9bb4", "#4a5568"),
    ("logout", "#f78ca0", "#e0457b"),
    ("moon", "#8f7bff", "#4b3fd6"),
    ("sun", "#ffd76a", "#ff9f1c"),
    ("power", "#ff7a7a", "#e0313f"),
    ("restart", "#7ee0b5", "#16a37a"),
    ("settings", "#a0a7b8", "#5c6478"),
    ("folder", "#7cc4ff", "#3a8dde"),
    ("file", "#c3cad8", "#8a93a6"),
    ("trash", "#9aa5b8", "#5a6477"),
    ("wifi", "#59d8ff", "#1e9bd7"),
    ("bell", "#c28dff", "#8b48e8"),
    ("refresh", "#71e1c6", "#1aa889"),
    ("quit", "#ff9a9a", "#d94848"),
    ("sparkles", "#b69cff", "#7c6cff"),
    ("copy", "#a0a7b8", "#5c6478"),
    ("keyboard", "#b69cff", "#7c6cff"),
    ("clipboard", "#ffd27a", "#e8912d"),
    ("pin", "#ff9eb5", "#e0457b"),
    ("paste", "#ffd27a", "#e8912d"),
];

fn tile_css() -> String {
    let mut s = String::new();
    for (name, a, b) in TINTS {
        let shade = Rgba::hex(b).unwrap_or(BLACK).alpha(0.9).css();
        s.push_str(&format!(
            ".tile.tint-{name} {{ background-image: linear-gradient(160deg, {a}, {b}); box-shadow: inset 0 1px rgba(255,255,255,0.35), inset 0 -1px rgba(0,0,0,0.15), 0 3px 8px -3px {shade}; }}\n"
        ));
    }
    s
}

/// Whether the desktop prefers dark, for ThemeMode::System.
pub fn resolve_dark(mode: ThemeMode, system_dark: bool) -> bool {
    match mode {
        ThemeMode::System => system_dark,
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixing_matches_css() {
        let m = mix(rgba(255, 0, 0, 1.0), 0.18, TRANSPARENT);
        assert_eq!((m.r, m.g, m.a), (255.0, 0.0, 0.18));
        let m = mix(WHITE, 0.5, BLACK);
        assert_eq!(m.r.round(), 128.0);
    }

    #[test]
    fn every_token_resolves() {
        for dark in [true, false] {
            let css = stylesheet(dark, "#7c6cff", false);
            assert!(!css.contains('$'), "unfilled token");
        }
    }
}
