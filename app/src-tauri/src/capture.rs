//! Clipboard capture: reacts to WM_CLIPBOARDUPDATE, applies privacy rules, stores, indexes
//! and kicks off OCR for new images.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::model::now_ms;
use crate::platform::{self, Limits};
use crate::store::Upsert;
use crate::AppState;

const EXCLUDED_MEMORY: usize = 64;
const THUMB_MAX: u32 = 360;

pub fn start(app: AppHandle) {
    platform::spawn_clipboard_listener(move || on_change(&app));
}

fn on_change(app: &AppHandle) {
    let st = app.state::<AppState>();

    // Our own write in progress: wait until its sequence number is known.
    for _ in 0..30 {
        if !st.writing.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if platform::clipboard_seq() == st.ignore_seq.load(Ordering::Acquire) {
        return;
    }
    if (now_ms() as u64) < st.grab_until.load(Ordering::Acquire) {
        return; // copy we triggered ourselves to read the selection
    }

    let (paused, limits, ocr_enabled) = {
        let s = st.settings.read();
        (
            s.paused,
            Limits { max_text_bytes: s.max_text_mb as usize * 1024 * 1024, max_image_bytes: s.max_image_mb as usize * 1024 * 1024 },
            s.ocr_enabled,
        )
    };
    if paused {
        return;
    }

    let cap = match platform::read_clipboard(limits) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("read clipboard: {e:#}");
            return;
        }
    };
    let content = cap.content.normalize();
    if content.is_empty() {
        if cap.too_large {
            log::info!("clipboard content skipped: exceeds size limit");
        }
        return;
    }

    let blocked = st.settings.read().is_blocked(cap.source_exe.as_deref());
    let hash = st.store.lock().content_hash(&content);
    if cap.excluded || blocked {
        {
            let mut ex = st.excluded_hashes.lock();
            if !ex.contains(&hash) {
                ex.push_back(hash);
                if ex.len() > EXCLUDED_MEMORY {
                    ex.pop_front();
                }
            }
        }
        // Same content already in the history (e.g. captured before the app was blocked,
        // or the exclusion marker arrived in a second clipboard transaction): remove it.
        let existing = st.store.lock().find_by_hash(&hash).ok().flatten();
        if let Some(id) = existing {
            let removed = st.store.lock().delete(id);
            if removed.is_ok() {
                st.index.write().remove(id);
                let _ = app.emit("history-changed", ());
            }
        }
        log::info!("sensitive clipboard content ignored (source: {})", cap.source_exe.as_deref().unwrap_or("?"));
        return;
    }
    if st.excluded_hashes.lock().contains(&hash) {
        log::info!("previously excluded content re-appeared on the clipboard; ignored");
        return;
    }

    let (thumb, dims) = match &content.image_png {
        Some(png) => match platform::make_thumbnail(png, THUMB_MAX) {
            Ok((t, w, h)) => (Some(t), Some((w, h))),
            Err(e) => {
                log::warn!("thumbnail failed: {e}");
                (None, None)
            }
        },
        None => (None, None),
    };

    let result = st.store.lock().upsert(&content, cap.source_exe.as_deref(), thumb.as_deref(), dims);
    match result {
        Ok(Upsert::Inserted(id)) => {
            let summary = crate::model::EntrySummary {
                id,
                kind: content.kind(),
                title: None,
                preview: crate::model::make_preview(&content),
                source_app: cap.source_exe.clone(),
                created_at: now_ms(),
                last_used_at: now_ms(),
                pinned: false,
                formats: content.formats(),
                image_w: dims.map(|d| d.0),
                image_h: dims.map(|d| d.1),
                file_count: content.files.len(),
                byte_size: content.byte_size() as i64,
                has_ocr: false,
            };
            st.index.write().upsert(summary, &content, None);
            let _ = app.emit("history-changed", ());
            if let Some(text) = content.text.as_deref() {
                crate::sensitive::on_new_entry(app, id, text);
                // Copied link with trackers: put the cleaned link on the clipboard; it is recorded
                // as the newest entry, the original stays in the history below it.
                if st.settings.read().clean_links {
                    if let Some(clean) = crate::cleanlinks::clean(text) {
                        let _ = platform::write_clipboard(&crate::model::ClipContent::text_only(clean));
                    }
                }
            }
            if ocr_enabled {
                if let Some(png) = content.image_png {
                    spawn_ocr(app.clone(), id, png);
                }
            }
        }
        Ok(Upsert::Bumped(id)) => {
            let now = now_ms();
            st.index.write().update(id, |s| s.last_used_at = now);
            let _ = app.emit("history-changed", ());
        }
        Err(e) => log::error!("store entry: {e:#}"),
    }
}

static OCR_QUEUE: OnceLock<Sender<(i64, Vec<u8>)>> = OnceLock::new();

/// Queues an image for OCR. A single worker thread processes the queue, so copying many
/// screenshots in a row doesn't spawn many decoders at once.
pub fn spawn_ocr(app: AppHandle, id: i64, png: Vec<u8>) {
    let tx = OCR_QUEUE.get_or_init(|| {
        let (tx, rx) = channel::<(i64, Vec<u8>)>();
        std::thread::Builder::new()
            .name("ocr".into())
            .spawn(move || {
                for (id, png) in rx {
                    if let Err(e) = ocr_entry(&app, id, &png) {
                        log::info!("OCR for entry {id}: {e:#}");
                    }
                }
            })
            .expect("spawn OCR worker");
        tx
    });
    let _ = tx.send((id, png));
}

/// Runs OCR and updates store + index. Returns the recognized text.
pub fn ocr_entry(app: &AppHandle, id: i64, png: &[u8]) -> anyhow::Result<String> {
    let text = platform::ocr_png(png)?;
    let text = text.trim().to_string();
    let st = app.state::<AppState>();
    st.store.lock().set_ocr(id, &text)?;
    let content = st.store.lock().get_content(id)?;
    // Separate statement: an `if let` would keep the read guard alive into the block (deadlock).
    let summary = st.index.read().get(id).cloned();
    if let (Some(content), Some(mut summary)) = (content, summary) {
        summary.has_ocr = !text.is_empty();
        st.index.write().upsert(summary, &content, Some(&text));
    }
    let _ = app.emit("history-changed", ());
    Ok(text)
}

/// After startup: recognize images that have no OCR text yet (e.g. OCR was off before).
pub fn ocr_backfill(app: AppHandle) {
    std::thread::Builder::new()
        .name("ocr-backfill".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(20));
            let st = app.state::<AppState>();
            if !st.settings.read().ocr_enabled {
                return;
            }
            let ids: Vec<i64> = {
                let ix = st.index.read();
                ix.recent_ids().into_iter().filter(|id| ix.get(*id).is_some_and(|s| s.kind == crate::model::Kind::Image && !s.has_ocr)).take(200).collect()
            };
            for id in ids {
                let already = st.store.lock().get_ocr(id).ok().flatten().is_some();
                if already {
                    continue;
                }
                let png = st.store.lock().get_content(id).ok().flatten().and_then(|c| c.image_png);
                if let Some(png) = png {
                    if let Err(e) = ocr_entry(&app, id, &png) {
                        log::info!("OCR backfill stopped: {e:#}");
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        })
        .ok();
}
