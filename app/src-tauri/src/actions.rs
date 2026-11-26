//! Window and paste mechanics: showing the palette, restoring focus, putting content on the
//! clipboard and sending Ctrl+V to the previous window.

use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::model::{ClipContent, PasteMode};
use crate::platform;
use crate::AppState;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Delivery {
    Pasted,
    Copied,
    /// Target runs as administrator: Windows blocks synthetic input, user must press Ctrl+V.
    CopiedElevated,
}

fn own_hwnd(w: &tauri::WebviewWindow) -> isize {
    #[cfg(windows)]
    {
        w.hwnd().map(|h| h.0 as isize).unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        let _ = w;
        0
    }
}

/// Shows the palette (or hides it if visible and `toggle` is set).
/// Shows the palette for a snippet's input form; the paste target is already stored.
pub fn show_palette_for_form(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let st = app.state::<AppState>();
    let fg = st.prev_foreground.load(Ordering::Acquire);
    let (layout, position) = {
        let s = st.settings.read();
        (s.layout.clone(), s.position.clone())
    };
    place(app, &w, fg, &layout, &position);
    let _ = w.emit("palette-shown", ());
    let _ = w.show();
    let _ = w.set_focus();
}

pub fn show_palette(app: &AppHandle, toggle: bool) {
    let Some(w) = app.get_webview_window("main") else { return };
    if w.is_visible().unwrap_or(false) {
        if toggle {
            hide_palette(app, true);
        } else {
            let _ = w.set_focus();
        }
        return;
    }
    let st = app.state::<AppState>();
    let fg = platform::foreground_window();
    // Only real app windows are paste targets (not the taskbar/tray, not our own windows).
    // Opened from the tray, there is no target: paste then just copies.
    let target = if fg != own_hwnd(&w) && platform::is_valid_paste_target(fg) { fg } else { 0 };
    st.prev_foreground.store(target, Ordering::Release);
    log::info!("palette: show, target {target:#x} ({}), foreground {fg:#x}", platform::exe_of_window(target).as_deref().unwrap_or("-"));
    let (layout, position) = {
        let s = st.settings.read();
        (s.layout.clone(), s.position.clone())
    };
    place(app, &w, fg, &layout, &position);
    let _ = w.emit("palette-shown", ());
    let _ = w.show();
    let _ = w.set_focus();
}

/// Logical window size per layout (the compact/drawer/board windows include a transparent
/// margin for their own CSS shadow).
fn layout_size(app: &AppHandle, layout: &str) -> (f64, f64) {
    if let Some([w, h]) = app.state::<AppState>().settings.read().window_sizes.get(layout).copied() {
        let (_, dh) = default_size(layout);
        return (w.max(360.0), if dh == 0.0 { 0.0 } else { h.max(240.0) });
    }
    default_size(layout)
}

fn default_size(layout: &str) -> (f64, f64) {
    match layout {
        "board" => (980.0, 660.0),
        "drawer" => (452.0, 0.0), // height = work area
        "compact" => (680.0, 430.0),
        _ => (860.0, 520.0),
    }
}

fn scale_at(w: &tauri::WebviewWindow, x: i32, y: i32) -> f64 {
    w.monitor_from_point(x as f64, y as f64)
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .or_else(|| w.scale_factor().ok())
        .unwrap_or(1.0)
}

fn clamp_to(area: platform::Rect, x: i32, y: i32, sw: i32, sh: i32) -> (i32, i32) {
    (x.clamp(area.x, (area.x + area.w - sw).max(area.x)), y.clamp(area.y, (area.y + area.h - sh).max(area.y)))
}

/// Area and position for a popup of logical size (lw, lh) anchored at the text caret of `fg`
/// (or at the mouse when the app does not expose its caret).
fn caret_rect(w: &tauri::WebviewWindow, fg: isize, lw: f64, lh: f64) -> Option<(platform::Rect, i32, i32, i32, i32)> {
    let (ax, ay) = platform::caret_screen_pos(fg).or_else(|| platform::work_area_at_cursor().map(|(_, x, y)| (x, y)))?;
    let a = platform::work_area_at_point(ax, ay)?;
    let s = scale_at(w, ax, ay);
    let (sw, sh) = ((lw * s) as i32, (lh * s) as i32);
    let margin = (16.0 * s) as i32;
    let mut y = ay + (4.0 * s) as i32 - margin;
    if y + sh > a.y + a.h {
        // Not enough room below the line: open above it.
        y = ay - (28.0 * s) as i32 - sh + margin;
    }
    Some((a, ax - margin - (8.0 * s) as i32, y, sw, sh))
}

/// Window rectangle (work area, x, y, physical width, height) for a window of logical size
/// `lw` × `lh` opened for the program `fg`: `position` is "caret" (text cursor, mouse if it
/// can't be found), "mouse"/"cursor" or "center" (screen of `fg`).
pub fn position_rect(w: &tauri::WebviewWindow, fg: isize, lw: f64, lh: f64, position: &str) -> Option<(platform::Rect, i32, i32, i32, i32)> {
    match position {
        "caret" => caret_rect(w, fg, lw, lh),
        "mouse" | "cursor" => {
            let (a, cx, cy) = platform::work_area_at_cursor()?;
            let s = scale_at(w, cx, cy);
            let (sw, sh) = ((lw * s) as i32, (lh * s) as i32);
            Some((a, cx - sw / 2, cy - (40.0 * s) as i32, sw, sh))
        }
        _ => {
            let a = platform::work_area_for_window(fg).or_else(|| platform::work_area_at_cursor().map(|(a, _, _)| a))?;
            let s = scale_at(w, a.x + a.w / 2, a.y + a.h / 3);
            let (sw, sh) = ((lw * s) as i32, (lh * s) as i32);
            Some((a, a.x + (a.w - sw) / 2, a.y + (a.h - sh) / 3, sw, sh))
        }
    }
}

/// Places a window with the given logical size by `position` (see [`position_rect`]).
pub fn place_window(app: &AppHandle, w: &tauri::WebviewWindow, fg: isize, lw: f64, lh: f64, position: &str) {
    let Some((area, x, y, sw, sh)) = position_rect(w, fg, lw, lh, position) else { return };
    app.state::<AppState>().placing.store(true, Ordering::Release);
    let (x, y) = clamp_to(area, x, y, sw.min(area.w), sh.min(area.h));
    let _ = w.set_size(PhysicalSize::new(sw.clamp(200, area.w.max(200)) as u32, sh.clamp(200, area.h.max(200)) as u32));
    let _ = w.set_position(PhysicalPosition::new(x, y));
    app.state::<AppState>().placing.store(false, Ordering::Release);
}

/// Popup windows that close when focus moves elsewhere and are hidden before pasting.
pub const POPUPS: [&str; 2] = ["main", "picker"];

/// Returns the popup window `label`, creating it (hidden, frameless, transparent) on first use.
/// Must run on the main thread or in an async context (never in a sync command).
pub fn ensure_popup(app: &AppHandle, label: &str, lw: f64, lh: f64, focus: bool) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(label) {
        return Some(w);
    }
    let r = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("UltimatePaste")
        .inner_size(lw, lh)
        .visible(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(focus)
        .build();
    match r {
        Ok(w) => {
            crate::apply_theme(app);
            Some(w)
        }
        Err(e) => {
            log::error!("create window {label}: {e}");
            None
        }
    }
}

/// Native window handle (0 if unavailable).
pub fn hwnd_of(w: &tauri::WebviewWindow) -> isize {
    own_hwnd(w)
}

/// Sizes and positions the palette for the chosen layout.
fn place(app: &AppHandle, w: &tauri::WebviewWindow, fg: isize, layout: &str, position: &str) {
    let fallback_area = || platform::work_area_at_cursor().map(|(a, _, _)| a);
    let (lw, lh) = layout_size(app, layout);
    app.state::<AppState>().placing.store(true, Ordering::Release);
    let (area, x, y, sw, sh) = match layout {
        "drawer" => {
            let Some(a) = platform::work_area_for_window(fg).or_else(fallback_area) else { return };
            let s = scale_at(w, a.x + a.w / 2, a.y + a.h / 2);
            let sw = (lw * s) as i32;
            (a, a.x + a.w - sw, a.y, sw, a.h)
        }
        "compact" => match caret_rect(w, fg, lw, lh) {
            Some(v) => v,
            None => return,
        },
        _ => match position_rect(w, fg, lw, lh, position) {
            Some(v) => v,
            None => return,
        },
    };
    let (x, y) = clamp_to(area, x, y, sw.min(area.w), sh.min(area.h));
    let _ = w.set_size(PhysicalSize::new(sw.clamp(200, area.w.max(200)) as u32, sh.clamp(200, area.h.max(200)) as u32));
    let _ = w.set_position(PhysicalPosition::new(x, y));
    app.state::<AppState>().placing.store(false, Ordering::Release);
}

/// Remembers a window size the user chose (debounced save to settings.json).
pub fn remember_size(app: &AppHandle, label: &str, size: tauri::PhysicalSize<u32>, scale: f64) {
    let st = app.state::<AppState>();
    if st.placing.load(Ordering::Acquire) || size.width < 100 || size.height < 100 {
        return;
    }
    // Only the palette (per layout), settings and translate windows remember a user size.
    let key = match label {
        "main" => st.settings.read().layout.clone(),
        "settings" | "translate" => label.to_string(),
        _ => return,
    };
    let logical = size.to_logical::<f64>(scale);
    {
        let mut s = st.settings.write();
        let entry = [logical.width.round(), logical.height.round()];
        if s.window_sizes.get(&key) == Some(&entry) {
            return;
        }
        s.window_sizes.insert(key, entry);
    }
    let gen = st.size_save_gen.fetch_add(1, Ordering::AcqRel) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        let st = app.state::<AppState>();
        if st.size_save_gen.load(Ordering::Acquire) == gen {
            let s = st.settings.read().clone();
            if let Err(e) = s.save(&st.data_dir) {
                log::warn!("save window size: {e}");
            }
        }
    });
}

/// Window chrome per layout: the Windows-style layout uses Mica + the native shadow; the
/// others are fully transparent windows that draw their own rounded shapes and shadows.
pub fn apply_layout(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let fluent = app.state::<AppState>().settings.read().layout == "fluent";
    if fluent {
        let _ = w.set_effects(tauri::window::EffectsBuilder::new().effect(tauri::window::Effect::Mica).build());
    } else {
        let _ = w.set_effects(None);
    }
    let _ = w.set_shadow(fluent);
}

pub fn hide_palette(app: &AppHandle, restore_focus: bool) {
    if restore_focus {
        let target = app.state::<AppState>().prev_foreground.load(Ordering::Acquire);
        platform::set_foreground(target);
    }
    hide_popups(app);
}

fn hide_popups(app: &AppHandle) {
    platform::redirect_keys(0, 0);
    for label in POPUPS {
        if let Some(w) = app.get_webview_window(label) {
            // The picker is shown via Win32 (not activated); Tauri's hide alone wouldn't reach it.
            platform::hide_window(own_hwnd(&w));
            let _ = w.hide();
        }
    }
}

/// The welcome tour (Q27): a normal window in the middle of the screen.
pub fn open_tour(app: &AppHandle) {
    hide_palette(app, false);
    if let Some(w) = app.get_webview_window("tour") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let title = crate::i18n::tr(app, "Willkommen bei UltimatePaste", "Welcome to UltimatePaste");
    let r = WebviewWindowBuilder::new(app, "tour", WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(760.0, 600.0)
        .min_inner_size(640.0, 520.0)
        .center()
        .build();
    match r {
        Ok(_) => crate::apply_theme(app),
        Err(e) => log::error!("open tour: {e}"),
    }
}

pub fn open_settings(app: &AppHandle) {
    hide_palette(app, false);
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let title = crate::i18n::tr(app, "UltimatePaste – Einstellungen", "UltimatePaste – Settings");
    let [sw, sh] = app.state::<AppState>().settings.read().window_sizes.get("settings").copied().unwrap_or([820.0, 680.0]);
    let r = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(sw.max(640.0), sh.max(480.0))
        .min_inner_size(640.0, 480.0)
        .center()
        .build();
    if r.is_ok() {
        crate::apply_theme(app);
    }
    if let Err(e) = r {
        log::error!("open settings: {e}");
    }
}

/// Writes content to the clipboard without recording it as a new history entry.
pub fn write_silently(app: &AppHandle, c: &ClipContent) -> anyhow::Result<()> {
    let st = app.state::<AppState>();
    st.writing.store(true, Ordering::Release);
    let r = platform::write_clipboard(c);
    if let Ok(seq) = &r {
        st.ignore_seq.store(*seq, Ordering::Release);
    }
    st.writing.store(false, Ordering::Release);
    r.map(|_| ())
}

/// Paste rule of the program that owns window `target`, if any.
pub fn rule_for(app: &AppHandle, target: isize) -> Option<String> {
    if target == 0 {
        return None;
    }
    let exe = platform::exe_of_window(target)?.to_lowercase();
    let st = app.state::<AppState>();
    let s = st.settings.read();
    s.app_rules.iter().find(|r| r.app.trim().to_lowercase() == exe || r.app.trim().to_lowercase() == exe.trim_end_matches(".exe")).map(|r| r.mode.clone())
}

/// Puts content on the clipboard and (unless `Copy`) pastes it into the previous window.
/// Ctrl+V is only sent once the target window really is in the foreground again.
pub fn deliver(app: &AppHandle, content: &ClipContent, mode: PasteMode, cursor_back: Option<usize>) -> anyhow::Result<Delivery> {
    let target = app.state::<AppState>().prev_foreground.load(Ordering::Acquire);
    // A program rule only applies to a normal paste (not when "paste as …" was chosen).
    let ruled = if mode == PasteMode::Default { rule_for(app, target).map(|r| content.with_rule(&r)) } else { None };
    write_silently(app, &ruled.unwrap_or_else(|| content.for_mode(mode)))?;
    if mode == PasteMode::Copy {
        hide_palette(app, true);
        return Ok(Delivery::Copied);
    }
    // The target may have been hidden (closed to the tray) while the palette was open.
    if target == 0 || !platform::is_valid_paste_target(target) {
        hide_palette(app, false);
        return Ok(Delivery::Copied);
    }
    if platform::is_window_elevated_for_us(target) {
        // The palette shows a hint and closes itself.
        return Ok(Delivery::CopiedElevated);
    }
    // We are foreground right now, so Windows allows us to hand focus back.
    platform::set_foreground(target);
    hide_popups(app);
    if !platform::wait_foreground(target, 250) {
        log::info!("paste target did not regain focus; content left on the clipboard");
        return Ok(Delivery::Copied);
    }
    std::thread::sleep(Duration::from_millis(40));
    // Something else may have come to the front in the meantime (a notification, a program
    // that grabs the focus): then rather leave it on the clipboard than paste there.
    if platform::foreground_window() != target {
        log::info!("paste target lost the focus again ({:#x} in front); content left on the clipboard", platform::foreground_window());
        return Ok(Delivery::Copied);
    }
    log::info!("paste into {}", platform::exe_of_window(target).as_deref().unwrap_or("?"));
    if !platform::send_paste() {
        return Ok(Delivery::Copied);
    }
    if let Some(n) = cursor_back {
        std::thread::sleep(Duration::from_millis(60));
        platform::send_left(n);
    }
    crate::stats::record(app, crate::stats::Event::App(platform::exe_of_window(target)));
    Ok(Delivery::Pasted)
}

/// Clipboard content to put back after a paste from the picker, while that is still pending.
static RESTORE: parking_lot::Mutex<Option<ClipContent>> = parking_lot::Mutex::new(None);
static RESTORE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Like [`deliver`], but puts the previous clipboard content back `delay_ms` after pasting:
/// for things picked rather than copied (emoji, symbols, GIFs), so they don't replace what
/// the user copied. "Copy only" and a paste that fell back to copying keep the new content.
pub fn deliver_and_restore(app: &AppHandle, content: &ClipContent, mode: PasteMode, delay_ms: u64) -> anyhow::Result<Delivery> {
    if mode == PasteMode::Copy {
        RESTORE.lock().take();
        return deliver(app, content, mode, None);
    }
    // Several quick pastes in a row: keep the content from before the first one.
    let before = RESTORE.lock().take().unwrap_or_else(|| {
        let limits = platform::Limits { max_text_bytes: 5 << 20, max_image_bytes: 50 << 20 };
        platform::read_clipboard(limits).map(|c| ClipContent { private: c.excluded, ..c.content }).unwrap_or_default()
    });
    let d = deliver(app, content, mode, None)?;
    if d == Delivery::Pasted && !before.is_empty() {
        *RESTORE.lock() = Some(before);
        let gen = RESTORE_GEN.fetch_add(1, Ordering::AcqRel) + 1;
        let seq = platform::clipboard_seq();
        let app = app.clone();
        std::thread::spawn(move || {
            // Give the program time to read the clipboard.
            std::thread::sleep(Duration::from_millis(delay_ms));
            if RESTORE_GEN.load(Ordering::Acquire) != gen {
                return; // a newer paste takes care of it
            }
            let Some(before) = RESTORE.lock().take() else { return };
            // Only if nothing else was copied in the meantime.
            if platform::clipboard_seq() == seq {
                let _ = write_silently(&app, &before);
            }
        });
    }
    Ok(d)
}

/// Each press pastes the next older history entry (newest first); restarts after the timeout.
/// The `seq` mutex is held for the whole press, so quick repeated presses are processed strictly
/// one after another (no clipboard write can overtake a pending Ctrl+V).
pub fn sequential_paste(app: &AppHandle) {
    let st = app.state::<AppState>();
    let (timeout, newline) = {
        let s = st.settings.read();
        (Duration::from_secs(s.sequential_timeout_secs.max(5) as u64), s.sequential_newline)
    };
    let mut seq = st.seq.lock();
    let expired = seq.last.is_none_or(|l| l.elapsed() > timeout);
    if expired || seq.pos >= seq.ids.len() {
        seq.ids = st.index.read().recent_ids();
        seq.pos = 0;
    }
    let Some(id) = seq.ids.get(seq.pos).copied() else { return };
    seq.pos += 1;
    seq.last = Some(Instant::now());

    let content = st.store.lock().get_content(id);
    let Ok(Some(mut content)) = content else { return };
    if newline {
        if let Some(t) = content.text.as_mut() {
            t.push_str("\r\n");
            content.html = None;
            content.rtf = None;
        }
    }
    let target = platform::foreground_window();
    if let Some(rule) = rule_for(app, target) {
        content = content.with_rule(&rule);
    }
    if let Err(e) = write_silently(app, &content) {
        log::warn!("sequential paste: {e:#}");
        return;
    }
    if !platform::is_valid_paste_target(target) || platform::is_window_elevated_for_us(target) {
        return;
    }
    std::thread::sleep(Duration::from_millis(30));
    platform::send_paste();
    crate::stats::record(app, crate::stats::Event::Sequential);
    crate::stats::record(app, crate::stats::Event::App(platform::exe_of_window(target)));
    // Give the target time to read the clipboard before the next press overwrites it.
    std::thread::sleep(Duration::from_millis(80));
    seq.last = Some(Instant::now());
}
