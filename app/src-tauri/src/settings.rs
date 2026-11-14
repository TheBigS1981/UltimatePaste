//! User settings, stored as JSON next to the database. Unknown/missing fields fall back
//! to defaults so older settings files keep working after updates.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    pub hotkey: String,
    pub sequential_hotkey: String,
    /// "fluent" | "board" | "drawer" | "compact"
    pub layout: String,
    /// Color/font design: "auto" (matches the layout) or a design name
    pub design: String,
    /// "center" | "cursor" (fluent and board layouts)
    pub position: String,
    pub autostart: bool,
    pub paused: bool,
    /// None = keep forever
    pub retention_days: Option<u32>,
    pub max_text_mb: u32,
    pub max_image_mb: u32,
    /// Enter pastes as plain text, Shift+Enter keeps formatting.
    pub prefer_plain_text: bool,
    /// "paste" | "copy"
    pub primary_action: String,
    pub move_used_to_top: bool,
    /// Lower-case executable names whose clipboard content is never recorded.
    pub blocked_apps: Vec<String>,
    pub ocr_enabled: bool,
    pub inbox_dir: Option<String>,
    pub inbox_tags: Vec<String>,
    pub sequential_timeout_secs: u32,
    pub sequential_newline: bool,
    pub ai: AiSettings,
    /// Logical window sizes chosen by the user, per layout ("fluent", …) and "settings".
    pub window_sizes: std::collections::BTreeMap<String, [f64; 2]>,
    /// In-window shortcut that opens the actions menu, e.g. "Ctrl+Enter".
    pub actions_key: String,
    /// Global shortcut for the emoji / GIF / symbol picker ("" = off).
    pub picker_hotkey: String,
    /// Global shortcut for translating the selection ("" = off).
    pub translate_hotkey: String,
    /// Optional shortcut that uses the other translation mode ("" = off).
    pub translate_alt_hotkey: String,
    /// Global shortcut for correcting / improving the selection ("" = off).
    pub correct_hotkey: String,
    /// Shortcut conflicts the user was already told about (see conflicts.rs).
    pub notified_conflicts: Vec<String>,
    /// How to paste into specific programs (Q31).
    pub app_rules: Vec<AppRule>,
    /// Remove 2FA codes, card numbers, keys … after a few minutes (Q30).
    pub sensitive: crate::sensitive::SensitiveSettings,
    /// Remove tracking parameters from copied links automatically (Q29).
    pub clean_links: bool,
    /// Replace typed snippet keywords like ";mfg" in any program (Q28).
    pub snippet_expand: bool,
    /// The welcome tour was finished or skipped (Q27).
    pub tour_done: bool,
    /// Look for a new version on GitHub at start and daily (Q49).
    pub update_check: bool,
    pub picker: PickerSettings,
    pub translate: TranslateSettings,
}

/// Paste rule for one program.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppRule {
    /// Exe name, e.g. "outlook.exe" (case-insensitive)
    pub app: String,
    /// "plain" | "formatted" | "oneline" (plain, line breaks removed) | "trim" (plain, trimmed)
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerSettings {
    /// Entries kept under "recently used" (per tab).
    pub recent_count: u32,
    /// 0 = default yellow, 1..=5 = Fitzpatrick skin tones
    pub skin_tone: u8,
    pub gif_enabled: bool,
    /// "giphy" | "klipy"
    pub gif_provider: String,
    /// "g" | "pg" | "pg-13" | "r"
    pub gif_rating: String,
    pub gif_original_size: bool,
    /// Tell the provider which GIF was sent (anonymous random id).
    pub gif_feedback: bool,
}

impl Default for PickerSettings {
    fn default() -> Self {
        Self {
            recent_count: 24,
            skin_tone: 0,
            gif_enabled: true,
            gif_provider: "giphy".into(),
            gif_rating: "pg".into(),
            gif_original_size: false,
            gif_feedback: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TranslateSettings {
    /// "window" | "direct"
    pub mode: String,
    /// "pair" (primary <-> secondary) | "fixed" (always to `fixed_target`)
    pub rule: String,
    /// DeepL-style language codes, e.g. "DE", "EN-US"
    pub primary: String,
    pub secondary: String,
    pub fixed_target: String,
    /// "deepl" | "ai"
    pub engine: String,
    /// AI provider id ("" = default provider) and model override ("" = provider model)
    pub ai_provider: String,
    pub ai_model: String,
    /// "auto" | "less" (informal) | "more" (formal)
    pub formality: String,
    /// USD per million tokens for models without built-in prices: model -> [input, output]
    pub custom_prices: std::collections::BTreeMap<String, [f64; 2]>,
    /// Service for correcting / improving text: "deepl" | "ai"
    pub correct_engine: String,
    /// Default style when improving: "" (none), "simple", "business", "academic", "casual",
    /// or a tone: "tone:enthusiastic", "tone:friendly", "tone:confident", "tone:diplomatic"
    pub improve_style: String,
    /// Re-run automatically one second after typing in the window.
    pub live: bool,
}

impl Default for TranslateSettings {
    fn default() -> Self {
        Self {
            mode: "window".into(),
            rule: "pair".into(),
            primary: "DE".into(),
            secondary: "EN-US".into(),
            fixed_target: "EN-US".into(),
            engine: "deepl".into(),
            ai_provider: String::new(),
            ai_model: "claude-sonnet-5-5".into(),
            formality: "auto".into(),
            custom_prices: Default::default(),
            correct_engine: "deepl".into(),
            improve_style: String::new(),
            live: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    pub enabled: bool,
    pub providers: Vec<AiProvider>,
    pub default_provider: String,
    pub actions: Vec<AiAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    /// "anthropic" | "openai"
    pub kind: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiAction {
    pub id: String,
    pub name: String,
    /// Instruction for the model. The clipboard content is sent as the user message.
    pub prompt: String,
    /// Empty = default provider
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub builtin: bool,
}

pub const DEFAULT_BLOCKED_APPS: &[&str] = &[
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
    "lastpass.exe",
    "dashlane.exe",
    "enpass.exe",
    "nordpass.exe",
    "proton pass.exe",
    "keeper.exe",
    "credentialuibroker.exe",
];

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "de".into(),
            theme: "system".into(),
            hotkey: "Ctrl+Shift+V".into(),
            sequential_hotkey: "Ctrl+Shift+Alt+V".into(),
            layout: "fluent".into(),
            design: "auto".into(),
            position: "center".into(),
            autostart: true,
            paused: false,
            retention_days: Some(90),
            max_text_mb: 5,
            max_image_mb: 50,
            prefer_plain_text: false,
            primary_action: "paste".into(),
            move_used_to_top: true,
            blocked_apps: DEFAULT_BLOCKED_APPS.iter().map(|s| s.to_string()).collect(),
            ocr_enabled: true,
            inbox_dir: None,
            inbox_tags: vec!["clipboard".into()],
            sequential_timeout_secs: 60,
            sequential_newline: false,
            ai: AiSettings::default(),
            window_sizes: Default::default(),
            actions_key: "Ctrl+Enter".into(),
            picker_hotkey: "Ctrl+Period".into(),
            translate_hotkey: "Ctrl+Alt+T".into(),
            translate_alt_hotkey: String::new(),
            correct_hotkey: "Ctrl+Alt+K".into(),
            notified_conflicts: Vec::new(),
            app_rules: Vec::new(),
            sensitive: Default::default(),
            clean_links: false,
            snippet_expand: false,
            tour_done: false,
            update_check: true,
            picker: PickerSettings::default(),
            translate: TranslateSettings::default(),
        }
    }
}

impl Default for AiSettings {
    fn default() -> Self {
        let action = |id: &str, name: &str, prompt: &str| AiAction {
            id: id.into(),
            name: name.into(),
            prompt: prompt.into(),
            provider: String::new(),
            builtin: true,
        };
        Self {
            enabled: true,
            providers: vec![
                AiProvider {
                    id: "claude".into(),
                    name: "Claude".into(),
                    kind: "anthropic".into(),
                    base_url: "https://api.anthropic.com".into(),
                    model: "claude-opus-5-5".into(),
                },
                AiProvider {
                    id: "ollama".into(),
                    name: "Ollama (lokal)".into(),
                    kind: "openai".into(),
                    base_url: "http://localhost:11434/v1".into(),
                    model: "llama3.2".into(),
                },
            ],
            default_provider: "claude".into(),
            actions: vec![
                action("fix", "Korrigieren", "Fix spelling, grammar and punctuation. Keep the language, meaning, tone and formatting. Change nothing else."),
                action("formal", "Förmlicher formulieren", "Rewrite the text in a more formal, professional tone. Keep the language and meaning."),
                action("casual", "Lockerer formulieren", "Rewrite the text in a more casual, friendly tone. Keep the language and meaning."),
                action("shorten", "Kürzen", "Make the text shorter and clearer while keeping all essential information. Keep the language."),
                action("summarize", "Zusammenfassen", "Summarize the text in a few concise bullet points in the same language."),
                action("reply", "Antwort entwerfen", "The text is a message I received. Draft a polite, concise reply in the same language. Output only the reply."),
            ],
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Settings {
        let path = dir.join("settings.json");
        match std::fs::read(&path) {
            Ok(bytes) => {
                let mut s: Settings = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                    log::warn!("settings.json invalid ({e}), using defaults");
                    Settings::default()
                });
                // 0.5.1–0.6.1 had a separate "winV" switch; now the hotkey itself says it.
                let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                if raw.get("winV").and_then(|v| v.as_bool()) == Some(true) {
                    s.hotkey = "Super+V".into();
                }
                // 0.8: translation has its own feature; drop the old built-in AI translate actions.
                s.ai.actions.retain(|a| !(a.builtin && a.id.starts_with("translate-")));
                s
            }
            Err(_) => Settings { language: crate::i18n::default_language(), ..Settings::default() },
        }
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let tmp = dir.join("settings.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, dir.join("settings.json"))?;
        Ok(())
    }

    pub fn is_blocked(&self, exe: Option<&str>) -> bool {
        exe.is_some_and(|e| {
            let e = e.to_lowercase();
            self.blocked_apps.iter().any(|b| b.eq_ignore_ascii_case(&e))
        })
    }

    pub fn retention_cutoff(&self, now_ms: i64) -> Option<i64> {
        self.retention_days.map(|d| now_ms - d as i64 * 86_400_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_uses_defaults() {
        let s: Settings = serde_json::from_str(r#"{"hotkey":"Alt+V"}"#).unwrap();
        assert_eq!(s.hotkey, "Alt+V");
        assert_eq!(s.retention_days, Some(90));
        assert!(!s.ai.actions.is_empty());
    }

    #[test]
    fn blocked_is_case_insensitive() {
        let s = Settings::default();
        assert!(s.is_blocked(Some("KeePassXC.exe")));
        assert!(!s.is_blocked(Some("notepad.exe")));
        assert!(!s.is_blocked(None));
    }

    #[test]
    fn unlimited_retention() {
        let s = Settings { retention_days: None, ..Default::default() };
        assert_eq!(s.retention_cutoff(1000), None);
    }
}
