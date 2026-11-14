//! Texts of the backend (tray, notices, errors) in the UI language. German and English are
//! written inline; the other languages come from the same catalogs as the frontend
//! (`src/lib/locales/*.json`, keyed by the German text), falling back to English.

use std::collections::HashMap;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

use crate::AppState;

pub const LANGS: &[&str] = &["de", "en", "it", "es", "fr", "nl", "pl", "pt", "vi"];

fn catalog() -> &'static HashMap<&'static str, HashMap<String, String>> {
    static CAT: OnceLock<HashMap<&'static str, HashMap<String, String>>> = OnceLock::new();
    CAT.get_or_init(|| {
        let raw: [(&str, &str); 7] = [
            ("it", include_str!("../../src/lib/locales/it.json")),
            ("es", include_str!("../../src/lib/locales/es.json")),
            ("fr", include_str!("../../src/lib/locales/fr.json")),
            ("nl", include_str!("../../src/lib/locales/nl.json")),
            ("pl", include_str!("../../src/lib/locales/pl.json")),
            ("pt", include_str!("../../src/lib/locales/pt.json")),
            ("vi", include_str!("../../src/lib/locales/vi.json")),
        ];
        raw.into_iter().map(|(l, json)| (l, serde_json::from_str(json).unwrap_or_default())).collect()
    })
}

/// Text in `lang`; `{0}`, `{1}` … are replaced by `args`.
pub fn tr_lang(lang: &str, de: &str, en: &str, args: &[&str]) -> String {
    let mut s = match lang {
        "de" => de.to_string(),
        "en" => en.to_string(),
        l => catalog().get(l).and_then(|m| m.get(de)).cloned().unwrap_or_else(|| en.to_string()),
    };
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), a);
    }
    s
}

pub fn lang(app: &AppHandle) -> String {
    app.state::<AppState>().settings.read().language.clone()
}

/// Text in the app's UI language.
pub fn tr(app: &AppHandle, de: &str, en: &str) -> String {
    tr_lang(&lang(app), de, en, &[])
}

pub fn trf(app: &AppHandle, de: &str, en: &str, args: &[&str]) -> String {
    tr_lang(&lang(app), de, en, args)
}

/// UI language for a first start: Windows' display language if supported, else English.
pub fn default_language() -> String {
    let sys = crate::platform::system_language();
    if LANGS.contains(&sys.as_str()) {
        sys
    } else {
        "en".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_english() {
        assert_eq!(tr_lang("de", "Öffnen", "Open", &[]), "Öffnen");
        assert_eq!(tr_lang("en", "Öffnen", "Open", &[]), "Open");
        assert_eq!(tr_lang("xx", "Öffnen", "Open", &[]), "Open");
        assert_eq!(tr_lang("en", "{0} belegt", "{0} taken", &["Win + V"]), "Win + V taken");
    }

    #[test]
    fn catalogs_parse() {
        for l in ["it", "es", "fr", "nl", "pl", "pt", "vi"] {
            assert!(catalog().contains_key(l));
        }
    }
}
