//! Snippet placeholder expansion.
//!
//! Supported: `{date}`, `{time}`, `{datetime}`, `{date:<chrono format>}`, `{clipboard}`, `{cursor}`,
//! input fields `{eingabe:Name}` / `{eingabe:Name|Default}` (also `{input:…}`) and choices
//! `{auswahl:Name|A|B}` (also `{select:…}`). `{{` and `}}` produce literal braces.

use chrono::{DateTime, Local};
use serde::Serialize;
use std::collections::HashMap;

/// A field the user fills in before the snippet is pasted.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub default: String,
    /// Non-empty for a choice
    pub options: Vec<String>,
}

/// Parses `eingabe:Name|Default` / `auswahl:Name|A|B` (and the English forms).
fn parse_field(token: &str) -> Option<Field> {
    let (kind, rest) = token.split_once(':')?;
    let choice = match kind.trim().to_lowercase().as_str() {
        "eingabe" | "input" => false,
        "auswahl" | "select" => true,
        _ => return None,
    };
    let mut parts = rest.split('|').map(str::trim);
    let name = parts.next()?.to_string();
    if name.is_empty() {
        return None;
    }
    let rest: Vec<String> = parts.map(str::to_string).collect();
    Some(if choice {
        Field { name, default: rest.first().cloned().unwrap_or_default(), options: rest }
    } else {
        Field { name, default: rest.first().cloned().unwrap_or_default(), options: Vec::new() }
    })
}

/// Input fields of a snippet in order of appearance, each name once.
pub fn fields(body: &str) -> Vec<Field> {
    let mut out: Vec<Field> = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        if let Some(skipped) = after.strip_prefix('{') {
            rest = skipped;
            continue;
        }
        let Some(end) = after.find('}') else { break };
        if let Some(f) = parse_field(&after[..end]) {
            if !out.iter().any(|x| x.name == f.name) {
                out.push(f);
            }
        }
        rest = &after[end + 1..];
    }
    out
}

pub struct Expanded {
    pub text: String,
    /// How many characters the caret must move left after pasting (for `{cursor}`).
    pub cursor_back: Option<usize>,
}

pub fn expand(body: &str, now: DateTime<Local>, lang: &str, clipboard: Option<&str>) -> Expanded {
    expand_with(body, now, lang, clipboard, &HashMap::new())
}

/// Like `expand`, with the values the user entered for the input fields.
pub fn expand_with(body: &str, now: DateTime<Local>, lang: &str, clipboard: Option<&str>, values: &HashMap<String, String>) -> Expanded {
    let date_fmt = match lang {
        "en" => "%Y-%m-%d",
        "de" | "pl" => "%d.%m.%Y",
        "nl" => "%d-%m-%Y",
        _ => "%d/%m/%Y",
    };
    let time_fmt = "%H:%M";
    let mut out = String::with_capacity(body.len());
    let mut cursor_at: Option<usize> = None; // char index in out
    let mut chars = body.char_indices().peekable();

    while let Some((i, ch)) = chars.next() {
        match ch {
            '{' if chars.peek().map(|p| p.1) == Some('{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek().map(|p| p.1) == Some('}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let rest = &body[i + 1..];
                let Some(end) = rest.find('}') else {
                    out.push('{');
                    continue;
                };
                let token = &rest[..end];
                let replacement = match token {
                    "date" => Some(now.format(date_fmt).to_string()),
                    "time" => Some(now.format(time_fmt).to_string()),
                    "datetime" => Some(format!("{} {}", now.format(date_fmt), now.format(time_fmt))),
                    "clipboard" => Some(clipboard.unwrap_or("").to_string()),
                    "cursor" => {
                        if cursor_at.is_none() {
                            cursor_at = Some(out.chars().count());
                        }
                        Some(String::new())
                    }
                    t if parse_field(t).is_some() => {
                        let f = parse_field(t).expect("checked");
                        Some(values.get(&f.name).cloned().unwrap_or(f.default))
                    }
                    t if t.starts_with("date:") => {
                        let f = &t[5..];
                        // chrono panics on invalid format strings when displayed; validate first.
                        let items: Vec<_> = chrono::format::StrftimeItems::new(f).collect();
                        if items.iter().any(|it| matches!(it, chrono::format::Item::Error)) {
                            None
                        } else {
                            Some(now.format_with_items(items.into_iter()).to_string())
                        }
                    }
                    _ => None,
                };
                match replacement {
                    Some(r) => {
                        out.push_str(&r);
                        // skip the token and closing brace
                        let target = i + 1 + end + 1;
                        while chars.peek().is_some_and(|(j, _)| *j < target) {
                            chars.next();
                        }
                    }
                    None => out.push('{'),
                }
            }
            _ => out.push(ch),
        }
    }

    // Windows editors treat "\r\n" as one caret step; count chars without '\r'.
    let cursor_back = cursor_at.map(|pos| out.chars().skip(pos).filter(|c| *c != '\r').count()).filter(|n| *n > 0);
    Expanded { text: out, cursor_back }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 11, 13, 9, 5, 0).unwrap()
    }

    #[test]
    fn dates_by_language() {
        assert_eq!(expand("{date}", t(), "de", None).text, "13.11.2026");
        assert_eq!(expand("{date}", t(), "en", None).text, "2026-11-13");
        assert_eq!(expand("{datetime}", t(), "de", None).text, "13.11.2026 09:05");
        assert_eq!(expand("KW {date:%V}", t(), "de", None).text, "KW 46");
    }

    #[test]
    fn clipboard_and_cursor() {
        let e = expand("Hallo {clipboard},\n{cursor}\nGruß", t(), "de", Some("Anna"));
        assert_eq!(e.text, "Hallo Anna,\n\nGruß");
        assert_eq!(e.cursor_back, Some(5)); // "\nGruß"
    }

    #[test]
    fn unknown_and_escapes_are_literal() {
        assert_eq!(expand("{nope} {{x}} {", t(), "de", None).text, "{nope} {x} {");
        assert_eq!(expand("{date:%Q}", t(), "de", None).text, "{date:%Q}");
    }

    #[test]
    fn input_fields() {
        let body = "Hallo {auswahl:Anrede|Herr|Frau} {eingabe:Name}, {eingabe:Name} – {input:Ort|Stuttgart} {{eingabe:x}}";
        let f = fields(body);
        assert_eq!(f.len(), 3);
        assert_eq!(f[0], Field { name: "Anrede".into(), default: "Herr".into(), options: vec!["Herr".into(), "Frau".into()] });
        assert_eq!(f[1].name, "Name");
        assert_eq!(f[2].default, "Stuttgart");
        let mut v = HashMap::new();
        v.insert("Anrede".to_string(), "Frau".to_string());
        v.insert("Name".to_string(), "Meier".to_string());
        assert_eq!(expand_with(body, t(), "de", None, &v).text, "Hallo Frau Meier, Meier – Stuttgart {eingabe:x}");
    }

    #[test]
    fn cursor_at_end_means_no_move() {
        assert_eq!(expand("abc{cursor}", t(), "de", None).cursor_back, None);
    }
}
