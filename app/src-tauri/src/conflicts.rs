//! Detects shortcuts that UltimatePaste shares with Windows or PowerToys, and frees Win+V
//! in Windows on request (Explorer's `DisabledHotkeys` registry value).

use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use tauri::{AppHandle, Manager};

use crate::settings::Settings;
use crate::{platform, AppState};

const VK_V: u32 = 0x56;
const VK_PERIOD: u32 = 0xBE;
const VK_SEMICOLON: u32 = 0xBA;
/// VK_OEM_PERIOD as a character of Explorer's DisabledHotkeys value.
const PERIOD_CHAR: char = '\u{BE}';

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// Hotkey id (`HK_*`)
    pub id: u8,
    pub hotkey: String,
    /// "windows-v" (clipboard history, can be switched off), "windows-emoji" (hint only),
    /// "powertoys" (change it there)
    pub kind: &'static str,
    /// PowerToys module name
    pub module: Option<String>,
    /// Windows' own shortcut is already switched off
    pub resolved: bool,
    /// Stable id for "don't ask again"
    pub key: String,
}

/// A PowerToys shortcut: (module, win, ctrl, alt, shift, vk).
type PtHotkey = (String, bool, bool, bool, bool, u32);

fn collect_hotkeys(v: &Value, module: &str, out: &mut Vec<PtHotkey>) {
    match v {
        Value::Object(m) => {
            let b = |k: &str| m.get(k).and_then(Value::as_bool);
            if let (Some(win), Some(code)) = (b("win"), m.get("code").and_then(Value::as_u64)) {
                if code != 0 {
                    out.push((module.to_string(), win, b("ctrl").unwrap_or(false), b("alt").unwrap_or(false), b("shift").unwrap_or(false), code as u32));
                }
                return;
            }
            m.values().for_each(|x| collect_hotkeys(x, module, out));
        }
        Value::Array(a) => a.iter().for_each(|x| collect_hotkeys(x, module, out)),
        _ => {}
    }
}

/// Shortcuts of enabled PowerToys modules, read from PowerToys' settings folder.
pub fn powertoys_hotkeys_in(root: &Path) -> Vec<PtHotkey> {
    let read = |p: &Path| std::fs::read(p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let Some(general) = read(&root.join("settings.json")) else { return Vec::new() };
    let Some(enabled) = general.get("enabled").and_then(Value::as_object) else { return Vec::new() };
    let mut out = Vec::new();
    for (module, on) in enabled {
        if on.as_bool() != Some(true) {
            continue;
        }
        if let Some(v) = read(&root.join(module).join("settings.json")) {
            collect_hotkeys(&v, module, &mut out);
        }
    }
    out
}

fn powertoys_root() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let p = Path::new(&base).join("Microsoft").join("PowerToys");
    p.is_dir().then_some(p)
}

pub fn find_with(s: &Settings, disabled: &str, pt: &[PtHotkey]) -> Vec<Conflict> {
    let mut out = Vec::new();
    for (id, text) in crate::hotkey_texts(s) {
        let Some(c) = platform::parse_combo(&text, id) else { continue };
        let c_ = |kind: &'static str, module: Option<String>, resolved| {
            let key = format!("{kind}:{}:{}", module.as_deref().unwrap_or(""), text.to_ascii_lowercase());
            Conflict { id, hotkey: text.clone(), kind, module, resolved, key }
        };
        if c.win && !c.ctrl && !c.alt && c.vk == VK_V {
            out.push(c_("windows-v", None, disabled.to_ascii_uppercase().contains('V')));
        }
        if c.win && !c.ctrl && !c.alt && !c.shift && (c.vk == VK_PERIOD || c.vk == VK_SEMICOLON) {
            out.push(c_("windows-emoji", None, false));
        }
        for (module, win, ctrl, alt, shift, vk) in pt {
            if (*win, *ctrl, *alt, *shift, *vk) == (c.win, c.ctrl, c.alt, c.shift, c.vk) {
                out.push(c_("powertoys", Some(module.clone()), false));
            }
        }
    }
    out
}

pub fn find(app: &AppHandle) -> Vec<Conflict> {
    let s = app.state::<AppState>().settings.read().clone();
    let pt = powertoys_root().map(|r| powertoys_hotkeys_in(&r)).unwrap_or_default();
    find_with(&s, &platform::shell_disabled_hotkeys(), &pt)
}

/// Switches Windows' own Win+V (clipboard history) off or back on. Explorer reads this at
/// sign-in, so it takes effect after signing out and in again (no Explorer restart: killing
/// Explorer is disruptive and looks like malware to virus scanners).
pub fn set_windows_v(app: &AppHandle, off: bool) -> anyhow::Result<()> {
    let cur = platform::shell_disabled_hotkeys();
    // Also drop "¾" written by 0.12.2 (it has no effect on Win+.).
    let mut keys: String = cur.chars().filter(|c| !c.eq_ignore_ascii_case(&'v') && *c != PERIOD_CHAR).collect();
    if off {
        keys.push('V');
    }
    platform::set_shell_disabled_hotkeys(&keys)?;
    log::info!("Windows Win+V {} (effective after the next sign-in)", if off { "switched off" } else { "restored" });
    crate::register_hotkeys(app);
    Ok(())
}

/// Conflicts worth a notice: not yet solved, actionable, not dismissed for good.
pub fn open_notices(app: &AppHandle) -> Vec<Conflict> {
    let dismissed = app.state::<AppState>().settings.read().notified_conflicts.clone();
    find(app).into_iter().filter(|c| !c.resolved && c.kind != "windows-emoji" && !dismissed.contains(&c.key)).collect()
}

/// "Don't ask again" for these conflicts.
pub fn dismiss(app: &AppHandle, keys: Vec<String>) {
    let st = app.state::<AppState>();
    let mut s = st.settings.write();
    for k in keys {
        if !s.notified_conflicts.contains(&k) {
            s.notified_conflicts.push(k);
        }
    }
    let _ = s.save(&st.data_dir);
}

pub const NOTICE: &str = "notice";

/// After start: a small window in the middle of the screen when a shortcut is shared with
/// Windows or PowerToys, with buttons to fix it right away.
pub fn notify_on_start(app: &AppHandle) {
    if open_notices(app).is_empty() {
        return;
    }
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = h.get_webview_window(NOTICE) {
            let _ = w.show();
            let _ = w.set_focus();
            return;
        }
        let r = tauri::WebviewWindowBuilder::new(&h, NOTICE, tauri::WebviewUrl::App("index.html".into()))
            .title("UltimatePaste")
            .inner_size(500.0, 260.0)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .always_on_top(true)
            .center()
            .build();
        match r {
            Ok(_) => crate::apply_theme(&h),
            Err(e) => log::error!("open notice window: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_conflicts() {
        let mut s = Settings { hotkey: "Super+V".into(), picker_hotkey: "Super+Period".into(), ..Default::default() };
        s.translate_hotkey = "Shift+Super+V".into();
        let pt = vec![("AdvancedPaste".to_string(), true, false, false, true, 0x56)];
        let c = find_with(&s, "", &pt);
        let kinds: Vec<_> = c.iter().map(|c| (c.id, c.kind, c.resolved)).collect();
        assert!(kinds.contains(&(crate::HK_MAIN, "windows-v", false)));
        assert!(kinds.contains(&(crate::HK_PICKER, "windows-emoji", false)));
        assert!(kinds.contains(&(crate::HK_TRANSLATE, "windows-v", false)));
        assert!(kinds.contains(&(crate::HK_TRANSLATE, "powertoys", false)));
        assert!(!kinds.iter().any(|k| k.0 == crate::HK_MAIN && k.1 == "powertoys"));
        assert!(find_with(&s, "CV", &[]).iter().filter(|c| c.kind == "windows-v").all(|c| c.resolved));
    }

    #[test]
    fn reads_powertoys_settings() {
        let dir = std::env::temp_dir().join(format!("up-pt-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("AdvancedPaste")).unwrap();
        std::fs::create_dir_all(dir.join("ColorPicker")).unwrap();
        std::fs::write(dir.join("settings.json"), r#"{"enabled":{"AdvancedPaste":true,"ColorPicker":false}}"#).unwrap();
        std::fs::write(
            dir.join("AdvancedPaste/settings.json"),
            r#"{"properties":{"advanced-paste-ui-hotkey":{"win":true,"ctrl":false,"alt":false,"shift":true,"code":86,"key":""},"custom-actions":{"value":[{"shortcut":{"win":true,"ctrl":false,"alt":false,"shift":false,"code":0}}]}}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("ColorPicker/settings.json"), r#"{"properties":{"ActivationShortcut":{"win":true,"ctrl":false,"alt":false,"shift":true,"code":67}}}"#).unwrap();
        let hk = powertoys_hotkeys_in(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(hk, vec![("AdvancedPaste".to_string(), true, false, false, true, 0x56)]);
    }
}
