//! Small popup at the text caret that asks for a typed snippet's fields (choices and inputs).
//! Like the picker it never takes the focus: the keyboard hook hands key presses to it, and the
//! typed keyword stays in the text until a value is chosen.

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::{actions, platform, snippets, AppState};

pub const LABEL: &str = "fill";
/// Start size (logical px, incl. the 16 px transparent margin); the page fits it to its content.
const SIZE: (f64, f64) = (340.0, 200.0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub fields: Vec<snippets::Field>,
    #[serde(skip)]
    body: String,
    /// Window the keyword was typed in
    #[serde(skip)]
    target: isize,
    /// Characters of the typed keyword (removed when pasting)
    #[serde(skip)]
    keyword_chars: usize,
}

static JOB: Mutex<Option<Job>> = Mutex::new(None);

/// Opens the popup for a snippet with fields, typed as a keyword in window `target`.
pub fn open(app: &AppHandle, body: String, target: isize, keyword_chars: usize) {
    let fields = snippets::fields(&body);
    *JOB.lock() = Some(Job { fields, body, target, keyword_chars });
    crate::picker::hide(app);
    let Some(w) = actions::ensure_popup(app, LABEL, SIZE.0, SIZE.1, false) else { return };
    let own = actions::hwnd_of(&w);
    platform::set_no_activate(own);
    actions::place_window(app, &w, target, SIZE.0, SIZE.1, "caret");
    let _ = w.emit_to(tauri::EventTarget::webview_window(LABEL), "fill-shown", ());
    platform::show_no_activate(own);
    let wv: &tauri::Webview = w.as_ref();
    let _ = wv.show();
    crate::picker::set_key_owner(LABEL);
    platform::redirect_keys(own, target);
    // Should Windows have activated the popup anyway, give the focus back right away.
    let fg = platform::foreground_window();
    if fg != target {
        log::info!("fill: foreground {fg:#x} instead of target {target:#x}, giving it back");
        platform::set_foreground(target);
    }
    log::info!("fill: show for a typed snippet");
}

/// Loads the popup invisibly, so that opening it never creates (and activates) a new window.
pub fn prewarm(app: &AppHandle) {
    if let Some(w) = actions::ensure_popup(app, LABEL, SIZE.0, SIZE.1, false) {
        platform::set_no_activate(actions::hwnd_of(&w));
    }
}

/// Hides the popup (key redirection stops first).
pub fn hide(app: &AppHandle) {
    platform::redirect_keys(0, 0);
    if let Some(w) = app.get_webview_window(LABEL) {
        platform::hide_window(actions::hwnd_of(&w));
        let _ = w.hide();
    }
}

/// Closes without pasting: the keyword stays as typed.
pub fn cancel(app: &AppHandle) {
    JOB.lock().take();
    hide(app);
}

pub fn job() -> Option<Job> {
    JOB.lock().clone()
}

/// Removes the keyword and pastes the snippet with the given values.
pub fn submit(app: &AppHandle, values: HashMap<String, String>) {
    let Some(job) = JOB.lock().take() else { return };
    // The target must be in front when the popup goes away: if the popup was activated,
    // Windows would otherwise pass the focus to some other window (e.g. Explorer).
    if platform::foreground_window() != job.target {
        platform::set_foreground(job.target);
    }
    hide(app);
    let app = app.clone();
    std::thread::spawn(move || {
        if platform::foreground_window() != job.target {
            platform::set_foreground(job.target);
            if !platform::wait_foreground(job.target, 400) {
                log::info!("fill: target {:#x} not in front ({:#x}), nothing pasted", job.target, platform::foreground_window());
                return;
            }
        }
        // Enter / the digit that confirmed the popup must be up before keys are sent.
        if !platform::wait_redirected_keys_released(800) {
            log::info!("fill: confirming key still down after 800 ms");
        }
        std::thread::sleep(Duration::from_millis(30));
        platform::send_backspaces(job.keyword_chars);
        let st = app.state::<AppState>();
        let lang = st.settings.read().language.clone();
        let limits = platform::Limits { max_text_bytes: 5 << 20, max_image_bytes: 50 << 20 };
        let clip = platform::read_clipboard(limits).ok().and_then(|c| if c.excluded { None } else { c.content.text });
        let ex = snippets::expand_with(&job.body, chrono::Local::now(), &lang, clip.as_deref(), &values);
        crate::typing::paste_keeping_clipboard(&app, &ex.text, ex.cursor_back);
    });
}

/// Fits the window to the page content (logical px).
pub fn fit(app: &AppHandle, w: f64, h: f64) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.set_size(tauri::LogicalSize::new(w.clamp(160.0, 720.0), h.clamp(80.0, 640.0)));
    }
}
