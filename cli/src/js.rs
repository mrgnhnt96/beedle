//! The JavaScript behaviour the audit and the review depend on: regex meaning, whitespace,
//! UTF-16 lengths, truthiness and path joining.

use std::collections::HashMap;
use std::env;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use regex::{Regex, RegexBuilder};
use serde_json::Value;

const SPACE: &str =
    r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";

fn translate(pattern: &str) -> String {
    let source = pattern.replace(r"[\s\S]", "(?s:.)");
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('b') => out.push_str(r"(?-u:\b)"),
            Some('s') => out.push_str(&format!("[{SPACE}]")),
            Some('S') => out.push_str(&format!("[^{SPACE}]")),
            Some('d') => out.push_str("[0-9]"),
            Some('W') => out.push_str("[^0-9A-Za-z_]"),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Compiles a pattern written for JavaScript, with its `i` and `m` flags, and keeps it for reuse.
pub fn re(pattern: &str, flags: &str) -> Regex {
    static CACHE: LazyLock<Mutex<HashMap<(String, String), Regex>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut cache = CACHE.lock().unwrap();
    cache
        .entry((pattern.to_string(), flags.to_string()))
        .or_insert_with(|| {
            RegexBuilder::new(&translate(pattern))
                .case_insensitive(flags.contains('i'))
                .multi_line(flags.contains('m'))
                .crlf(flags.contains('m'))
                .build()
                .unwrap_or_else(|error| panic!("bad pattern {pattern}: {error}"))
        })
        .clone()
}

pub fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\x0B'
            | '\x0C'
            | '\r'
            | ' '
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

pub fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

pub fn trim_end(text: &str) -> &str {
    text.trim_end_matches(is_space)
}

pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

pub fn utf16_slice(text: &str, from: usize, to: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let to = to.min(units.len());
    String::from_utf16_lossy(&units[from.min(to)..to])
}

pub fn round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// `toFixed(2)`: an exact tie rounds away from zero, where `{:.2}` rounds it to even.
pub fn to_fixed_2(value: f64) -> String {
    let eighths = value * 8.0;
    let is_tie = eighths.fract() == 0.0 && eighths % 2.0 != 0.0;
    let nudge = if is_tie { value.signum() / 1024.0 } else { 0.0 };
    format!("{:.2}", value + nudge)
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Writes a whole float as an integer, the way JSON.stringify does.
pub fn number(value: &Value) -> Value {
    match value.as_f64() {
        Some(n) if value.is_f64() && n.fract() == 0.0 && n.abs() < 9e15 => Value::from(n as i64),
        _ => value.clone(),
    }
}

pub fn resolve(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

pub fn join(dir: &Path, rest: &str) -> PathBuf {
    let mut out = dir.to_path_buf();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            name => out.push(name),
        }
    }
    if rest.ends_with('/') {
        out.push("");
    }
    out
}

/// None where decodeURIComponent would throw.
pub fn decode_uri_component(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'%' {
            out.push(bytes[at]);
            at += 1;
            continue;
        }
        let high = (*bytes.get(at + 1)? as char).to_digit(16)?;
        let low = (*bytes.get(at + 2)? as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
        at += 3;
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn two_places_round_ties_as_to_fixed_does() {
        let cases = [
            (0.125, "0.13"),
            (0.375, "0.38"),
            (0.625, "0.63"),
            (1.125, "1.13"),
            (-0.125, "-0.13"),
            (1.005, "1.00"),
            (0.25, "0.25"),
            (0.0, "0.00"),
            (1.5, "1.50"),
        ];
        for (value, expected) in cases {
            assert_eq!(to_fixed_2(value), expected, "{value}");
        }
    }

    #[test]
    fn word_boundaries_are_ascii_as_in_javascript() {
        assert!(re(r"\bTODO\b", "").is_match("é TODO é"));
        assert!(re(r"\bTODO\b", "").is_match("éTODOé"));
    }

    #[test]
    fn whitespace_classes_match_the_javascript_set() {
        assert!(re(r"^\s+$", "").is_match("\u{FEFF}\u{A0}\u{2003}"));
        assert!(!re(r"\s", "").is_match("\u{85}"));
        assert!(re(r"[^\s>]+", "").is_match("a"));
    }

    #[test]
    fn multi_line_anchors_also_stop_at_a_carriage_return() {
        assert!(re(r"^sitemap:", "im").is_match("a\r\nSitemap: x"));
    }

    #[test]
    fn slices_count_utf16_units() {
        assert_eq!(utf16_len("a😀"), 3);
        assert_eq!(utf16_slice("a😀b", 3, 9), "b");
        assert_eq!(utf16_slice("abc", 5, 9), "");
    }

    #[test]
    fn only_a_whole_float_loses_its_point() {
        assert_eq!(number(&json!(2.0)).to_string(), "2");
        assert_eq!(number(&json!(1.5)).to_string(), "1.5");
    }

    #[test]
    fn bad_escapes_are_refused() {
        assert_eq!(decode_uri_component("a%20b").as_deref(), Some("a b"));
        assert_eq!(decode_uri_component("100%"), None);
        assert_eq!(decode_uri_component("%ff"), None);
    }

    #[test]
    fn join_stays_literal_about_the_trailing_slash() {
        assert_eq!(join(Path::new("/a"), "/b/../c.png"), Path::new("/a/c.png"));
        assert!(join(Path::new("/a"), "b/").to_string_lossy().ends_with('/'));
    }
}
