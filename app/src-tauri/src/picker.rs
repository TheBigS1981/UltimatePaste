//! Emoji / GIF / symbol picker: the popup window and its small persistent state
//! (recently used, favorites, anonymous ids for the GIF services).

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

use crate::{actions, platform, AppState};

pub const LABEL: &str = "picker";
const SIZE: (f64, f64) = (452.0, 500.0);

/// A GIF or sticker the user sent: only the provider and its id are stored
/// (the services forbid storing media or media URLs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GifRef {
    pub provider: String,
    pub id: String,
    #[serde(default)]
    pub sticker: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerState {
    /// Emoji as sent (including the chosen skin tone), newest first.
    pub emoji_recent: Vec<String>,
    pub symbol_recent: Vec<String>,
    pub gif_recent: Vec<GifRef>,
    /// Pinned emoji and symbols.
    pub favorites: Vec<String>,
    /// How often each emoji/symbol was sent: favorites and recently used are shown most used first.
    pub uses: std::collections::BTreeMap<String, u32>,
    /// Random install id, sent as `customer_id` / `random_id` (no personal data).
    pub customer_id: String,
    /// GIPHY's own anonymous id for this install (from /v1/randomid).
    pub giphy_random_id: String,
}

fn path(dir: &Path) -> std::path::PathBuf {
    dir.join("picker.json")
}

impl PickerState {
    pub fn load(dir: &Path) -> Self {
        let mut s: PickerState = std::fs::read(path(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut dirty = false;
        if s.customer_id.is_empty() {
            s.customer_id = random_id();
            dirty = true;
        }
        if s.uses.is_empty() {
            // first start with usage counts: take them over from the statistics
            let st = crate::stats::Stats::load(dir);
            for (k, n) in st.emoji.into_iter().chain(st.symbols) {
                s.uses.insert(k, n.min(u32::MAX as u64) as u32);
            }
            dirty |= !s.uses.is_empty();
        }
        if dirty {
            let _ = s.save(dir);
        }
        s
    }

    pub fn save(&self, dir: &Path) -> anyhow::Result<()> {
        let tmp = path(dir).with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path(dir))?;
        Ok(())
    }
}

pub fn random_id() -> String {
    let bytes: [u8; 16] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Moves `item` to the front of `list` and caps the length.
pub fn push_recent<T: PartialEq>(list: &mut Vec<T>, item: T, max: usize) {
    list.retain(|x| *x != item);
    list.insert(0, item);
    list.truncate(max.max(1));
}

/// Counts one use of an emoji/symbol.
pub fn count_use(state: &mut PickerState, item: &str) {
    let n = state.uses.entry(item.to_string()).or_default();
    *n = n.saturating_add(1);
}

/// Opens the picker at the text caret, or closes it when it is already open.
///
/// The picker never takes the focus (WS_EX_NOACTIVATE): the program the user types in stays in
/// front, and the keyboard hook hands key presses to the picker while it is visible. With our
/// own window in front, Windows' Win+. (emoji panel) would get through despite the hook.
pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        if platform::is_window_visible(actions::hwnd_of(&w)) {
            hide_logged(app, "toggle");
            return;
        }
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    let fg = platform::foreground_window();
    let Some(w) = actions::ensure_popup(app, LABEL, SIZE.0, SIZE.1, false) else { return };
    let own = actions::hwnd_of(&w);
    platform::set_no_activate(own);
    let target = if fg != own && platform::is_valid_paste_target(fg) { fg } else { 0 };
    app.state::<AppState>().prev_foreground.store(target, Ordering::Release);
    let position = app.state::<AppState>().settings.read().picker.position.clone();
    actions::place_window(app, &w, fg, SIZE.0, SIZE.1, &position);
    let _ = w.emit("picker-shown", ());
    platform::show_no_activate(own);
    // Tauri doesn't know the window is visible (shown via Win32), so WebView2 may have been put
    // to sleep while hidden and would stay blank: tell the webview itself that it is visible.
    let wv: &tauri::Webview = w.as_ref();
    if let Err(e) = wv.show() {
        log::warn!("picker: webview show failed: {e}");
    }
    crate::fill::cancel(app);
    set_key_owner(LABEL);
    platform::redirect_keys(own, fg);
    log::info!("picker: show (not activated), target {target:#x}, foreground {:#x}", platform::foreground_window());
}

/// Hides the picker. Key redirection stops first, so the keyboard is never held even if
/// hiding failed.
pub fn hide(app: &AppHandle) {
    platform::redirect_keys(0, 0);
    if let Some(w) = app.get_webview_window(LABEL) {
        let h = actions::hwnd_of(&w);
        platform::hide_window(h);
        let _ = w.hide();
    }
}

pub fn hide_logged(app: &AppHandle, why: &str) {
    hide(app);
    let visible = app.get_webview_window(LABEL).is_some_and(|w| platform::is_window_visible(actions::hwnd_of(&w)));
    log::info!("picker: hide ({why}), visible after = {visible}");
}

/// Loads the picker page invisibly so the first Win+. opens it at once.
pub fn prewarm(app: &AppHandle) {
    if let Some(w) = actions::ensure_popup(app, LABEL, SIZE.0, SIZE.1, false) {
        platform::set_no_activate(actions::hwnd_of(&w));
        log::info!("picker: pre-warmed");
    }
}

/// Window that currently receives the redirected keys: the picker or the snippet fields popup.
static KEY_OWNER: parking_lot::Mutex<&'static str> = parking_lot::Mutex::new(LABEL);

pub fn set_key_owner(label: &'static str) {
    *KEY_OWNER.lock() = label;
}

/// Receives key presses from the hook (on its thread) and forwards them to the picker page
/// (or the snippet fields popup).
pub fn init_key_redirect(app: &AppHandle) {
    let (tx, rx) = std::sync::mpsc::channel::<platform::RedirKey>();
    let h = app.clone();
    std::thread::Builder::new()
        .name("picker-keys".into())
        .spawn(move || {
            for k in rx {
                let owner = *KEY_OWNER.lock();
                if k.key == "__blur" {
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || {
                        if owner == crate::fill::LABEL {
                            crate::fill::cancel(&h2);
                        } else {
                            hide_logged(&h2, "other window in front");
                        }
                    });
                    continue;
                }
                // Only that window gets typed keys (emit would broadcast to every window).
                let _ = h.emit_to(tauri::EventTarget::webview_window(owner), "picker-key", &k);
            }
        })
        .expect("spawn picker-keys");
    let tx = std::sync::Mutex::new(tx);
    platform::init_key_redirect(move |k| {
        if let Ok(t) = tx.lock() {
            let _ = t.send(k);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_moves_to_front_and_caps() {
        let mut l = vec!["a".to_string(), "b".into(), "c".into()];
        push_recent(&mut l, "c".into(), 3);
        assert_eq!(l, ["c", "a", "b"]);
        push_recent(&mut l, "d".into(), 3);
        assert_eq!(l, ["d", "c", "a"]);
    }

    #[test]
    fn counts_uses() {
        let mut s = PickerState::default();
        count_use(&mut s, "😀");
        count_use(&mut s, "😀");
        count_use(&mut s, "→");
        assert_eq!(s.uses["😀"], 2);
        assert_eq!(s.uses["→"], 1);
    }

    #[test]
    fn state_roundtrip_creates_id() {
        let dir = std::env::temp_dir().join(format!("up-picker-{}", random_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = PickerState::load(&dir);
        assert_eq!(s.customer_id.len(), 32);
        assert_eq!(PickerState::load(&dir).customer_id, s.customer_id);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
