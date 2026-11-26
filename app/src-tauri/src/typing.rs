//! Snippets while typing (Q28): typing a keyword like ";mfg" anywhere replaces it with the
//! snippet text. Off by default; the keyboard hook only keeps the last few characters in memory.

use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::model::ClipContent;
use crate::{actions, platform, snippets, AppState};

/// Trigger form of a keyword: "mfg" → ";mfg"; a keyword that already starts with a
/// punctuation character is used as it is.
pub fn trigger(keyword: &str) -> Option<String> {
    let k = keyword.trim().to_lowercase();
    if k.is_empty() {
        return None;
    }
    Some(if k.chars().next().is_some_and(|c| c.is_alphanumeric()) { format!(";{k}") } else { k })
}

/// Re-reads settings and snippets and (de)activates expansion in the hook.
pub fn refresh(app: &AppHandle) {
    let st = app.state::<AppState>();
    let enabled = st.settings.read().snippet_expand;
    let keywords: Vec<String> = if enabled {
        st.store.lock().list_snippets().map(|l| l.iter().filter_map(|s| trigger(&s.keyword)).collect()).unwrap_or_default()
    } else {
        Vec::new()
    };
    if enabled && !keywords.is_empty() {
        let h = app.clone();
        let _ = app.run_on_main_thread(move || crate::fill::prewarm(&h));
    }
    let h = app.clone();
    platform::set_typing(enabled && !keywords.is_empty(), keywords, move |kw, fg| {
        let h = h.clone();
        std::thread::spawn(move || expand(&h, &kw, fg));
    });
}

fn expand(app: &AppHandle, kw: &str, fg: isize) {
    let st = app.state::<AppState>();
    let exe = platform::exe_of_window(fg);
    if st.settings.read().is_blocked(exe.as_deref()) || platform::is_window_elevated_for_us(fg) || platform::focused_is_password(fg) {
        return;
    }
    // Our own windows (palette search, settings) never expand.
    if exe.as_deref().is_some_and(|e| e.to_lowercase().starts_with("ultimatepaste")) {
        return;
    }
    let Ok(list) = st.store.lock().list_snippets() else { return };
    let Some(snippet) = list.into_iter().find(|s| trigger(&s.keyword).as_deref() == Some(kw)) else { return };
    if !snippets::fields(&snippet.body).is_empty() {
        // Ask for the values in a small popup at the caret; the keyword stays until then.
        log::info!("typed snippet keyword: asking for fields");
        let (h, body, n) = (app.clone(), snippet.body.clone(), kw.chars().count());
        let _ = app.run_on_main_thread(move || crate::fill::open(&h, body, fg, n));
        return;
    }
    std::thread::sleep(Duration::from_millis(30));
    if platform::foreground_window() != fg {
        log::info!("typed snippet keyword: another window came to the front, nothing replaced");
        return;
    }
    let lang = st.settings.read().language.clone();
    let ex = snippets::expand_with(&snippet.body, chrono::Local::now(), &lang, None, &HashMap::new());
    replace_keyword(app, fg, kw.chars().count(), &ex.text, ex.cursor_back);
    log::info!("typed snippet keyword expanded ({})", exe.as_deref().unwrap_or("?"));
}

/// Terminals (often with a remote shell or a TUI behind them) get their input in chunks: a
/// backspace that arrives in the same chunk as the pasted text can get lost, leaving the
/// first character of the keyword (";") behind. They get slower backspaces and a pause.
pub fn is_terminal(exe: Option<&str>) -> bool {
    const TERMINALS: [&str; 16] = [
        "windowsterminal", "openconsole", "conhost", "cmd", "powershell", "pwsh", "wezterm-gui", "alacritty", "mintty",
        "putty", "kitty", "tabby", "hyper", "warp", "termius", "mobaxterm",
    ];
    let Some(exe) = exe else { return false };
    let name = exe.to_lowercase();
    let name = name.trim_end_matches(".exe");
    TERMINALS.contains(&name)
}

/// Deletes the typed keyword (`keyword_chars` characters) in window `target` and pastes `text`
/// in its place. Stops if `target` is no longer in front.
pub fn replace_keyword(app: &AppHandle, target: isize, keyword_chars: usize, text: &str, cursor_back: Option<usize>) {
    let terminal = is_terminal(platform::exe_of_window(target).as_deref());
    let (gap, settle) = if terminal { (25, 150) } else { (12, 20) };
    platform::send_backspaces(keyword_chars, gap);
    // Let the program process the last backspace before the paste follows.
    std::thread::sleep(Duration::from_millis(settle));
    paste_keeping_clipboard(app, target, text, cursor_back);
}

/// Pastes `text` via the clipboard into window `target` (if it is still in front), then puts
/// the previous clipboard content back.
pub fn paste_keeping_clipboard(app: &AppHandle, target: isize, text: &str, cursor_back: Option<usize>) {
    let limits = platform::Limits { max_text_bytes: 5 << 20, max_image_bytes: 50 << 20 };
    let before = platform::read_clipboard(limits)
        .map(|c| ClipContent { private: c.excluded, ..c.content })
        .unwrap_or_default();
    if actions::write_silently(app, &ClipContent::text_only(text.to_string())).is_err() {
        return;
    }
    std::thread::sleep(Duration::from_millis(30));
    if platform::foreground_window() != target {
        log::info!("snippet paste: target {target:#x} no longer in front ({:#x}), nothing pasted", platform::foreground_window());
        if !before.is_empty() {
            let _ = actions::write_silently(app, &before);
        }
        return;
    }
    platform::send_paste();
    if let Some(n) = cursor_back {
        std::thread::sleep(Duration::from_millis(60));
        platform::send_left(n);
    }
    // Give the program time to read the clipboard, then put the previous content back.
    std::thread::sleep(Duration::from_millis(400));
    if !before.is_empty() {
        let _ = actions::write_silently(app, &before);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers() {
        assert_eq!(trigger("mfg").as_deref(), Some(";mfg"));
        assert_eq!(trigger("MfG").as_deref(), Some(";mfg"));
        assert_eq!(trigger("//sig").as_deref(), Some("//sig"));
        assert_eq!(trigger("  "), None);
    }

    #[test]
    fn terminals() {
        assert!(is_terminal(Some("WindowsTerminal.exe")));
        assert!(is_terminal(Some("wezterm-gui.exe")));
        assert!(!is_terminal(Some("slack.exe")));
        assert!(!is_terminal(None));
    }
}
