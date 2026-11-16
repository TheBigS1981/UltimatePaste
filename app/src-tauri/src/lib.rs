//! UltimatePaste – application wiring: state, tray, hotkeys, clipboard capture, retention.

pub mod actions;
pub mod ai;
pub mod capture;
pub mod cleanlinks;
pub mod commands;
pub mod conflicts;
pub mod gif;
pub mod i18n;
pub mod crypto;
pub mod inbox;
pub mod model;
pub mod picker;
pub mod platform;
pub mod search;
pub mod sensitive;
pub mod settings;
pub mod snippets;
pub mod fill;
pub mod stats;
pub mod store;
pub mod thist;
pub mod transfer;
pub mod translate;
pub mod typing;
pub mod updater;

use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64};
use std::time::Instant;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::search::Index;
use crate::settings::Settings;
use crate::store::Store;

pub struct SeqPaste {
    pub ids: Vec<i64>,
    pub pos: usize,
    pub last: Option<Instant>,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub store: Mutex<Store>,
    pub index: RwLock<Index>,
    pub settings: RwLock<Settings>,
    /// Window that was in the foreground before the palette opened (paste target).
    pub prev_foreground: AtomicIsize,
    /// Clipboard sequence number of our own last write (not recorded as new entry).
    pub ignore_seq: AtomicU32,
    pub writing: AtomicBool,
    /// Keyed hashes of excluded/sensitive content seen recently (memory only).
    pub excluded_hashes: Mutex<VecDeque<[u8; 32]>>,
    pub seq: Mutex<SeqPaste>,
    /// Registered global shortcuts with their hotkey id (`HK_*`).
    pub hotkeys: Mutex<Vec<(u8, Shortcut)>>,
    /// Set while we size/position the palette ourselves (those resizes are not user choices).
    pub placing: AtomicBool,
    pub size_save_gen: AtomicU64,
    pub picker: Mutex<picker::PickerState>,
    /// Clipboard changes before this time (ms) are not recorded (selection grab for translation).
    pub grab_until: AtomicU64,
    /// Text waiting for the translate window / toast window (they may still be loading).
    pub translate_pending: Mutex<Option<translate::Pending>>,
    /// Snippet whose input form the palette shows next (typed keyword with fields).
    pub snippet_form: Mutex<Option<i64>>,
    /// Result of the last update check.
    pub update: Mutex<Option<updater::UpdateInfo>>,
    pub toast_text: Mutex<String>,
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("UltimatePaste")
}

fn init_logging(dir: &std::path::Path) {
    let path = dir.join("ultimatepaste.log");
    // Keep the log small: start fresh when it grows beyond 1 MB.
    if std::fs::metadata(&path).map(|m| m.len() > 1 << 20).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        // Times in local time (the default would be UTC).
        let mut cfg = simplelog::ConfigBuilder::new();
        let _ = cfg.set_time_offset_to_local();
        let _ = simplelog::WriteLogger::init(log::LevelFilter::Info, cfg.build(), f);
    }
}

fn build_state() -> anyhow::Result<AppState> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    init_logging(&dir);
    log::info!("UltimatePaste {} starting, data dir {}", env!("CARGO_PKG_VERSION"), dir.display());
    let settings = Settings::load(&dir);
    let vault = crypto::Vault::open(&dir)?;
    let store = Store::open(&dir, vault)?;

    let started = Instant::now();
    let mut index = Index::default();
    for row in store.load_all()? {
        index.upsert(row.summary, &row.content, row.ocr.as_deref());
    }
    log::info!("index built: {} entries in {:?}", index.len(), started.elapsed());

    Ok(AppState {
        data_dir: dir.clone(),
        store: Mutex::new(store),
        index: RwLock::new(index),
        settings: RwLock::new(settings),
        prev_foreground: AtomicIsize::new(0),
        ignore_seq: AtomicU32::new(0),
        writing: AtomicBool::new(false),
        excluded_hashes: Mutex::new(VecDeque::new()),
        seq: Mutex::new(SeqPaste { ids: Vec::new(), pos: 0, last: None }),
        hotkeys: Mutex::new(Vec::new()),
        placing: AtomicBool::new(false),
        size_save_gen: AtomicU64::new(0),
        picker: Mutex::new(picker::PickerState::load(&dir)),
        grab_until: AtomicU64::new(0),
        translate_pending: Mutex::new(None),
        snippet_form: Mutex::new(None),
        update: Mutex::new(None),
        toast_text: Mutex::new(String::new()),
    })
}

/// Human-readable form of a hotkey string, e.g. "Ctrl+Super+V" → "Strg + Win + V".
pub fn hotkey_label(text: &str, lang: &str) -> String {
    text.split('+')
        .map(|p| match p.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => i18n::tr_lang(lang, "Strg", "Ctrl", &[]),
            "super" | "win" | "meta" => "Win".to_string(),
            "shift" => i18n::tr_lang(lang, "Umschalt", "Shift", &[]),
            "alt" => "Alt".to_string(),
            "period" => ".".to_string(),
            "comma" => ",".to_string(),
            _ => p.trim().to_string(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

pub struct HotkeyResult {
    pub errors: Vec<String>,
    /// Hotkey ids (`HK_*`) that could not be set up.
    pub failed: Vec<u8>,
}

pub const HK_MAIN: u8 = 0;
pub const HK_SEQ: u8 = 1;
pub const HK_PICKER: u8 = 2;
pub const HK_TRANSLATE: u8 = 3;
pub const HK_TRANSLATE_ALT: u8 = 4;
pub const HK_CORRECT: u8 = 5;

/// The configured global hotkeys by id.
pub fn hotkey_texts(s: &Settings) -> Vec<(u8, String)> {
    vec![
        (HK_MAIN, s.hotkey.clone()),
        (HK_SEQ, s.sequential_hotkey.clone()),
        (HK_PICKER, s.picker_hotkey.clone()),
        (HK_TRANSLATE, s.translate_hotkey.clone()),
        (HK_TRANSLATE_ALT, s.translate_alt_hotkey.clone()),
        (HK_CORRECT, s.correct_hotkey.clone()),
    ]
}

/// Runs the action behind a global hotkey. Callable from any thread.
pub fn on_hotkey(app: &AppHandle, id: u8) {
    let h = app.clone();
    match id {
        HK_MAIN => {
            let _ = app.run_on_main_thread(move || actions::show_palette(&h, true));
        }
        HK_SEQ => {
            std::thread::spawn(move || actions::sequential_paste(&h));
        }
        HK_PICKER => {
            let _ = app.run_on_main_thread(move || picker::show(&h));
        }
        HK_TRANSLATE | HK_TRANSLATE_ALT => {
            std::thread::spawn(move || translate::start(&h, id == HK_TRANSLATE_ALT));
        }
        HK_CORRECT => {
            std::thread::spawn(move || translate::start_correct(&h));
        }
        _ => {}
    }
}

fn normalize_combo(text: &str) -> String {
    let mut parts: Vec<String> = text.split('+').map(|p| p.trim().to_ascii_lowercase()).filter(|p| !p.is_empty()).collect();
    parts.sort();
    parts.join("+")
}

/// (Re-)registers the global hotkeys from the settings.
pub fn register_hotkeys(app: &AppHandle) -> HotkeyResult {
    let st = app.state::<AppState>();
    let (texts, lang) = {
        let s = st.settings.read();
        (hotkey_texts(&s), s.language.clone())
    };
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut errors = Vec::new();
    let mut hooked: Vec<platform::HookCombo> = Vec::new();
    let mut parse_and_register = |text: &str, id: u8| -> Result<Option<Shortcut>, ()> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        let label = hotkey_label(text, &lang);
        let t = |de: &str, en: &str, args: &[&str]| i18n::tr_lang(&lang, de, en, args);
        match text.parse::<Shortcut>() {
            Ok(sc) => match gs.register(sc) {
                Ok(()) => Ok(Some(sc)),
                Err(e) if e.to_string().to_lowercase().contains("already registered") && platform::parse_combo(text, id).is_some() => {
                    // Owned by Windows or another app (e.g. Win+V): take it over with our hook.
                    log::info!("hotkey {text} is reserved; handled by keyboard hook");
                    hooked.push(platform::parse_combo(text, id).expect("checked"));
                    Ok(None)
                }
                Err(e) => {
                    log::warn!("hotkey {text} registration failed: {e}");
                    let es = e.to_string();
                    errors.push(if es.to_lowercase().contains("already registered") {
                        t(
                            "„{0}“ ist schon belegt – von Windows oder einem anderen Programm. Bitte eine andere Kombination wählen.",
                            "“{0}” is already taken by Windows or another program. Please choose a different combination.",
                            &[&label],
                        )
                    } else {
                        t("„{0}“ ließ sich nicht einrichten: {1}", "“{0}” could not be registered: {1}", &[&label, &es])
                    });
                    Err(())
                }
            },
            Err(e) => {
                log::warn!("hotkey {text} invalid: {e}");
                errors.push(t("„{0}“ ist kein gültiges Tastenkürzel.", "“{0}” is not a valid shortcut.", &[&label]));
                Err(())
            }
        }
    };
    let mut registered = Vec::new();
    let mut failed = Vec::new();
    let mut dup_errors = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (id, text) in &texts {
        let norm = normalize_combo(text);
        if !norm.is_empty() && seen.contains(&norm) {
            // The same combination twice would silently end up in the hook; refuse instead.
            let label = hotkey_label(text, &lang);
            dup_errors.push(i18n::tr_lang(&lang, "„{0}“ ist schon für eine andere UltimatePaste-Funktion vergeben.", "“{0}” is already used for another UltimatePaste function.", &[&label]));
            failed.push(*id);
            continue;
        }
        seen.push(norm);
        match parse_and_register(text, *id) {
            Ok(Some(sc)) => registered.push((*id, sc)),
            Ok(None) => {}
            Err(()) => failed.push(*id),
        }
    }
    errors.extend(dup_errors);
    let handle = app.clone();
    platform::set_hook_combos(hooked, move |id| {
        // Runs inside the low-level keyboard hook: it must return at once. Anything that
        // waits for the main thread (e.g. asking a window for its handle) can exceed Windows'
        // hook timeout, and then the key goes on to Windows (emoji panel, clipboard history).
        let h = handle.clone();
        std::thread::spawn(move || {
            log::info!("reserved hotkey pressed (id {id})");
            on_hotkey(&h, id);
        });
    });
    *st.hotkeys.lock() = registered;
    HotkeyResult { errors, failed }
}

/// Window theme (affects Mica and native controls); None = follow Windows.
pub fn apply_theme(app: &AppHandle) {
    let theme = match app.state::<AppState>().settings.read().theme.as_str() {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    };
    for w in app.webview_windows().values() {
        let _ = w.set_theme(theme);
    }
}

pub fn apply_autostart(app: &AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let want = app.state::<AppState>().settings.read().autostart;
    let al = app.autolaunch();
    let is = al.is_enabled().unwrap_or(false);
    let exe = std::env::current_exe().unwrap_or_default();
    let registered = platform::autostart_command(&app.package_info().name);
    // An existing entry may point to another copy (an old portable exe, one on a network drive
    // that isn't there yet at sign-in, or a deleted file): the installed copy always takes over,
    // and an entry whose file is gone is replaced.
    let stale = registered.as_deref().is_some_and(|cmd| autostart_needs_update(cmd, &exe, updater::is_installed(&exe)));
    let r = match (want, is) {
        (true, false) => al.enable(),
        (true, true) if stale => {
            log::info!("autostart: entry pointed to {registered:?}, now {}", exe.display());
            al.enable()
        }
        (false, true) => al.disable(),
        _ => Ok(()),
    };
    if let Err(e) = r {
        log::warn!("autostart: {e}");
    }
}

/// The program path of a Run command line (`"C:\\x\\a.exe" --autostart` or unquoted).
fn run_target(cmd: &str) -> &str {
    let c = cmd.trim();
    if let Some(rest) = c.strip_prefix('"') {
        return rest.split('"').next().unwrap_or("");
    }
    match c.to_ascii_lowercase().find(".exe") {
        Some(i) => &c[..i + 4],
        None => c,
    }
}

/// Whether the autostart entry `cmd` should be rewritten to start `exe`.
fn autostart_needs_update(cmd: &str, exe: &std::path::Path, exe_installed: bool) -> bool {
    let target = run_target(cmd);
    let same = target.eq_ignore_ascii_case(&exe.to_string_lossy());
    if same {
        return false;
    }
    exe_installed || !std::path::Path::new(target).exists()
}

fn tr(app: &AppHandle, de: &str, en: &str) -> String {
    i18n::tr(app, de, en)
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let paused = app.state::<AppState>().settings.read().paused;
    let open = MenuItem::with_id(app, "open", tr(app, "Öffnen", "Open"), true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", tr(app, "Aufnahme pausieren", "Pause recording"), true, paused, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", tr(app, "Einstellungen…", "Settings…"), true, None::<&str>)?;
    let translate = MenuItem::with_id(app, "translate", tr(app, "Übersetzen…", "Translate…"), true, None::<&str>)?;
    let correct = MenuItem::with_id(app, "correct", tr(app, "Korrigieren…", "Correct…"), true, None::<&str>)?;
    let emoji = MenuItem::with_id(app, "emoji", tr(app, "Emoji, GIF & Sonderzeichen…", "Emoji, GIF & symbols…"), true, None::<&str>)?;
    let sep0 = PredefinedMenuItem::separator(app)?;
    let update_label = app.state::<AppState>().update.lock().as_ref().filter(|u| u.newer).map(|u| i18n::trf(app, "Update auf {0} verfügbar…", "Update to {0} available…", &[&u.latest]));
    let update_item = match &update_label {
        Some(l) => Some(MenuItem::with_id(app, "update", l, true, None::<&str>)?),
        None => None,
    };
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", tr(app, "Beenden", "Quit"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &translate, &correct, &emoji, &sep0, &pause, &settings, &sep, &quit])?;
    if let Some(u) = &update_item {
        menu.insert(u, 0)?;
        menu.insert(&PredefinedMenuItem::separator(app)?, 1)?;
    }

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
        return Ok(());
    }
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(format!("UltimatePaste {}", env!("CARGO_PKG_VERSION")))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => actions::show_palette(app, false),
            "pause" => {
                let paused = {
                    let st = app.state::<AppState>();
                    let mut s = st.settings.write();
                    s.paused = !s.paused;
                    let _ = s.save(&st.data_dir);
                    s.paused
                };
                let _ = app.emit("settings-changed", ());
                let _ = build_tray(app);
                log::info!("recording paused: {paused}");
            }
            "settings" => actions::open_settings(app),
            "update" => updater::open_window(app),
            // Opens the window empty, ready to type or paste into.
            "translate" => translate::open_with(app, String::new(), false),
            "correct" => translate::open_with(app, String::new(), true),
            // Wait until the tray menu is gone: the previous program is in front again and gets
            // the emoji (otherwise the menu closing would count as "another window in front").
            "emoji" => {
                let h = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || picker::show(&h2));
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                actions::show_palette(tray.app_handle(), false);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn spawn_retention(app: AppHandle) {
    std::thread::Builder::new()
        .name("retention".into())
        .spawn(move || loop {
            {
                let st = app.state::<AppState>();
                let cutoff = st.settings.read().retention_cutoff(model::now_ms());
                if let Some(cutoff) = cutoff {
                    let deleted = st.store.lock().delete_older_than(cutoff);
                    match deleted {
                        Ok(ids) if !ids.is_empty() => {
                            st.index.write().remove_many(&ids);
                            log::info!("retention removed {} entries", ids.len());
                            let _ = app.emit("history-changed", ());
                        }
                        Err(e) => log::warn!("retention failed: {e}"),
                        _ => {}
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(3600));
        })
        .expect("spawn retention");
}

pub fn run() {
    let state = match build_state() {
        Ok(s) => s,
        Err(e) => {
            log::error!("startup failed: {e:#}");
            eprintln!("UltimatePaste startup failed: {e:#}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            actions::show_palette(app, false);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let id = app.state::<AppState>().hotkeys.lock().iter().find(|(_, sc)| sc == shortcut).map(|(id, _)| *id);
                    if let Some(id) = id {
                        log::info!("registered hotkey pressed (id {id})");
                        on_hotkey(app, id);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .manage(state)
        .setup(|app| {
            let handle = app.handle().clone();
            build_tray(&handle)?;
            register_hotkeys(&handle);
            apply_theme(&handle);
            actions::apply_layout(&handle);
            apply_autostart(&handle);
            capture::start(handle.clone());
            spawn_retention(handle.clone());
            sensitive::spawn_sweeper(handle.clone());
            typing::refresh(&handle);
            picker::init_key_redirect(&handle);
            std::thread::spawn(updater::cleanup_old);
            updater::spawn_checker(handle.clone());
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || picker::prewarm(&h2));
                });
            }
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if platform::exe_on_network_drive() {
                        log::warn!("running from a network drive: hotkeys may react late");
                        translate::toast(
                            &h,
                            &i18n::tr(
                                &h,
                                "UltimatePaste läuft von einem Netzlaufwerk – Tastenkürzel können dadurch verzögert reagieren. Besser lokal ablegen.",
                                "UltimatePaste runs from a network drive – shortcuts may react late. Better keep it on a local drive.",
                            ),
                        );
                    }
                    // First start: the tour covers shortcuts (incl. Win+V); otherwise check conflicts.
                    if !h.state::<AppState>().settings.read().tour_done {
                        let h2 = h.clone();
                        let _ = h.run_on_main_thread(move || actions::open_tour(&h2));
                    } else {
                        std::thread::sleep(std::time::Duration::from_secs(4));
                        conflicts::notify_on_start(&h);
                    }
                });
            }
            capture::ocr_backfill(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(size) = event {
                // Only sizes the user picks while the window is visible count.
                if window.is_visible().unwrap_or(false) {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    actions::remember_size(window.app_handle(), window.label(), *size, scale);
                }
            }
            if !actions::POPUPS.contains(&window.label()) {
                return;
            }
            match event {
                WindowEvent::Focused(false) => {
                    let _ = window.hide();
                }
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::get_detail,
            commands::get_thumb,
            commands::paste_entry,
            commands::missing_files,
            commands::paste_text,
            commands::hide_palette,
            commands::toggle_pin,
            commands::rename_entry,
            commands::update_text,
            commands::delete_entry,
            commands::bulk_delete,
            commands::copy_ocr,
            commands::run_ocr,
            commands::export_inbox,
            commands::list_snippets,
            commands::save_snippet,
            commands::delete_snippet,
            commands::paste_snippet,
            commands::get_settings,
            commands::save_settings,
            commands::set_paused,
            commands::open_settings,
            commands::app_info,
            commands::ai_run,
            commands::ai_set_key,
            commands::ai_has_key,
            commands::picker_state,
            commands::picker_paste,
            commands::picker_favorite,
            commands::picker_forget,
            commands::gif_search,
            commands::gif_recent,
            commands::gif_paste,
            commands::translate_pending,
            commands::translate_run,
            commands::translate_apply,
            commands::translate_languages,
            commands::translate_usage,
            commands::toast_text,
            commands::detect_language,
            commands::improve_run,
            commands::hotkey_conflicts,
            commands::set_windows_v,
            commands::notice_conflicts,
            commands::open_link,
            commands::open_data,
            commands::open_tour,
            commands::finish_tour,
            commands::snippet_fields,
            commands::take_snippet_form,
            commands::fill_job,
            commands::fill_submit,
            commands::fill_cancel,
            commands::fill_fit,
            commands::clean_link,
            commands::open_external,
            commands::recent_apps,
            commands::stats_get,
            commands::version_info,
            commands::update_check,
            commands::update_info,
            commands::open_update,
            commands::update_install,
            commands::settings_export,
            commands::settings_inspect,
            commands::settings_import,
            commands::thist_list,
            commands::thist_save,
            commands::thist_clear,
            commands::stats_reset,
            commands::dismiss_conflicts,
            commands::translate_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running UltimatePaste");
}

#[cfg(test)]
mod autostart_tests {
    use super::*;

    #[test]
    fn parses_run_target() {
        assert_eq!(run_target(r#""C:\Apps\UltimatePaste.exe" --autostart"#), r"C:\Apps\UltimatePaste.exe");
        assert_eq!(run_target(r"C:\Apps\UltimatePaste.exe --autostart"), r"C:\Apps\UltimatePaste.exe");
    }

    #[test]
    fn stale_entries_are_replaced() {
        let exe = std::path::Path::new(r"C:\Users\x\AppData\Local\UltimatePaste\ultimatepaste.exe");
        let same = r#""C:\Users\X\AppData\Local\UltimatePaste\UltimatePaste.exe" --autostart"#;
        assert!(!autostart_needs_update(same, exe, true));
        // installed copy takes over from another copy
        assert!(autostart_needs_update(r#""Z:\old\UltimatePaste-0.13.6-portable.exe" --autostart"#, exe, true));
        // a portable copy replaces an entry whose file is gone
        assert!(autostart_needs_update(r#""Z:\does-not-exist\up.exe" --autostart"#, exe, false));
    }
}
