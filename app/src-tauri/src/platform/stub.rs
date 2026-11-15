//! Non-Windows stand-ins. Only used for building/testing the core on Linux.
#![allow(dead_code)]

use super::{Captured, Limits, Rect};
use crate::model::ClipContent;
use anyhow::{bail, Result};

pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}
pub fn spawn_clipboard_listener<F: Fn() + Send + 'static>(_on_change: F) {}
pub fn clipboard_seq() -> u32 {
    0
}
pub fn read_clipboard(_limits: Limits) -> Result<Captured> {
    bail!("clipboard access is only implemented on Windows")
}
pub fn write_clipboard(_c: &ClipContent) -> Result<u32> {
    bail!("clipboard access is only implemented on Windows")
}
pub fn foreground_window() -> isize {
    0
}
pub fn set_foreground(_hwnd: isize) -> bool {
    false
}
pub fn send_paste() -> bool {
    false
}
pub fn is_valid_paste_target(_hwnd: isize) -> bool {
    false
}
pub fn wait_foreground(_hwnd: isize, _timeout_ms: u64) -> bool {
    false
}
pub fn send_left(_n: usize) {}
pub fn is_window_elevated_for_us(_hwnd: isize) -> bool {
    false
}
pub fn work_area_for_window(_hwnd: isize) -> Option<Rect> {
    None
}
pub fn work_area_at_cursor() -> Option<(Rect, i32, i32)> {
    None
}
pub fn ocr_png(_png: &[u8]) -> Result<String> {
    bail!("OCR is only implemented on Windows")
}
pub fn secret_get(_service: &str, _user: &str) -> Result<Option<String>> {
    Ok(None)
}
pub fn secret_set(_service: &str, _user: &str, _value: Option<&str>) -> Result<()> {
    bail!("credential storage is only implemented on Windows")
}
pub fn set_hook_combos<F: Fn(u8) + Send + Sync + 'static>(_combos: Vec<super::HookCombo>, _on_press: F) {}
pub fn work_area_at_point(_x: i32, _y: i32) -> Option<Rect> {
    None
}
pub fn caret_screen_pos(_hwnd: isize) -> Option<(i32, i32)> {
    None
}
pub fn send_copy() -> bool {
    false
}
pub fn show_no_activate(_hwnd: isize) {}

pub fn autostart_command(_name: &str) -> Option<String> {
    None
}

pub fn shell_disabled_hotkeys() -> String {
    String::new()
}
pub fn set_shell_disabled_hotkeys(_keys: &str) -> Result<()> {
    bail!("only on Windows")
}
pub fn system_language() -> String {
    "en".into()
}
pub fn open_url(_url: &str) -> Result<()> {
    bail!("only on Windows")
}
pub fn exe_of_window(_h: isize) -> Option<String> {
    None
}
pub fn set_typing<F: Fn(String, isize) + Send + Sync + 'static>(_enabled: bool, _keywords: Vec<String>, _on_match: F) {}
pub fn focused_is_password(_h: isize) -> bool {
    false
}
pub fn send_backspaces(_n: usize) {}
pub fn exe_on_network_drive() -> bool {
    false
}
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirKey {
    pub key: String,
    pub code: String,
    pub text: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}
pub fn init_key_redirect<F: Fn(RedirKey) + Send + Sync + 'static>(_f: F) {}
pub fn redirect_keys(_hwnd: isize, _target: isize) {}
pub fn set_no_activate(_h: isize) {}
pub fn hide_window(_h: isize) {}
pub fn is_window_visible(_h: isize) -> bool {
    false
}
