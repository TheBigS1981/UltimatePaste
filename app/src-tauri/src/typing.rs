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
    std::thread::sleep(Duration::from_millis(30));
    platform::send_backspaces(kw.chars().count());
    log::info!("typed snippet keyword expanded");

    if !snippets::fields(&snippet.body).is_empty() {
        // Ask for the values in the paste window, then paste into this program.
        st.prev_foreground.store(fg, std::sync::atomic::Ordering::Release);
        *st.snippet_form.lock() = Some(snippet.id);
        let h = app.clone();
        let _ = app.run_on_main_thread(move || actions::show_palette_for_form(&h));
        return;
    }
    let lang = st.settings.read().language.clone();
    let ex = snippets::expand_with(&snippet.body, chrono::Local::now(), &lang, None, &HashMap::new());
    let limits = platform::Limits { max_text_bytes: 5 << 20, max_image_bytes: 50 << 20 };
    let before = platform::read_clipboard(limits)
        .map(|c| ClipContent { private: c.excluded, ..c.content })
        .unwrap_or_default();
    if actions::write_silently(app, &ClipContent::text_only(ex.text)).is_err() {
        return;
    }
    std::thread::sleep(Duration::from_millis(30));
    platform::send_paste();
    if let Some(n) = ex.cursor_back {
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
}
