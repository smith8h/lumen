//! Calculator, unit and currency conversion.
//!   2^10 · sqrt(2) · 15% of 80 · 100 + 10% · 5 km to mi · 30 c to f · 5 GB to MB · 255 to hex · 100 usd to iqd

use regex::Regex;
use std::sync::LazyLock;

use crate::format::{fmt_fixed, fmt_number};
use crate::rates::Rates;
use crate::types::*;
use crate::units::{self, Dim, NONE, UnitDef};

// ── Values ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
struct Unit {
    name: String,
    factor: f64,
    offset: f64,
    dim: Dim,
}

impl Unit {
    fn from_def(def: &UnitDef) -> Self {
        Unit { name: def.names[0].to_string(), factor: def.factor, offset: def.offset, dim: def.dim }
    }
}

#[derive(Clone, Debug)]
struct Val {
    /// The value in SI base units (or the plain number when dimensionless).
    si: f64,
    dim: Dim,
    /// The unit to show the value in.
    unit: Option<Unit>,
    /// Written as a percentage ("10%"), for `100 + 10%`.
    pct: bool,
    /// A ready-made textual result (hex/bin/oct output).
    text: Option<String>,
}

type R<T> = Result<T, String>;

impl Val {
    fn num(v: f64) -> Val {
        Val { si: v, dim: NONE, unit: None, pct: false, text: None }
    }
    fn qty(v: f64, unit: Unit) -> Val {
        Val { si: v * unit.factor + unit.offset, dim: unit.dim, unit: Some(unit), pct: false, text: None }
    }
    fn dimensionless(&self) -> bool {
        self.dim == NONE
    }
    fn number(&self) -> R<f64> {
        if self.dimensionless() { Ok(self.si) } else { Err("expected a plain number".into()) }
    }
}

fn add_dims(a: Dim, b: Dim, sign: i8) -> Dim {
    let mut out = a;
    for i in 0..out.len() {
        out[i] += sign * b[i];
    }
    out
}

fn compose(a: &Option<Unit>, b: &Option<Unit>, op: char) -> Option<Unit> {
    let (a, b) = (a.as_ref()?, b.as_ref()?);
    let name = if op == '*' && a.name == b.name { format!("{}^2", a.name) } else { format!("{}{op}{}", a.name, b.name) };
    let factor = if op == '*' { a.factor * b.factor } else { a.factor / b.factor };
    Some(Unit { name, factor, offset: 0.0, dim: add_dims(a.dim, b.dim, if op == '*' { 1 } else { -1 }) })
}

fn add(a: Val, b: Val, sign: f64) -> R<Val> {
    if b.pct && b.dimensionless() && !a.pct {
        // 100 + 10% = 110
        let mut out = a.clone();
        let base = a.unit.as_ref().map_or(a.si, |u| (a.si - u.offset) / u.factor);
        let v = base * (1.0 + sign * b.si);
        out.si = a.unit.as_ref().map_or(v, |u| v * u.factor + u.offset);
        return Ok(out);
    }
    if a.dim != b.dim {
        return Err("units don't match".into());
    }
    let unit = a.unit.clone().or(b.unit.clone());
    // Temperatures add as differences: 20 °C + 5 °C = 25 °C.
    let offset = unit.as_ref().map_or(0.0, |u| u.offset);
    let si = if a.unit.is_some() && b.unit.is_some() { a.si + sign * (b.si - offset) } else { a.si + sign * b.si };
    Ok(Val { si, dim: a.dim, unit, pct: a.pct && b.pct, text: None })
}

fn mul(a: Val, b: Val) -> Val {
    let dim = add_dims(a.dim, b.dim, 1);
    let unit = match (&a.unit, &b.unit) {
        (Some(_), None) => a.unit.clone(),
        (None, Some(_)) => b.unit.clone(),
        _ => compose(&a.unit, &b.unit, '*'),
    };
    // Scaling a temperature: scale the shown value, not Kelvin.
    let si = match (&a.unit, &b.unit) {
        (Some(u), None) if u.offset != 0.0 => ((a.si - u.offset) * b.si) + u.offset,
        (None, Some(u)) if u.offset != 0.0 => ((b.si - u.offset) * a.si) + u.offset,
        _ => a.si * b.si,
    };
    Val { si, dim, unit: if dim == NONE { None } else { unit }, pct: false, text: None }
}

fn div(a: Val, b: Val) -> R<Val> {
    if b.si == 0.0 {
        return Err("division by zero".into());
    }
    let dim = add_dims(a.dim, b.dim, -1);
    let unit = match (&a.unit, &b.unit) {
        (Some(_), None) => a.unit.clone(),
        _ => compose(&a.unit, &b.unit, '/'),
    };
    let si = match (&a.unit, &b.unit) {
        (Some(u), None) if u.offset != 0.0 => ((a.si - u.offset) / b.si) + u.offset,
        _ => a.si / b.si,
    };
    Ok(Val { si, dim, unit: if dim == NONE { None } else { unit }, pct: false, text: None })
}

fn pow(a: Val, b: Val) -> R<Val> {
    let e = b.number()?;
    if a.dimensionless() {
        return Ok(Val::num(a.si.powf(e)));
    }
    if e.fract() != 0.0 || e.abs() > 6.0 {
        return Err("units need a small whole power".into());
    }
    let n = e as i8;
    let mut dim = a.dim;
    for d in dim.iter_mut() {
        *d *= n;
    }
    let unit = a.unit.map(|u| Unit { name: format!("{}^{n}", u.name), factor: u.factor.powi(n as i32), offset: 0.0, dim });
    Ok(Val { si: a.si.powi(n as i32), dim, unit, pct: false, text: None })
}

fn factorial(n: f64) -> R<f64> {
    if n < 0.0 || n.fract() != 0.0 {
        return Err("factorial needs a whole number".into());
    }
    if n > 170.0 {
        return Ok(f64::INFINITY);
    }
    Ok((1..=n as u64).fold(1.0, |acc, k| acc * k as f64))
}

fn gcd(mut a: f64, mut b: f64) -> f64 {
    a = a.abs().round();
    b = b.abs().round();
    while b > 0.0 {
        (a, b) = (b, a % b);
    }
    a
}

// ── Tokens ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
}

fn tokenize(s: &str) -> R<Vec<Tok>> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            if c == '0' && matches!(chars.get(i + 1), Some('x' | 'X' | 'b' | 'B' | 'o' | 'O')) {
                let radix = match chars[i + 1].to_ascii_lowercase() {
                    'x' => 16,
                    'b' => 2,
                    _ => 8,
                };
                let start = i + 2;
                let mut j = start;
                while j < chars.len() && chars[j].is_digit(radix) {
                    j += 1;
                }
                if j > start {
                    let digits: String = chars[start..j].iter().collect();
                    out.push(Tok::Num(u64::from_str_radix(&digits, radix).map_err(|e| e.to_string())? as f64));
                    i = j;
                    continue;
                }
            }
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // Exponent: 1e3, 2.5E-4 (but not "2e" or "2 exp").
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Tok::Num(text.parse::<f64>().map_err(|_| format!("bad number {text}"))?));
        } else if c.is_alphabetic() || c == '_' || c == '°' || c == 'µ' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '°' || chars[i] == '²' || chars[i] == '³') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if c == '*' && chars.get(i + 1) == Some(&'*') {
            out.push(Tok::Op('^'));
            i += 2;
        } else if "+-*/^%!(),".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else {
            return Err(format!("unexpected {c}"));
        }
    }
    Ok(out)
}

// ── Parser ─────────────────────────────────────────────────────────────────

const FUNCTIONS: &[&str] = &[
    "sqrt",
    "cbrt",
    "abs",
    "exp",
    "ln",
    "log",
    "log10",
    "log2",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "sinh",
    "cosh",
    "tanh",
    "floor",
    "ceil",
    "round",
    "trunc",
    "sign",
    "min",
    "max",
    "pow",
    "mod",
    "hypot",
    "fact",
    "factorial",
    "gcd",
    "lcm",
    "root",
    "nthroot",
    "hex",
    "bin",
    "oct",
];
const CONSTANTS: &[(&str, f64)] = &[
    ("pi", std::f64::consts::PI),
    ("π", std::f64::consts::PI),
    ("e", std::f64::consts::E),
    ("tau", std::f64::consts::TAU),
    ("phi", 1.618_033_988_749_895),
];
const KEYWORDS: &[&str] = &["to", "in", "as", "mod", "of"];

fn constant(name: &str) -> Option<f64> {
    CONSTANTS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| *v)
}

/// What a conversion asks for: a unit, or a base for whole numbers.
enum Target {
    Unit(Unit),
    Radix(u32),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.pos + n)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn is_op(&self, c: char) -> bool {
        self.peek() == Some(&Tok::Op(c))
    }
    fn is_ident(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(x)) if x.eq_ignore_ascii_case(s))
    }
    fn expect(&mut self, c: char) -> R<()> {
        if self.next() == Some(Tok::Op(c)) { Ok(()) } else { Err(format!("expected {c}")) }
    }

    /// Whether the "in" at the cursor means "convert to" (`6 ft in m`) rather than inches (`12 in to cm`).
    fn in_is_keyword(&self) -> bool {
        matches!(self.peek_at(1), Some(Tok::Ident(next)) if !KEYWORDS.contains(&next.to_lowercase().as_str()))
    }

    fn is_conversion(&self) -> bool {
        match self.peek() {
            Some(Tok::Ident(w)) => match w.to_lowercase().as_str() {
                "to" | "as" => true,
                "in" => self.in_is_keyword(),
                _ => false,
            },
            _ => false,
        }
    }

    fn parse(mut self) -> R<(Val, Option<Target>)> {
        let value = self.additive()?;
        let target = if self.is_conversion() {
            self.next();
            Some(self.target()?)
        } else {
            None
        };
        if self.pos < self.toks.len() {
            return Err("unexpected input".into());
        }
        Ok((value, target))
    }

    fn target(&mut self) -> R<Target> {
        if let Some(Tok::Ident(w)) = self.peek() {
            let radix = match w.to_lowercase().as_str() {
                "hex" | "hexadecimal" => Some(16),
                "bin" | "binary" => Some(2),
                "oct" | "octal" => Some(8),
                "dec" | "decimal" => Some(10),
                _ => None,
            };
            if let Some(r) = radix {
                self.next();
                return Ok(Target::Radix(r));
            }
        }
        Ok(Target::Unit(self.unit_expr()?.ok_or("expected a unit")?))
    }

    /// km, km/h, m^2, kg*m/s^2 …
    fn unit_expr(&mut self) -> R<Option<Unit>> {
        let Some(mut unit) = self.unit_atom()? else { return Ok(None) };
        while let Some(Tok::Op(op @ ('/' | '*'))) = self.peek() {
            let op = *op;
            let Some(Tok::Ident(name)) = self.peek_at(1) else { break };
            if !units::is_unit(name) || FUNCTIONS.contains(&name.as_str()) {
                break;
            }
            self.next();
            let rhs = self.unit_atom()?.ok_or("expected a unit")?;
            if unit.offset != 0.0 || rhs.offset != 0.0 {
                return Err("temperatures can't be combined".into());
            }
            unit = compose(&Some(unit), &Some(rhs), op).unwrap();
        }
        Ok(Some(unit))
    }

    fn unit_atom(&mut self) -> R<Option<Unit>> {
        let Some(Tok::Ident(name)) = self.peek() else { return Ok(None) };
        let Some(def) = units::lookup(name) else { return Ok(None) };
        self.next();
        let mut unit = Unit::from_def(def);
        if self.is_op('^')
            && let Some(Tok::Num(n)) = self.peek_at(1)
            && n.fract() == 0.0
            && n.abs() <= 6.0
        {
            let n = *n as i8;
            self.pos += 2;
            if unit.offset != 0.0 {
                return Err("temperatures can't be raised to a power".into());
            }
            let mut dim = unit.dim;
            for d in dim.iter_mut() {
                *d *= n;
            }
            unit = Unit { name: format!("{}^{n}", unit.name), factor: unit.factor.powi(n as i32), offset: 0.0, dim };
        }
        Ok(Some(unit))
    }

    fn additive(&mut self) -> R<Val> {
        let mut left = self.term()?;
        while let Some(Tok::Op(c @ ('+' | '-'))) = self.peek() {
            let sign = if *c == '+' { 1.0 } else { -1.0 };
            self.next();
            let right = self.term()?;
            left = add(left, right, sign)?;
        }
        Ok(left)
    }

    fn starts_operand(&self) -> bool {
        match self.peek() {
            Some(Tok::Num(_)) | Some(Tok::Op('(')) => true,
            Some(Tok::Ident(w)) => !KEYWORDS.contains(&w.to_lowercase().as_str()),
            _ => false,
        }
    }

    fn term(&mut self) -> R<Val> {
        let mut left = self.unary()?;
        loop {
            if self.is_op('*') {
                self.next();
                left = mul(left, self.unary()?);
            } else if self.is_op('/') {
                self.next();
                left = div(left, self.unary()?)?;
            } else if self.is_op('%') || self.is_ident("mod") {
                self.next();
                let right = self.unary()?;
                let (a, b) = (left.number()?, right.number()?);
                if b == 0.0 {
                    return Err("modulo by zero".into());
                }
                left = Val::num(a - b * (a / b).floor());
            } else if self.starts_operand() {
                // Implicit multiplication: 2pi, 3(4+5), 2 sqrt(2)
                left = mul(left, self.unary()?);
            } else {
                return Ok(left);
            }
        }
    }

    fn unary(&mut self) -> R<Val> {
        if self.is_op('-') {
            self.next();
            let v = self.unary()?;
            return Ok(mul(Val::num(-1.0), v));
        }
        if self.is_op('+') {
            self.next();
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> R<Val> {
        let base = self.postfix()?;
        if self.is_op('^') {
            self.next();
            let exp = self.unary()?;
            return pow(base, exp);
        }
        Ok(base)
    }

    fn postfix(&mut self) -> R<Val> {
        let mut v = self.primary()?;
        loop {
            if self.is_op('!') {
                self.next();
                v = Val::num(factorial(v.number()?)?);
            } else if self.is_op('%') && !self.percent_is_modulo() {
                self.next();
                let n = v.number()?;
                v = Val { pct: true, ..Val::num(n / 100.0) };
            } else {
                return Ok(v);
            }
        }
    }

    /// "10 % 3" is modulo; "15%", "15% * 80" and "(15%)" are percentages.
    fn percent_is_modulo(&self) -> bool {
        match self.peek_at(1) {
            Some(Tok::Num(_)) | Some(Tok::Op('(')) => true,
            Some(Tok::Ident(w)) => !KEYWORDS.contains(&w.to_lowercase().as_str()),
            _ => false,
        }
    }

    fn primary(&mut self) -> R<Val> {
        match self.next() {
            Some(Tok::Num(n)) => {
                // "5 km", "12 in to cm" (but "12 ft in m" converts).
                if let Some(Tok::Ident(w)) = self.peek()
                    && units::is_unit(w)
                    && constant(w).is_none()
                    && !FUNCTIONS.contains(&w.as_str())
                    && !(w.eq_ignore_ascii_case("in") && self.in_is_keyword())
                {
                    let unit = self.unit_expr()?.ok_or("expected a unit")?;
                    return Ok(Val::qty(n, unit));
                }
                Ok(Val::num(n))
            }
            Some(Tok::Op('(')) => {
                let v = self.additive()?;
                self.expect(')')?;
                Ok(v)
            }
            Some(Tok::Ident(name)) => {
                let lower = name.to_lowercase();
                if FUNCTIONS.contains(&lower.as_str()) && self.is_op('(') {
                    self.next();
                    let mut args = Vec::new();
                    if !self.is_op(')') {
                        args.push(self.additive()?);
                        while self.is_op(',') {
                            self.next();
                            args.push(self.additive()?);
                        }
                    }
                    self.expect(')')?;
                    return call(&lower, args);
                }
                if let Some(c) = constant(&name) {
                    return Ok(Val::num(c));
                }
                if let Some(def) = units::lookup(&name) {
                    return Ok(Val::qty(1.0, Unit::from_def(def)));
                }
                Err(format!("unknown name {name}"))
            }
            _ => Err("expected a value".into()),
        }
    }
}

fn radians(v: &Val) -> R<f64> {
    if v.dim == [0, 0, 0, 0, 0, 1] { Ok(v.si) } else { v.number() }
}

fn call(name: &str, args: Vec<Val>) -> R<Val> {
    let n = |i: usize| -> R<f64> { args.get(i).ok_or_else(|| format!("{name} needs more arguments"))?.number() };
    let one = |f: fn(f64) -> f64| -> R<Val> { Ok(Val::num(f(n(0)?))) };
    let radix = |r: u32, prefix: &str| -> R<Val> {
        let x = n(0)?;
        if x.fract() != 0.0 || x.abs() > 9.007_199_254_740_991e15 {
            return Err("needs a whole number".into());
        }
        let digits = match r {
            16 => format!("{:x}", x.abs() as u64),
            2 => format!("{:b}", x.abs() as u64),
            _ => format!("{:o}", x.abs() as u64),
        };
        Ok(Val { text: Some(format!("{}{prefix}{digits}", if x < 0.0 { "-" } else { "" })), ..Val::num(x) })
    };
    match name {
        "sqrt" => {
            let a = args.first().ok_or("sqrt needs a value")?;
            if a.dimensionless() {
                return one(f64::sqrt);
            }
            if a.dim.iter().all(|d| d % 2 == 0) {
                let mut dim = a.dim;
                dim.iter_mut().for_each(|d| *d /= 2);
                return Ok(Val { si: a.si.sqrt(), dim, unit: None, pct: false, text: None });
            }
            Err("can't take that root".into())
        }
        "cbrt" => one(f64::cbrt),
        "abs" => {
            let mut a = args.into_iter().next().ok_or("abs needs a value")?;
            a.si = a.si.abs();
            Ok(a)
        }
        "exp" => one(f64::exp),
        "ln" => one(f64::ln),
        "log" if args.len() == 2 => Ok(Val::num(n(0)?.ln() / n(1)?.ln())),
        // Like mathjs (and most programming languages), log is the natural logarithm.
        "log" => one(f64::ln),
        "log10" => one(f64::log10),
        "log2" => one(f64::log2),
        "sin" => Ok(Val::num(radians(args.first().ok_or("sin needs a value")?)?.sin())),
        "cos" => Ok(Val::num(radians(args.first().ok_or("cos needs a value")?)?.cos())),
        "tan" => Ok(Val::num(radians(args.first().ok_or("tan needs a value")?)?.tan())),
        "asin" => one(f64::asin),
        "acos" => one(f64::acos),
        "atan" => one(f64::atan),
        "atan2" => Ok(Val::num(n(0)?.atan2(n(1)?))),
        "sinh" => one(f64::sinh),
        "cosh" => one(f64::cosh),
        "tanh" => one(f64::tanh),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "trunc" => one(f64::trunc),
        "sign" => one(f64::signum),
        "round" if args.len() == 2 => {
            let p = 10f64.powi(n(1)? as i32);
            Ok(Val::num((n(0)? * p).round() / p))
        }
        "round" => one(f64::round),
        "min" | "max" => {
            let vals: R<Vec<f64>> = (0..args.len()).map(n).collect();
            let vals = vals?;
            if vals.is_empty() {
                return Err(format!("{name} needs values"));
            }
            let pick = if name == "min" { f64::min } else { f64::max };
            Ok(Val::num(vals.into_iter().reduce(pick).unwrap()))
        }
        "pow" => pow(args.first().cloned().ok_or("pow needs two values")?, args.get(1).cloned().ok_or("pow needs two values")?),
        "mod" => {
            let (a, b) = (n(0)?, n(1)?);
            if b == 0.0 { Err("modulo by zero".into()) } else { Ok(Val::num(a - b * (a / b).floor())) }
        }
        "hypot" => Ok(Val::num(n(0)?.hypot(n(1)?))),
        "fact" | "factorial" => Ok(Val::num(factorial(n(0)?)?)),
        "gcd" => Ok(Val::num(gcd(n(0)?, n(1)?))),
        "lcm" => {
            let (a, b) = (n(0)?, n(1)?);
            let g = gcd(a, b);
            Ok(Val::num(if g == 0.0 { 0.0 } else { (a * b).abs() / g }))
        }
        "root" | "nthroot" => {
            let (x, r) = (n(0)?, args.get(1).map(|v| v.number()).transpose()?.unwrap_or(2.0));
            Ok(Val::num(if x < 0.0 && r % 2.0 == 1.0 { -(-x).powf(1.0 / r) } else { x.powf(1.0 / r) }))
        }
        "hex" => radix(16, "0x"),
        "bin" => radix(2, "0b"),
        "oct" => radix(8, "0o"),
        _ => Err(format!("unknown function {name}")),
    }
}

// ── Evaluation and formatting ──────────────────────────────────────────────

fn display_name(unit: &Unit) -> String {
    unit.name.replace("degC", "°C").replace("degF", "°F")
}

fn format_val(v: &Val, target: Option<Target>) -> R<String> {
    if let Some(t) = &v.text {
        return Ok(t.clone());
    }
    match target {
        Some(Target::Radix(r)) => {
            let x = v.number()?;
            if r == 10 {
                return fmt_number(x, 12).ok_or_else(|| "not finite".into());
            }
            let f = match r {
                16 => "hex",
                2 => "bin",
                _ => "oct",
            };
            call(f, vec![Val::num(x)]).map(|v| v.text.unwrap_or_default())
        }
        Some(Target::Unit(unit)) => {
            if unit.dim != v.dim {
                return Err("units don't match".into());
            }
            let shown = (v.si - unit.offset) / unit.factor;
            Ok(format!("{} {}", fmt_number(shown, 10).ok_or("not finite")?, display_name(&unit)))
        }
        None => match &v.unit {
            Some(u) if v.dim != NONE => {
                let shown = (v.si - u.offset) / u.factor;
                Ok(format!("{} {}", fmt_number(shown, 10).ok_or("not finite")?, display_name(u)))
            }
            _ if v.dimensionless() => fmt_number(v.si, 12).ok_or_else(|| "not finite".into()),
            _ => Err("no unit to show".into()),
        },
    }
}

static UNIT_ALIASES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [(r"(?i)°\s*c\b|\bcelsius\b", "degC"), (r"(?i)°\s*f\b|\bfahrenheit\b", "degF"), (r"(?i)\bkelvin\b", "K")]
        .into_iter()
        .map(|(re, to)| (Regex::new(re).unwrap(), to))
        .collect()
});
static TIMES_X: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d)\s*x\s*(\d)").unwrap());
static PCT_OF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)%\s*of\s+").unwrap());
static C_TO_F: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^([\d.\s+-]+?)\s*c\s+(to|in)\s+f$").unwrap());
static F_TO_C: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^([\d.\s+-]+?)\s*f\s+(to|in)\s+c$").unwrap());
static C_TO_K: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^([\d.\s+-]+?)\s*([cf])\s+(to|in)\s+k$").unwrap());
static HEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b0x[0-9a-f]").unwrap());
static THOUSANDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d),(\d{3})\b").unwrap());

fn normalize(input: &str) -> String {
    let mut s = input.trim().replace(['×', '✕', '·'], "*").replace('÷', "/").replace('−', "-");
    // 1,000,000 → 1000000 (repeat for every group)
    loop {
        let next = THOUSANDS.replace_all(&s, "$1$2").into_owned();
        if next == s {
            break;
        }
        s = next;
    }
    // 3x4 → 3*4 (but leave hex like 0x1f alone)
    while !HEX.is_match(&s) {
        let next = TIMES_X.replace_all(&s, "$1*$2").into_owned();
        if next == s {
            break;
        }
        s = next;
    }
    // 15% of 80 → 15% * 80
    s = PCT_OF.replace_all(&s, "% * ").into_owned();
    // "30 c to f" — bare c/f only when converting temperatures.
    s = C_TO_F.replace(&s, "$1 degC to degF").into_owned();
    s = F_TO_C.replace(&s, "$1 degF to degC").into_owned();
    s = C_TO_K.replace(&s, |c: &regex::Captures| format!("{} deg{} to K", &c[1], c[2].to_uppercase())).into_owned();
    for (re, unit) in UNIT_ALIASES.iter() {
        s = re.replace_all(&s, *unit).into_owned();
    }
    s
}

static MATHY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\d|\b(pi|e|tau|phi|sqrt|sin|cos|tan|log|ln|abs)\b|π").unwrap());
static PLAIN_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\d.\s]+$").unwrap());
static SPACED_DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d\s+\d").unwrap());
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[a-z]+$").unwrap());

fn looks_like_math(q: &str) -> bool {
    if !MATHY.is_match(q) {
        return false;
    }
    // A plain number or a version like 1.2.3 isn't a calculation.
    if PLAIN_NUMBER.is_match(q) && !SPACED_DIGITS.is_match(q) {
        return false;
    }
    !WORD.is_match(q)
}

/// Evaluate an expression to its display string. Public for tests and the "=" prefix.
pub fn evaluate(expr: &str) -> Option<String> {
    let toks = tokenize(&normalize(expr)).ok()?;
    if toks.is_empty() {
        return None;
    }
    let (value, target) = Parser { toks, pos: 0 }.parse().ok()?;
    format_val(&value, target).ok()
}

pub fn plain(s: &str) -> String {
    s.replace(',', "")
}

fn calc_item(expression: &str, result: &str, subtitle: String) -> ResultItem {
    let mut item = ResultItem::new(format!("calc:{expression}"), ResultKind::Calc, result, Action::Copy(plain(result)));
    item.subtitle = Some(subtitle);
    item.icon = Some(IconSpec::Glyph(Glyph::Calculator));
    item.score = 3000.0;
    item.actions = vec![
        SecondaryAction::new("Copy result", Action::Copy(plain(result))).hint("Enter").glyph(Glyph::Copy),
        SecondaryAction::new("Copy expression", Action::Copy(format!("{expression} = {result}"))).glyph(Glyph::Copy),
    ];
    item
}

// ── Currency ───────────────────────────────────────────────────────────────

static CURRENCY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([$€£¥]?)\s*([\d.,]+)\s*([a-z$€£¥]{1,8})?\s+(?:to|in|as|=)\s+([a-z$€£¥]{1,8})$").unwrap());

fn currency_code(s: &str) -> String {
    let words: &[(&str, &str)] = &[
        ("dollar", "USD"),
        ("dollars", "USD"),
        ("$", "USD"),
        ("euro", "EUR"),
        ("euros", "EUR"),
        ("€", "EUR"),
        ("pound", "GBP"),
        ("pounds", "GBP"),
        ("£", "GBP"),
        ("yen", "JPY"),
        ("¥", "JPY"),
        ("dinar", "IQD"),
        ("dinars", "IQD"),
        ("riyal", "SAR"),
        ("lira", "TRY"),
        ("rupee", "INR"),
        ("rupees", "INR"),
        ("dirham", "AED"),
    ];
    let lower = s.to_lowercase();
    words.iter().find(|(w, _)| *w == lower).map_or_else(|| s.to_uppercase(), |(_, c)| c.to_string())
}

fn is_code(s: &str) -> bool {
    s.len() == 3 && s.chars().all(|c| c.is_ascii_uppercase())
}

fn currency(q: &str, rates: &Rates) -> Option<ResultItem> {
    let m = CURRENCY.captures(q.trim())?;
    let sym = m.get(1).map_or("", |x| x.as_str());
    let from = currency_code(if !sym.is_empty() { sym } else { m.get(3).map_or("", |x| x.as_str()) });
    let to = currency_code(&m[4]);
    if !is_code(&from) || !is_code(&to) {
        return None;
    }
    let data = rates.get()?;
    let (rf, rt) = (*data.rates.get(&from)?, *data.rates.get(&to)?);
    let amount: f64 = m[2].replace(',', "").parse().ok()?;
    if !amount.is_finite() || rf == 0.0 {
        return None;
    }
    let value = amount / rf * rt;
    let digits = if value >= 100.0 {
        2
    } else if value >= 1.0 {
        4
    } else {
        6
    };
    let result = format!("{} {to}", fmt_fixed(value, digits));
    let when = crate::format::date_time(data.fetched);
    Some(calc_item(q, &result, format!("{} {from} → {to} · rates from {when}", fmt_fixed(amount, 3))))
}

pub fn calculate(query: &str, currency_conversion: bool, rates: &Rates) -> Vec<ResultItem> {
    let raw = query.trim();
    let forced = raw.starts_with('=');
    let q = raw.trim_start_matches('=').trim();
    if q.chars().count() < 2 && !(forced && !q.is_empty()) {
        return Vec::new();
    }
    if currency_conversion && let Some(c) = currency(q, rates) {
        return vec![c];
    }
    if !looks_like_math(q) && !forced {
        return Vec::new();
    }
    let Some(result) = evaluate(q) else { return Vec::new() };
    if plain(&result) == q.chars().filter(|c| !c.is_whitespace()).collect::<String>() {
        return Vec::new();
    }
    vec![calc_item(q, &result, format!("{q} ="))]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> String {
        evaluate(s).unwrap_or_else(|| panic!("no result for {s}"))
    }

    #[test]
    fn arithmetic() {
        assert_eq!(ev("2^10"), "1,024");
        assert_eq!(ev("1 + 2 * 3"), "7");
        assert_eq!(ev("(1 + 2) * 3"), "9");
        assert_eq!(ev("-2^2"), "-4");
        assert_eq!(ev("2^3^2"), "512");
        assert_eq!(ev("0.1 + 0.2"), "0.3");
        assert_eq!(ev("1/3"), "0.333333333333");
        assert_eq!(ev("10 / 4"), "2.5");
        assert_eq!(ev("3x4"), "12");
        assert_eq!(ev("6 ÷ 3"), "2");
        assert_eq!(ev("1,000,000 * 2"), "2,000,000");
        assert_eq!(ev("5!"), "120");
        assert_eq!(ev("10 % 3"), "1");
        assert_eq!(ev("10 mod 4"), "2");
        assert_eq!(ev("2pi"), "6.28318530718");
        assert_eq!(ev("0xff"), "255");
        assert_eq!(ev("1e3"), "1,000");
        assert_eq!(ev("2 ** 8"), "256");
        assert!(evaluate("1/0").is_none());
        assert!(evaluate("2 +").is_none());
        assert!(evaluate("hello").is_none());
    }

    #[test]
    fn functions_and_constants() {
        assert_eq!(ev("sqrt(2) * pi"), "4.44288293816");
        assert_eq!(ev("sqrt(16)"), "4");
        assert_eq!(ev("abs(-3)"), "3");
        assert_eq!(ev("log10(1000)"), "3");
        assert_eq!(ev("ln(e)"), "1");
        assert_eq!(ev("round(2.567, 2)"), "2.57");
        assert_eq!(ev("max(3, 9, 4)"), "9");
        assert_eq!(ev("sin(90 deg)"), "1");
        assert_eq!(ev("cos(0)"), "1");
        assert_eq!(ev("hex(255)"), "0xff");
        assert_eq!(ev("255 to hex"), "0xff");
        assert_eq!(ev("10 in bin"), "0b1010");
    }

    #[test]
    fn percentages() {
        assert_eq!(ev("15% of 80"), "12");
        assert_eq!(ev("15% * 80"), "12");
        assert_eq!(ev("100 + 10%"), "110");
        assert_eq!(ev("200 - 25%"), "150");
        assert_eq!(ev("50%"), "0.5");
    }

    #[test]
    fn units() {
        assert_eq!(ev("5 km to mi"), "3.106855961 mi");
        assert_eq!(ev("30 c to f"), "86 °F");
        assert_eq!(ev("100 f to c"), "37.77777778 °C");
        assert_eq!(ev("0 c to k"), "273.15 K");
        assert_eq!(ev("5 GB to MB"), "5,000 MB");
        assert_eq!(ev("5 gb to mb"), "5,000 MB");
        assert_eq!(ev("1 GiB to MB"), "1,073.741824 MB");
        assert_eq!(ev("12 in to cm"), "30.48 cm");
        assert_eq!(ev("6 ft in m"), "1.8288 m");
        assert_eq!(ev("60 mph to km/h"), "96.56064 km/h");
        assert_eq!(ev("100 km / 2 h"), "50 km/h");
        assert_eq!(ev("1 l to ml"), "1,000 mL");
        assert_eq!(ev("2 lbs to kg"), "0.90718474 kg");
        assert_eq!(ev("90 deg to rad"), "1.570796327 rad");
        assert_eq!(ev("5 km + 300 m"), "5.3 km");
        assert_eq!(ev("1 day to h"), "24 h");
        assert!(evaluate("5 km to kg").is_none());
    }

    #[test]
    fn decides_what_is_math() {
        let rates = Rates::offline();
        assert!(calculate("2+2", true, &rates).len() == 1);
        assert!(calculate("1.2.3", true, &rates).is_empty());
        assert!(calculate("42", true, &rates).is_empty());
        assert!(calculate("firefox", true, &rates).is_empty());
        assert_eq!(calculate("= 42", true, &rates).len(), 0); // the answer would just echo the question
        assert_eq!(calculate("=2", true, &rates).len(), 0);
        let r = calculate("2^10", true, &rates);
        assert_eq!(r[0].title, "1,024");
        assert_eq!(r[0].action, Action::Copy("1024".into()));
        assert_eq!(r[0].subtitle.as_deref(), Some("2^10 ="));
    }

    #[test]
    fn converts_currency_with_cached_rates() {
        let rates = Rates::with_rates(&[("USD", 1.0), ("EUR", 0.5), ("IQD", 1310.0)]);
        let r = calculate("100 usd to eur", true, &rates);
        assert_eq!(r[0].title, "50 EUR");
        let r = calculate("$10 in iqd", true, &rates);
        assert_eq!(r[0].title, "13,100 IQD");
        assert!(calculate("100 usd to eur", false, &rates).is_empty());
    }
}
