//! Tauri commands called from the UI. Errors are returned as strings for display.

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::actions::{self, Delivery};
use crate::model::{now_ms, truncate_chars, ClipContent, EntryDetail, EntrySummary, PasteMode, Snippet};
use crate::search::Tab;
use crate::settings::Settings;
use crate::{ai, capture, inbox, platform, snippets, AppState};

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

fn changed(app: &AppHandle) {
    let _ = app.emit("history-changed", ());
}

const DETAIL_TEXT_LIMIT: usize = 200_000;
const DETAIL_IMAGE_MAX: u32 = 1200;

#[tauri::command]
pub fn search(state: State<AppState>, query: String, tab: Tab, limit: Option<usize>) -> Vec<EntrySummary> {
    state.index.read().search(&query, tab, limit.unwrap_or(300))
}

#[tauri::command]
pub async fn get_detail(state: State<'_, AppState>, id: i64) -> CmdResult<EntryDetail> {
    let summary = state.index.read().get(id).cloned().ok_or("not found")?;
    let (content, ocr) = {
        let store = state.store.lock();
        (store.get_content(id).map_err(err)?.ok_or("not found")?, store.get_ocr(id).map_err(err)?)
    };
    let (text, text_truncated) = match &content.text {
        Some(t) if t.len() > DETAIL_TEXT_LIMIT => (Some(truncate_chars(t, DETAIL_TEXT_LIMIT)), true),
        Some(t) => (Some(t.clone()), false),
        None => (None, false),
    };
    let image = match &content.image_png {
        Some(png) => {
            let shown = platform::make_thumbnail(png, DETAIL_IMAGE_MAX).map(|t| t.0).unwrap_or_else(|_| png.clone());
            Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(shown)))
        }
        None => None,
    };
    let (_, missing_files) = crate::model::split_missing(&content.files, false);
    Ok(EntryDetail { summary, text, text_truncated, files: content.files, missing_files, image, ocr_text: ocr.filter(|o| !o.is_empty()) })
}

/// Small preview image (data URL) for list/grid views.
#[tauri::command]
pub async fn get_thumb(app: AppHandle, id: i64) -> CmdResult<Option<String>> {
    let state = app.state::<AppState>();
    let t = state.store.lock().get_thumb(id).map_err(err)?;
    Ok(t.map(|png| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteResult {
    delivery: Delivery,
    /// Names of copied files that no longer exist and were left out.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    missing: Vec<String>,
}

fn load(state: &AppState, id: i64) -> CmdResult<ClipContent> {
    state.store.lock().get_content(id).map_err(err)?.ok_or_else(|| "not found".to_string())
}

#[tauri::command]
pub async fn paste_entry(app: AppHandle, id: i64, mode: PasteMode) -> CmdResult<PasteResult> {
    let state = app.state::<AppState>();
    let mut content = load(&state, id)?;
    // Copied files are stored as paths only. Paste what still exists and report the rest;
    // "plain" pastes the paths as text, which always works.
    let mut missing = Vec::new();
    if !content.files.is_empty() && matches!(mode, PasteMode::Default | PasteMode::Copy) {
        let (present, gone) = crate::model::split_missing(&content.files, true);
        missing = gone.iter().map(|f| crate::model::file_name(f).to_string()).collect();
        if present.is_empty() {
            return Err(format!("files-missing:{}", missing.join(", ")));
        }
        content.files = present;
    }
    if state.settings.read().move_used_to_top {
        let now = state.store.lock().touch(id).map_err(err)?;
        state.index.write().update(id, |s| s.last_used_at = now);
    }
    let delivery = actions::deliver(&app, &content, mode, None).map_err(err)?;
    crate::stats::record(&app, crate::stats::Event::Paste);
    changed(&app);
    Ok(PasteResult { delivery, missing })
}

/// Ids of file entries (from `ids`) whose files are all gone. Network paths are skipped.
#[tauri::command]
pub async fn missing_files(state: State<'_, AppState>, ids: Vec<i64>) -> CmdResult<Vec<i64>> {
    let mut out = Vec::new();
    for id in ids.into_iter().take(300) {
        let content = state.store.lock().get_content(id).map_err(err)?;
        if let Some(c) = content {
            if !c.files.is_empty() && crate::model::split_missing(&c.files, false).0.is_empty() {
                out.push(id);
            }
        }
    }
    Ok(out)
}

/// Pastes arbitrary text (edited content, AI results). Optionally stores it in the history.
#[tauri::command]
pub async fn paste_text(app: AppHandle, text: String, save: bool, copy_only: Option<bool>) -> CmdResult<PasteResult> {
    let content = ClipContent::text_only(text);
    if save {
        store_text(&app, &content)?;
    }
    let mode = if copy_only.unwrap_or(false) { PasteMode::Copy } else { PasteMode::Default };
    let delivery = actions::deliver(&app, &content, mode, None).map_err(err)?;
    Ok(PasteResult { delivery, missing: Vec::new() })
}

pub(crate) fn store_text(app: &AppHandle, content: &ClipContent) -> CmdResult<i64> {
    let state = app.state::<AppState>();
    let id = state.store.lock().upsert(content, Some("ultimatepaste"), None, None).map_err(err)?.id();
    let row = state.store.lock().load_row(id).map_err(err)?;
    if let Some(r) = row {
        state.index.write().upsert(r.summary, &r.content, r.ocr.as_deref());
    }
    changed(app);
    Ok(id)
}

#[tauri::command]
pub fn hide_palette(app: AppHandle, window: tauri::WebviewWindow, restore_focus: Option<bool>) {
    if window.label() == crate::picker::LABEL {
        log::info!("picker: hide (from the picker, e.g. Esc)");
    }
    actions::hide_palette(&app, restore_focus.unwrap_or(true));
}

#[tauri::command]
pub async fn toggle_pin(app: AppHandle, id: i64) -> CmdResult<bool> {
    let state = app.state::<AppState>();
    let pinned = !state.index.read().get(id).map(|s| s.pinned).unwrap_or(false);
    state.store.lock().set_pinned(id, pinned).map_err(err)?;
    state.index.write().update(id, |s| s.pinned = pinned);
    changed(&app);
    Ok(pinned)
}

#[tauri::command]
pub async fn rename_entry(app: AppHandle, id: i64, title: Option<String>) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let title = title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    state.store.lock().rename(id, title.as_deref()).map_err(err)?;
    state.index.write().set_title(id, title);
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn update_text(app: AppHandle, id: i64, text: String) -> CmdResult<i64> {
    let state = app.state::<AppState>();
    let new_id = state.store.lock().update_text(id, &text).map_err(err)?;
    let row = state.store.lock().load_row(new_id).map_err(err)?;
    {
        let mut ix = state.index.write();
        ix.remove(id);
        if let Some(r) = row {
            ix.upsert(r.summary, &r.content, r.ocr.as_deref());
        }
    }
    changed(&app);
    Ok(new_id)
}

#[tauri::command]
pub async fn delete_entry(app: AppHandle, id: i64) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.store.lock().delete(id).map_err(err)?;
    state.index.write().remove(id);
    changed(&app);
    Ok(())
}

/// Deletes unpinned entries used within the last `minutes` (None = everything unpinned).
#[tauri::command]
pub async fn bulk_delete(app: AppHandle, minutes: Option<u32>) -> CmdResult<usize> {
    let state = app.state::<AppState>();
    let since = minutes.map_or(0, |m| now_ms() - m as i64 * 60_000);
    let ids = state.store.lock().delete_since(since).map_err(err)?;
    state.index.write().remove_many(&ids);
    changed(&app);
    Ok(ids.len())
}

#[tauri::command]
pub async fn run_ocr(app: AppHandle, id: i64) -> CmdResult<String> {
    let png = load(&app.state::<AppState>(), id)?.image_png.ok_or("no image")?;
    capture::ocr_entry(&app, id, &png).map_err(err)
}

/// Copies the recognized text of an image (running OCR first if needed).
#[tauri::command]
pub async fn copy_ocr(app: AppHandle, id: i64) -> CmdResult<String> {
    let state = app.state::<AppState>();
    let existing = state.store.lock().get_ocr(id).map_err(err)?.filter(|t| !t.is_empty());
    let text = match existing {
        Some(t) => t,
        None => {
            let png = load(&state, id)?.image_png.ok_or("no image")?;
            capture::ocr_entry(&app, id, &png).map_err(err)?
        }
    };
    if text.is_empty() {
        return Err("no text recognized".into());
    }
    // OCR text becomes a regular history entry and goes to the clipboard.
    let content = ClipContent::text_only(text.clone());
    store_text(&app, &content)?;
    actions::write_silently(&app, &content).map_err(err)?;
    Ok(text)
}

#[tauri::command]
pub async fn export_inbox(app: AppHandle, id: i64) -> CmdResult<Vec<String>> {
    let state = app.state::<AppState>();
    let (dir, tags) = {
        let s = state.settings.read();
        (s.inbox_dir.clone().filter(|d| !d.trim().is_empty()).ok_or("inbox folder not configured")?, s.inbox_tags.clone())
    };
    let summary = state.index.read().get(id).cloned().ok_or("not found")?;
    let content = load(&state, id)?;
    let created = chrono::DateTime::from_timestamp_millis(summary.created_at).unwrap_or_default().with_timezone(&chrono::Local);
    let meta = inbox::ExportMeta { title: summary.title.as_deref(), source_app: summary.source_app.as_deref(), created, tags: &tags };
    let paths = inbox::export(std::path::Path::new(&dir), &content, &meta).map_err(err)?;
    Ok(paths.into_iter().map(|p| p.display().to_string()).collect())
}

// ---------------------------------------------------------------- snippets

#[tauri::command]
pub async fn list_snippets(app: AppHandle, query: String) -> CmdResult<Vec<Snippet>> {
    let state = app.state::<AppState>();
    let all = state.store.lock().list_snippets().map_err(err)?;
    let atoms: Vec<String> = query.split_whitespace().map(crate::search::fold).collect();
    if atoms.is_empty() {
        return Ok(all);
    }
    let mut scored: Vec<(i32, Snippet)> = all
        .into_iter()
        .filter_map(|s| {
            let name = crate::search::fold(&s.name);
            let kw = crate::search::fold(&s.keyword);
            let body = crate::search::fold(&s.body);
            let mut score = 0;
            for a in &atoms {
                if kw == *a {
                    score += 300;
                } else if kw.starts_with(a.as_str()) {
                    score += 150;
                } else if name.contains(a.as_str()) {
                    score += 100;
                } else if body.contains(a.as_str()) {
                    score += 20;
                } else {
                    return None;
                }
            }
            Some((score, s))
        })
        .collect();
    scored.sort_by_key(|s| std::cmp::Reverse(s.0));
    Ok(scored.into_iter().map(|(_, s)| s).collect())
}

#[tauri::command]
pub async fn save_snippet(app: AppHandle, snippet: Snippet) -> CmdResult<i64> {
    let state = app.state::<AppState>();
    if snippet.name.trim().is_empty() {
        return Err("name required".into());
    }
    let r = state.store.lock().save_snippet(&snippet).map_err(err);
    crate::typing::refresh(&app);
    r
}

#[tauri::command]
pub async fn delete_snippet(app: AppHandle, id: i64) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let r = state.store.lock().delete_snippet(id).map_err(err);
    crate::typing::refresh(&app);
    r
}

/// Snippet id whose input form the palette should open (once).
#[tauri::command]
pub fn take_snippet_form(state: State<AppState>) -> Option<i64> {
    state.snippet_form.lock().take()
}

#[tauri::command]
pub fn snippet_fields(state: State<AppState>, id: i64) -> CmdResult<Vec<snippets::Field>> {
    let list = state.store.lock().list_snippets().map_err(err)?;
    Ok(list.into_iter().find(|s| s.id == id).map(|s| snippets::fields(&s.body)).unwrap_or_default())
}

#[tauri::command]
pub async fn paste_snippet(app: AppHandle, id: i64, copy_only: Option<bool>, values: Option<std::collections::HashMap<String, String>>) -> CmdResult<PasteResult> {
    let state = app.state::<AppState>();
    let snippet = state.store.lock().list_snippets().map_err(err)?.into_iter().find(|s| s.id == id).ok_or("not found")?;
    let lang = state.settings.read().language.clone();
    // {clipboard} = newest text entry in the history
    let clipboard = {
        let ix = state.index.read();
        ix.recent_ids().into_iter().find(|i| ix.get(*i).is_some_and(|s| matches!(s.kind, crate::model::Kind::Text | crate::model::Kind::Link)))
    }
    .and_then(|i| state.store.lock().get_content(i).ok().flatten())
    .and_then(|c| c.text);
    let ex = snippets::expand_with(&snippet.body, chrono::Local::now(), &lang, clipboard.as_deref(), &values.unwrap_or_default());
    let mode = if copy_only.unwrap_or(false) { PasteMode::Copy } else { PasteMode::Default };
    let delivery = actions::deliver(&app, &ClipContent::text_only(ex.text), mode, ex.cursor_back).map_err(err)?;
    Ok(PasteResult { delivery, missing: Vec::new() })
}

// ---------------------------------------------------------------- settings

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings.read().clone()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    hotkey_errors: Vec<String>,
    /// Effective hotkeys after saving (a hotkey that could not be registered is reverted).
    hotkey: String,
    sequential_hotkey: String,
    picker_hotkey: String,
    translate_hotkey: String,
    translate_alt_hotkey: String,
    correct_hotkey: String,
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> CmdResult<SaveResult> {
    let old = state.settings.read().clone();
    // Window sizes are owned by the backend (the UI holds a stale copy).
    let mut settings = settings;
    settings.window_sizes = old.window_sizes.clone();
    *state.settings.write() = settings.clone();
    let mut hotkey_errors = Vec::new();
    if crate::hotkey_texts(&old) != crate::hotkey_texts(&settings) {
        let r = crate::register_hotkeys(&app);
        hotkey_errors = r.errors;
        if !r.failed.is_empty() {
            // Keep the previous, working shortcut instead of leaving none active.
            {
                let mut s = state.settings.write();
                for id in &r.failed {
                    match *id {
                        crate::HK_MAIN => s.hotkey = old.hotkey.clone(),
                        crate::HK_SEQ => s.sequential_hotkey = old.sequential_hotkey.clone(),
                        crate::HK_PICKER => s.picker_hotkey = old.picker_hotkey.clone(),
                        crate::HK_TRANSLATE => s.translate_hotkey = old.translate_hotkey.clone(),
                        crate::HK_TRANSLATE_ALT => s.translate_alt_hotkey = old.translate_alt_hotkey.clone(),
                        _ => s.correct_hotkey = old.correct_hotkey.clone(),
                    }
                }
            }
            crate::register_hotkeys(&app);
        }
    }
    let settings = state.settings.read().clone();
    settings.save(&state.data_dir).map_err(err)?;
    if old.theme != settings.theme {
        crate::apply_theme(&app);
    }
    if old.layout != settings.layout {
        actions::apply_layout(&app);
    }
    if old.snippet_expand != settings.snippet_expand {
        crate::typing::refresh(&app);
    }
    if old.autostart != settings.autostart {
        crate::apply_autostart(&app);
    }
    if old.language != settings.language || old.paused != settings.paused {
        let _ = crate::build_tray(&app);
    }
    let _ = app.emit("settings-changed", ());
    Ok(SaveResult {
        hotkey_errors,
        hotkey: settings.hotkey,
        sequential_hotkey: settings.sequential_hotkey,
        picker_hotkey: settings.picker_hotkey,
        translate_hotkey: settings.translate_hotkey,
        translate_alt_hotkey: settings.translate_alt_hotkey,
        correct_hotkey: settings.correct_hotkey,
    })
}

#[tauri::command]
pub fn set_paused(app: AppHandle, state: State<AppState>, paused: bool) -> CmdResult<()> {
    {
        let mut s = state.settings.write();
        s.paused = paused;
        s.save(&state.data_dir).map_err(err)?;
    }
    let _ = crate::build_tray(&app);
    let _ = app.emit("settings-changed", ());
    Ok(())
}

/// Async on purpose: creating a webview window inside a synchronous command deadlocks on
/// Windows (the window stays white).
#[tauri::command]
pub async fn open_settings(app: AppHandle) {
    actions::open_settings(&app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
    entries: usize,
}

#[tauri::command]
pub fn app_info(state: State<AppState>) -> AppInfo {
    AppInfo { version: env!("CARGO_PKG_VERSION").into(), data_dir: state.data_dir.display().to_string(), entries: state.index.read().len() }
}

// ---------------------------------------------------------------- AI

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRunArgs {
    action_id: String,
    /// Entry to work on; or `text` directly.
    id: Option<i64>,
    text: Option<String>,
}

/// Runs an AI action and returns the result text (the UI then pastes it via `paste_text`).
#[tauri::command]
pub async fn ai_run(app: AppHandle, args: AiRunArgs) -> CmdResult<String> {
    let state = app.state::<AppState>();
    let ai_settings = state.settings.read().ai.clone();
    if !ai_settings.enabled {
        return Err("AI is disabled".into());
    }
    let action = ai_settings.actions.iter().find(|a| a.id == args.action_id).cloned().ok_or("unknown action")?;
    let provider_id = if action.provider.is_empty() { ai_settings.default_provider.clone() } else { action.provider.clone() };
    let provider = ai_settings.providers.iter().find(|p| p.id == provider_id).cloned().ok_or("no-provider")?;
    let text = match (args.text, args.id) {
        (Some(t), _) => t,
        (None, Some(id)) => {
            let c = load(&state, id)?;
            c.text.or_else(|| (!c.files.is_empty()).then(|| c.files.join("\n"))).ok_or("entry has no text")?
        }
        _ => return Err("nothing to send".into()),
    };
    let key = platform::secret_get(ai::KEYRING_SERVICE, &provider.id).map_err(err)?;
    ai::run(&provider, key.as_deref(), &action, &text).await.map_err(err)
}

#[tauri::command]
pub fn ai_set_key(provider_id: String, key: Option<String>) -> CmdResult<()> {
    platform::secret_set(ai::KEYRING_SERVICE, &provider_id, key.as_deref().map(str::trim)).map_err(err)
}

#[tauri::command]
pub fn ai_has_key(provider_id: String) -> bool {
    platform::secret_get(ai::KEYRING_SERVICE, &provider_id).ok().flatten().is_some_and(|k| !k.is_empty())
}

// ---------------------------------------------------------------- picker (emoji, symbols, GIFs)

#[tauri::command]
pub fn picker_state(state: State<AppState>) -> crate::picker::PickerState {
    state.picker.lock().clone()
}

/// Pastes emoji/symbols (not recorded in the history) and remembers them as recently used.
#[tauri::command]
pub async fn picker_paste(app: AppHandle, kind: String, items: Vec<String>, copy_only: Option<bool>) -> CmdResult<PasteResult> {
    let state = app.state::<AppState>();
    let max = state.settings.read().picker.recent_count.clamp(8, 64) as usize;
    {
        let mut p = state.picker.lock();
        let list = if kind == "symbol" { &mut p.symbol_recent } else { &mut p.emoji_recent };
        for it in items.iter().rev() {
            crate::picker::push_recent(list, it.clone(), max);
        }
        p.save(&state.data_dir).map_err(err)?;
    }
    let content = ClipContent::text_only(items.concat());
    let mode = if copy_only.unwrap_or(false) { PasteMode::Copy } else { PasteMode::Default };
    log::info!("picker: hide (paste)");
    let delivery = actions::deliver(&app, &content, mode, None).map_err(err)?;
    for it in &items {
        crate::stats::record(&app, if kind == "symbol" { crate::stats::Event::Symbol(it) } else { crate::stats::Event::Emoji(it) });
    }
    Ok(PasteResult { delivery, missing: Vec::new() })
}

#[tauri::command]
pub fn picker_favorite(state: State<AppState>, value: String, on: bool) -> CmdResult<crate::picker::PickerState> {
    let mut p = state.picker.lock();
    p.favorites.retain(|f| *f != value);
    if on {
        p.favorites.push(value);
    }
    p.save(&state.data_dir).map_err(err)?;
    Ok(p.clone())
}

struct GifSetup {
    key: String,
    rating: String,
    lang: String,
    customer_id: String,
    original: bool,
    feedback: bool,
}

async fn gif_setup(app: &AppHandle, provider: &str) -> CmdResult<GifSetup> {
    let state = app.state::<AppState>();
    let name = if provider == "klipy" { crate::gif::KEY_KLIPY } else { crate::gif::KEY_GIPHY };
    let key = platform::secret_get(ai::KEYRING_SERVICE, name).map_err(err)?.filter(|k| !k.is_empty()).ok_or("no-key")?;
    let (rating, lang, original, feedback) = {
        let s = state.settings.read();
        (s.picker.gif_rating.clone(), s.language.clone(), s.picker.gif_original_size, s.picker.gif_feedback)
    };
    let mut customer_id = {
        let p = state.picker.lock();
        if provider == "giphy" { p.giphy_random_id.clone() } else { p.customer_id.clone() }
    };
    if customer_id.is_empty() {
        // GIPHY wants its own anonymous id; fall back to ours if that call fails.
        customer_id = match crate::gif::giphy_random_id(&key).await {
            Ok(id) => {
                let mut p = state.picker.lock();
                p.giphy_random_id = id.clone();
                let _ = p.save(&state.data_dir);
                id
            }
            Err(_) => state.picker.lock().customer_id.clone(),
        };
    }
    Ok(GifSetup { key, rating, lang, customer_id, original, feedback })
}

impl GifSetup {
    fn ctx(&self) -> crate::gif::Ctx<'_> {
        crate::gif::Ctx { key: &self.key, rating: &self.rating, lang: &self.lang, customer_id: &self.customer_id, original: self.original }
    }
}

#[tauri::command]
pub async fn gif_search(app: AppHandle, query: crate::gif::Query) -> CmdResult<Vec<crate::gif::GifItem>> {
    let setup = gif_setup(&app, &query.provider).await?;
    let url = crate::gif::search_url(&query, &setup.ctx()).map_err(err)?;
    let body = crate::gif::get_json(&url).await.map_err(err)?;
    Ok(crate::gif::parse_items(&query.provider, query.sticker, &body, setup.original))
}

/// "Recently used" GIFs, loaded fresh from the services (only ids are stored).
#[tauri::command]
pub async fn gif_recent(app: AppHandle) -> CmdResult<Vec<crate::gif::GifItem>> {
    let refs = app.state::<AppState>().picker.lock().gif_recent.clone();
    let mut found: Vec<crate::gif::GifItem> = Vec::new();
    for (provider, sticker, ids) in crate::gif::by_provider(&refs) {
        let Ok(setup) = gif_setup(&app, &provider).await else { continue };
        let Ok(url) = crate::gif::items_url(&provider, sticker, &ids, &setup.ctx()) else { continue };
        match crate::gif::get_json(&url).await {
            Ok(body) => found.extend(crate::gif::parse_items(&provider, sticker, &body, setup.original)),
            Err(e) if e.to_string() == "rate-limited" => return Err("rate-limited".into()),
            Err(e) => log::info!("gif recent ({provider}): {e}"),
        }
    }
    // Keep the user's order.
    let mut out = Vec::new();
    for r in &refs {
        if let Some(i) = found.iter().position(|g| g.provider == r.provider && g.id == r.id) {
            let mut g = found.remove(i);
            g.sticker = r.sticker;
            out.push(g);
        }
    }
    Ok(out)
}

/// Downloads the GIF for a moment and puts it on the clipboard as file, HTML image and link.
/// The file is deleted again after two minutes (the services forbid keeping copies).
#[tauri::command]
pub async fn gif_paste(app: AppHandle, item: crate::gif::GifItem, query: String, copy_only: Option<bool>) -> CmdResult<PasteResult> {
    let setup = gif_setup(&app, &item.provider).await?;
    // The item comes from the page: only fetch from the GIF services' own hosts.
    if !crate::gif::trusted_url(&item.gif_url) || !(item.ping.is_empty() || crate::gif::trusted_url(&item.ping)) {
        return Err("untrusted gif url".into());
    }
    let bytes = crate::gif::download(&item.gif_url).await.map_err(err)?;
    let dir = std::env::temp_dir().join("UltimatePaste").join("gif");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let safe: String = item.id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(60).collect();
    let prov = if item.provider == "klipy" { "klipy" } else { "giphy" };
    let file = dir.join(format!("{prov}-{safe}.gif"));
    std::fs::write(&file, &bytes).map_err(err)?;
    let path = file.to_string_lossy().into_owned();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(120));
        let _ = std::fs::remove_file(&file);
    });
    let content = ClipContent { text: Some(item.gif_url.clone()), html: Some(crate::gif::html_for(&item)), files: vec![path], ..Default::default() };
    let state = app.state::<AppState>();
    {
        let max = state.settings.read().picker.recent_count.clamp(8, 64) as usize;
        let mut p = state.picker.lock();
        crate::picker::push_recent(&mut p.gif_recent, crate::picker::GifRef { provider: item.provider.clone(), id: item.id.clone(), sticker: item.sticker }, max);
        let _ = p.save(&state.data_dir);
    }
    if setup.feedback {
        let (key, cid, q, it) = (setup.key.clone(), setup.customer_id.clone(), query.clone(), item.clone());
        tauri::async_runtime::spawn(async move { crate::gif::report_sent(&it, &key, &cid, &q).await });
    }
    let mode = if copy_only.unwrap_or(false) { PasteMode::Copy } else { PasteMode::Default };
    log::info!("picker: hide (paste)");
    let delivery = actions::deliver(&app, &content, mode, None).map_err(err)?;
    crate::stats::record(&app, crate::stats::Event::Gif);
    Ok(PasteResult { delivery, missing: Vec::new() })
}

// ---------------------------------------------------------------- translation

#[tauri::command]
pub fn translate_pending(state: State<AppState>) -> Option<crate::translate::Pending> {
    state.translate_pending.lock().take()
}

#[tauri::command]
pub async fn translate_run(app: AppHandle, args: crate::translate::Args) -> CmdResult<crate::translate::Outcome> {
    crate::translate::run(&app, &args).await.map_err(err)
}

#[tauri::command]
pub fn thist_list(app: AppHandle) -> Vec<crate::thist::Session> {
    crate::thist::load(&app)
}

#[tauri::command]
pub fn thist_save(app: AppHandle, session: crate::thist::Session) {
    crate::thist::upsert(&app, session)
}

#[tauri::command]
pub fn thist_clear(app: AppHandle) {
    crate::thist::clear(&app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    report: crate::transfer::Report,
    save: SaveResult,
}

#[tauri::command]
pub async fn settings_export(app: AppHandle, path: String, password: Option<String>) -> CmdResult<crate::transfer::Report> {
    // PBKDF2 takes a moment: keep it off the async workers.
    tauri::async_runtime::spawn_blocking(move || crate::transfer::export(&app, std::path::Path::new(&path), password.as_deref().filter(|p| !p.is_empty())))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
pub async fn settings_inspect(path: String) -> CmdResult<crate::transfer::Info> {
    crate::transfer::inspect(std::path::Path::new(&path)).map_err(err)
}

#[tauri::command]
pub async fn settings_import(app: AppHandle, path: String, password: Option<String>, replace: bool) -> CmdResult<ImportResult> {
    let h = app.clone();
    let (report, settings) = tauri::async_runtime::spawn_blocking(move || crate::transfer::import(&h, std::path::Path::new(&path), password.as_deref(), replace))
        .await
        .map_err(err)?
        .map_err(err)?;
    let save = save_settings(app.clone(), app.state::<AppState>(), settings)?;
    crate::typing::refresh(&app);
    let _ = app.emit("history-changed", ());
    Ok(ImportResult { report, save })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    version: &'static str,
    build_date: &'static str,
    portable: bool,
}

#[tauri::command]
pub fn version_info() -> VersionInfo {
    let portable = std::env::current_exe().map(|e| !crate::updater::is_installed(&e)).unwrap_or(true);
    VersionInfo { version: crate::updater::VERSION, build_date: crate::updater::BUILD_DATE, portable }
}

/// Checks GitHub for a newer version (also remembers the result for the tray).
#[tauri::command]
pub async fn update_check(app: AppHandle) -> CmdResult<crate::updater::UpdateInfo> {
    let info = crate::updater::check().await.map_err(err)?;
    *app.state::<AppState>().update.lock() = Some(info.clone());
    let _ = crate::build_tray(&app);
    Ok(info)
}

/// The update found by the last check (for the update window).
#[tauri::command]
pub fn update_info(state: State<AppState>) -> Option<crate::updater::UpdateInfo> {
    state.update.lock().clone()
}

#[tauri::command]
pub async fn open_update(app: AppHandle) {
    crate::updater::open_window(&app);
}

/// Downloads, verifies and installs the update; the app restarts.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> CmdResult<()> {
    let info = app.state::<AppState>().update.lock().clone().ok_or("no update")?;
    crate::updater::install(&app, &info).await.map_err(err)
}

#[tauri::command]
pub fn stats_get(state: State<AppState>) -> crate::stats::Stats {
    crate::stats::Stats::load(&state.data_dir)
}

#[tauri::command]
pub fn stats_reset(app: AppHandle) {
    crate::stats::reset(&app)
}

/// Opens the data folder ("data") or the log file ("log") in Explorer / the editor.
#[tauri::command]
pub fn open_data(state: State<AppState>, which: String) -> CmdResult<()> {
    let path = if which == "log" { state.data_dir.join("ultimatepaste.log") } else { state.data_dir.clone() };
    platform::open_url(&path.display().to_string()).map_err(err)
}

/// Opens a help page of a service in the browser (only known sites).
#[tauri::command]
pub fn open_link(url: String) -> CmdResult<()> {
    const ALLOWED: &[&str] = &[
        "https://www.deepl.com",
        "https://developers.giphy.com",
        "https://partner.klipy.com",
        "https://platform.claude.com",
        "https://platform.openai.com",
        "https://ollama.com",
        "https://github.com/TheBigS1981/UltimatePaste",
        "https://emojibase.dev",
        "https://github.com/lipis/flag-icons",
    ];
    // Exact site or a path below it ("https://emojibase.dev.evil.com" must not pass).
    let ok = ALLOWED.iter().any(|a| url == *a || url.strip_prefix(a).is_some_and(|rest| rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#')));
    if !ok || !safe_url_chars(&url) {
        return Err("link not allowed".into());
    }
    platform::open_url(&url).map_err(err)
}

/// Link without tracking parameters (None = nothing to remove).
#[tauri::command]
pub fn clean_link(text: String) -> Option<String> {
    crate::cleanlinks::clean(&text)
}

/// Opens a link the user picked in a quick action (web, mail, phone only).
#[tauri::command]
pub fn open_external(url: String) -> CmdResult<()> {
    let u = url.trim();
    let scheme_ok = u.starts_with("https://") || u.starts_with("http://") || u.starts_with("tel:") || (u.starts_with("mailto:") && !u.contains('?'));
    if !scheme_ok || !safe_url_chars(u) {
        return Err("link not allowed".into());
    }
    platform::open_url(u).map_err(err)
}

/// No whitespace, quotes or shell-ish characters: the URL goes to whatever program is
/// registered for the scheme, and must not be able to smuggle in extra arguments.
fn safe_url_chars(u: &str) -> bool {
    !u.chars().any(|c| c.is_control() || c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`' | '^' | '|' | '\\'))
}

/// Programs the user pasted into recently (for the rule picker).
#[tauri::command]
pub fn recent_apps(state: State<AppState>) -> Vec<String> {
    let s = crate::stats::Stats::load(&state.data_dir);
    let mut v: Vec<(String, u64)> = s.apps.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    v.into_iter().map(|(a, _)| a).take(20).collect()
}

#[tauri::command]
pub async fn open_tour(app: AppHandle) {
    actions::open_tour(&app);
}

/// Marks the tour as done (finished or skipped).
#[tauri::command]
pub fn finish_tour(state: State<AppState>) -> CmdResult<()> {
    let mut s = state.settings.write();
    s.tour_done = true;
    s.save(&state.data_dir).map_err(err)
}

/// Shortcuts shared with Windows or PowerToys.
#[tauri::command]
pub fn hotkey_conflicts(app: AppHandle) -> Vec<crate::conflicts::Conflict> {
    crate::conflicts::find(&app)
}

/// Conflicts for the start-up notice window.
#[tauri::command]
pub fn notice_conflicts(app: AppHandle) -> Vec<crate::conflicts::Conflict> {
    crate::conflicts::open_notices(&app)
}

#[tauri::command]
pub fn dismiss_conflicts(app: AppHandle, keys: Vec<String>) {
    crate::conflicts::dismiss(&app, keys)
}

/// Switches Windows' own Win+V off (`off = true`) or back on; restarts Explorer.
#[tauri::command]
pub async fn set_windows_v(app: AppHandle, off: bool) -> CmdResult<Vec<crate::conflicts::Conflict>> {
    let h = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::conflicts::set_windows_v(&h, off)).await.map_err(err)?.map_err(err)?;
    Ok(crate::conflicts::find(&app))
}

/// Corrects or improves (rephrases) a text in its own language.
#[tauri::command]
pub async fn improve_run(app: AppHandle, args: crate::translate::ImproveArgs) -> CmdResult<crate::translate::Outcome> {
    crate::translate::improve(&app, &args).await.map_err(err)
}

/// Pastes the translation into the window that had the selection (replacing it) and records it.
#[tauri::command]
pub async fn translate_apply(app: AppHandle, text: String, copy_only: Option<bool>, kind: Option<String>, pair: Option<String>, chars: Option<u64>) -> CmdResult<PasteResult> {
    if let Some(w) = app.get_webview_window(crate::translate::LABEL) {
        let _ = w.hide();
    }
    let content = ClipContent::text_only(text);
    store_text(&app, &content)?;
    crate::thist::touch_latest(&app);
    let mode = if copy_only.unwrap_or(false) { PasteMode::Copy } else { PasteMode::Default };
    let delivery = actions::deliver(&app, &content, mode, None).map_err(err)?;
    crate::stats::record(
        &app,
        if kind.as_deref() == Some("correct") {
            crate::stats::Event::Correct
        } else {
            crate::stats::Event::Translate { pair: pair.unwrap_or_default(), chars: chars.unwrap_or(0) }
        },
    );
    Ok(PasteResult { delivery, missing: Vec::new() })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    code: &'static str,
    de: &'static str,
    en: &'static str,
}

#[tauri::command]
pub fn translate_languages() -> Vec<Language> {
    crate::translate::LANGUAGES.iter().map(|(code, de, en)| Language { code, de, en }).collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageInfo {
    month: crate::translate::MonthUsage,
    deepl_used: Option<u64>,
    deepl_limit: Option<u64>,
    deepl_error: Option<String>,
}

#[tauri::command]
pub async fn translate_usage(app: AppHandle) -> UsageInfo {
    let month = crate::translate::load_usage(&app);
    let key = platform::secret_get(ai::KEYRING_SERVICE, crate::translate::KEY_DEEPL).ok().flatten().filter(|k| !k.is_empty());
    let (deepl_used, deepl_limit, deepl_error) = match key {
        Some(k) => match crate::translate::deepl_usage(&k).await {
            Ok((u, l)) => (Some(u), Some(l), None),
            Err(e) => (None, None, Some(e.to_string())),
        },
        None => (None, None, None),
    };
    UsageInfo { month, deepl_used, deepl_limit, deepl_error }
}

#[tauri::command]
pub fn toast_text(state: State<AppState>) -> String {
    state.toast_text.lock().clone()
}

/// Detected language of a text (DeepL code like "DE"), for the translate window.
#[tauri::command]
pub fn detect_language(text: String) -> Option<String> {
    crate::translate::detect(&text)
}

/// "Translate" / "Correct" actions in the palette: open the window with the entry's text.
#[tauri::command]
pub async fn translate_entry(app: AppHandle, id: i64, mode: Option<String>) -> CmdResult<()> {
    let c = load(&app.state::<AppState>(), id)?;
    let text = c.text.ok_or("entry has no text")?;
    crate::translate::open_with(&app, text, mode.as_deref() == Some("correct"));
    Ok(())
}
