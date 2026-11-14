//! Recognizes short-lived secrets in copied text (2FA codes, card numbers, API keys / tokens,
//! IBANs) so they can be removed from the history after a few minutes (Q30).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::model::now_ms;
use crate::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Otp,
    Card,
    Key,
    Iban,
}

fn digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn luhn(num: &str) -> bool {
    let mut sum = 0;
    for (i, c) in num.chars().rev().enumerate() {
        let mut d = c.to_digit(10).unwrap_or(0);
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}

fn iban_valid(s: &str) -> bool {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    // Byte slicing below needs ASCII (an umlaut at position 2 would otherwise crash the app).
    if !s.is_ascii() || !(15..=34).contains(&s.len()) || !s[..2].chars().all(|c| c.is_ascii_alphabetic()) || !s[2..4].chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let rearranged = format!("{}{}", &s[4..], &s[..4]);
    let mut rem: u32 = 0;
    for c in rearranged.chars() {
        let v = if c.is_ascii_digit() { c.to_digit(10).unwrap() } else { c as u32 - 'A' as u32 + 10 };
        for d in v.to_string().chars() {
            rem = (rem * 10 + d.to_digit(10).unwrap()) % 97;
        }
    }
    rem == 1
}

fn looks_like_key(t: &str) -> bool {
    const PREFIXES: &[&str] = &["sk-", "sk_live_", "sk_test_", "rk_live_", "ghp_", "gho_", "ghs_", "github_pat_", "glpat-", "xoxb-", "xoxp-", "AKIA", "AIza", "eyJ", "ya29."];
    if t.len() < 20 || t.chars().any(char::is_whitespace) {
        return false;
    }
    if PREFIXES.iter().any(|p| t.starts_with(p)) {
        return true;
    }
    // Long random-looking strings: letters and digits mixed, few repeats, no word-like shape.
    let allowed = t.chars().all(|c| c.is_ascii_alphanumeric() || "-_.+/=:".contains(c));
    let upper = t.chars().any(|c| c.is_ascii_uppercase());
    let lower = t.chars().any(|c| c.is_ascii_lowercase());
    let digit = t.chars().filter(|c| c.is_ascii_digit()).count();
    allowed && t.len() >= 32 && upper && lower && digit >= 4 && !t.contains("://") && !t.contains('.')
}

/// What kind of secret `text` is, if any. Only short, single-item texts qualify.
pub fn classify(text: &str) -> Option<Kind> {
    let t = text.trim();
    if t.is_empty() || t.len() > 200 || t.lines().count() > 1 {
        return None;
    }
    let d = digits(t);
    let only_digits_and_spaces = t.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '-');
    // 6–8 digits: shorter numbers are too often years, amounts or PINs of no concern.
    if only_digits_and_spaces && (6..=8).contains(&d.len()) && t.len() <= 9 {
        return Some(Kind::Otp);
    }
    if only_digits_and_spaces && (13..=19).contains(&d.len()) && luhn(&d) {
        return Some(Kind::Card);
    }
    if iban_valid(t) {
        return Some(Kind::Iban);
    }
    if looks_like_key(t) {
        return Some(Kind::Key);
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SensitiveSettings {
    pub otp: bool,
    pub cards: bool,
    pub keys: bool,
    pub iban: bool,
    /// Minutes until such entries are removed
    pub minutes: u32,
}

impl Default for SensitiveSettings {
    fn default() -> Self {
        Self { otp: true, cards: true, keys: true, iban: false, minutes: 5 }
    }
}

impl SensitiveSettings {
    pub fn wants(&self, k: Kind) -> bool {
        match k {
            Kind::Otp => self.otp,
            Kind::Card => self.cards,
            Kind::Key => self.keys,
            Kind::Iban => self.iban,
        }
    }
}

fn path(app: &AppHandle) -> std::path::PathBuf {
    app.state::<AppState>().data_dir.join("expiring.json")
}

static LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

fn load(app: &AppHandle) -> Vec<(i64, i64)> {
    std::fs::read(path(app)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(app: &AppHandle, list: &[(i64, i64)]) {
    if let Ok(b) = serde_json::to_vec(list) {
        let _ = std::fs::write(path(app), b);
    }
}

/// Called for every newly recorded text entry.
pub fn on_new_entry(app: &AppHandle, id: i64, text: &str) {
    let cfg = app.state::<AppState>().settings.read().sensitive.clone();
    let Some(kind) = classify(text) else { return };
    if !cfg.wants(kind) {
        return;
    }
    let _g = LOCK.lock();
    let mut list = load(app);
    list.retain(|(i, _)| *i != id);
    list.push((id, now_ms() + cfg.minutes.clamp(1, 1440) as i64 * 60_000));
    save(app, &list);
    log::info!("entry {id} marked to expire ({kind:?})");
}

/// Removes due entries (never pinned ones). Runs every 15 s.
pub fn spawn_sweeper(app: AppHandle) {
    std::thread::Builder::new()
        .name("expire".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(15));
            let due: Vec<i64> = {
                let _g = LOCK.lock();
                let mut list = load(&app);
                if list.is_empty() {
                    continue;
                }
                let now = now_ms();
                let due = list.iter().filter(|(_, t)| *t <= now).map(|(i, _)| *i).collect::<Vec<_>>();
                list.retain(|(_, t)| *t > now);
                if !due.is_empty() {
                    save(&app, &list);
                }
                due
            };
            if due.is_empty() {
                continue;
            }
            let st = app.state::<AppState>();
            let mut removed = 0;
            for id in due {
                let pinned = st.index.read().get(id).is_some_and(|s| s.pinned);
                if pinned {
                    continue;
                }
                if st.store.lock().delete(id).is_ok() {
                    st.index.write().remove(id);
                    removed += 1;
                }
            }
            if removed > 0 {
                log::info!("removed {removed} sensitive entries");
                let _ = app.emit("history-changed", ());
            }
        })
        .expect("spawn expire");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_secrets() {
        assert_eq!(classify("123456"), Some(Kind::Otp));
        assert_eq!(classify("123 456"), Some(Kind::Otp));
        assert_eq!(classify("4111 1111 1111 1111"), Some(Kind::Card));
        assert_eq!(classify("4111 1111 1111 1112"), None);
        assert_eq!(classify("DE89 3704 0044 0532 0130 00"), Some(Kind::Iban));
        assert_eq!(classify("DE89 3704 0044 0532 0130 01"), None);
        assert_eq!(classify("sk-ant-api03-abcdefghijklmnopqrstuvwxyz"), Some(Kind::Key));
        assert_eq!(classify("ghp_16C7e42F292c6912E7710c838347Ae178B4a"), Some(Kind::Key));
        assert_eq!(classify("Xk9fQ2mZ7pL4wR8tV1bN6cH3jD5sG0aYeU"), Some(Kind::Key));
    }

    #[test]
    fn non_ascii_text_does_not_crash() {
        for t in ["Für morgen um 10 Uhr", "Möchtest du heute kommen?", "Der Ärger ist groß", "Désolé pour le retard", "ÄÖÜäöüß€€€€€€€€€€€€€€", "日本語のテキストです日本語のテキスト"] {
            assert_eq!(classify(t), None, "{t}");
        }
    }

    #[test]
    fn ignores_normal_text() {
        assert_eq!(classify("Hallo Anna"), None);
        assert_eq!(classify("2026"), None);
        assert_eq!(classify("https://example.com/AbCdEfGh12345678IjKlMnOp"), None);
        assert_eq!(classify("12345678901"), None);
        assert_eq!(classify("Termin am 14.11.2026 um 14 Uhr"), None);
        assert_eq!(classify("ThisIsAVeryLongCamelCaseIdentifierName"), None);
    }
}
