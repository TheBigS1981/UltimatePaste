//! History of the translate / correct window (Q20–Q23): the last 20 runs, one entry per
//! session (live re-runs update it), encrypted with the same vault as the clipboard history.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::model::now_ms;
use crate::AppState;

pub const MAX: usize = 20;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    /// Start time of the session (ms), also its id
    pub id: i64,
    pub updated_at: i64,
    /// "translate" | "correct"
    pub mode: String,
    pub text: String,
    pub out: String,
    pub source: Option<String>,
    pub target: Option<String>,
    pub engine: String,
    /// For "correct": "correct" | "rephrase"
    pub kind: Option<String>,
    pub style: Option<String>,
}

/// Puts `s` first (replacing the entry with the same id) and keeps at most `MAX`.
pub fn upsert_in(list: &mut Vec<Session>, mut s: Session, now: i64) {
    if s.text.trim().is_empty() {
        return;
    }
    s.updated_at = now;
    list.retain(|x| x.id != s.id);
    list.insert(0, s);
    list.truncate(MAX);
}

fn path(app: &AppHandle) -> std::path::PathBuf {
    app.state::<AppState>().data_dir.join("translate-history.bin")
}

static LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

pub fn load(app: &AppHandle) -> Vec<Session> {
    let st = app.state::<AppState>();
    let Ok(bytes) = std::fs::read(path(app)) else { return Vec::new() };
    let plain = st.store.lock().vault().decrypt(&bytes);
    plain.ok().and_then(|p| serde_json::from_slice(&p).ok()).unwrap_or_default()
}

fn save(app: &AppHandle, list: &[Session]) {
    let st = app.state::<AppState>();
    let Ok(json) = serde_json::to_vec(list) else { return };
    let enc = st.store.lock().vault().encrypt(&json);
    if let Err(e) = std::fs::write(path(app), enc) {
        log::warn!("translate history: {e}");
    }
}

pub fn upsert(app: &AppHandle, s: Session) {
    let _g = LOCK.lock();
    let mut list = load(app);
    upsert_in(&mut list, s, now_ms());
    save(app, &list);
}

/// Marks the newest session as just used (after pasting its result, so that the result,
/// now the newest clipboard entry, does not count as "new text").
pub fn touch_latest(app: &AppHandle) {
    let _g = LOCK.lock();
    let mut list = load(app);
    if let Some(first) = list.first_mut() {
        first.updated_at = now_ms() + 2000;
        save(app, &list);
    }
}

pub fn clear(app: &AppHandle) {
    let _g = LOCK.lock();
    let _ = std::fs::remove_file(path(app));
}

/// Without a selection: continue the last session unless the clipboard holds something
/// copied after it (then that clipboard text is the new job).
pub fn should_restore(last_updated: Option<i64>, clipboard_empty: bool, clipboard_copied_at: Option<i64>) -> bool {
    match last_updated {
        None => false,
        Some(t) => clipboard_empty || clipboard_copied_at.is_none_or(|c| c <= t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: i64, text: &str) -> Session {
        Session { id, text: text.into(), mode: "translate".into(), ..Default::default() }
    }

    #[test]
    fn upsert_replaces_and_caps() {
        let mut l = Vec::new();
        upsert_in(&mut l, s(1, "a"), 10);
        upsert_in(&mut l, s(2, "b"), 20);
        upsert_in(&mut l, s(1, "a2"), 30);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].text, "a2");
        assert_eq!(l[0].updated_at, 30);
        upsert_in(&mut l, s(3, "  "), 40);
        assert_eq!(l.len(), 2);
        for i in 10..40 {
            upsert_in(&mut l, s(i, "x"), i);
        }
        assert_eq!(l.len(), MAX);
    }

    #[test]
    fn restore_rule() {
        assert!(!should_restore(None, true, None));
        assert!(should_restore(Some(100), true, Some(200)));
        assert!(should_restore(Some(100), false, Some(50)));
        assert!(!should_restore(Some(100), false, Some(150)));
        assert!(should_restore(Some(100), false, None));
    }
}
