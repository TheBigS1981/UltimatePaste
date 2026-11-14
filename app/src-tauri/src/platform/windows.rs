//! Windows implementation: clipboard (via clipboard-win), focus & keystrokes, DPAPI,
//! elevation checks, monitor geometry, Windows.Media.Ocr and the Credential Manager.

use super::{bmp_to_png, png_to_bmp, Captured, HookCombo, Limits, Rect};
use crate::model::ClipContent;
use anyhow::{anyhow, Context, Result};
use clipboard_win::options::NoClear;
use clipboard_win::{formats, raw, Clipboard};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering as AtomicOrdering};
use std::sync::{Mutex as StdMutex, OnceLock};
use std::time::Duration;

use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE, HLOCAL, HWND, LocalFree, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::DataExchange::{GetClipboardOwner, GetClipboardSequenceNumber};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_V, VK_C,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetClassNameW, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GUITHREADINFO, GetMessageW, GetWindowThreadProcessId, KillTimer, PostThreadMessageW, SetTimer, SetWindowsHookExW, WM_TIMER,
    UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOWNOACTIVATE,
};

// ---------------------------------------------------------------- DPAPI

pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(&input, w!("UltimatePaste"), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).context("CryptProtectData")?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut c_void)));
        Ok(v)
    }
}

pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).context("CryptUnprotectData")?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut c_void)));
        Ok(v)
    }
}

// ---------------------------------------------------------------- processes

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut c_void)
}

fn pid_of_window(h: HWND) -> Option<u32> {
    if h.0.is_null() {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

fn exe_name_of_pid(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(|s| s.to_lowercase())
    }
}

fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut elev = TOKEN_ELEVATION::default();
        let mut ret = 0u32;
        let r = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elev as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        );
        let _ = CloseHandle(token);
        r.ok()?;
        Some(elev.TokenIsElevated != 0)
    }
}

fn self_elevated() -> bool {
    token_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false)
}

/// True if Windows (UIPI) will block our synthetic keystrokes to that window,
/// i.e. the target runs elevated and we do not.
pub fn is_window_elevated_for_us(h: isize) -> bool {
    if self_elevated() {
        return false;
    }
    let Some(pid) = pid_of_window(hwnd(h)) else { return false };
    unsafe {
        let Ok(proc_) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        // An unelevated process cannot open the token of an elevated one → treat as elevated.
        let r = token_elevated(proc_).unwrap_or(true);
        let _ = CloseHandle(proc_);
        r
    }
}

// ---------------------------------------------------------------- clipboard

pub fn clipboard_seq() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Starts a background thread with a message-only window that receives WM_CLIPBOARDUPDATE.
pub fn spawn_clipboard_listener<F: Fn() + Send + 'static>(on_change: F) {
    std::thread::Builder::new()
        .name("clipboard-listener".into())
        .spawn(move || {
            let mut monitor = match clipboard_win::Monitor::new() {
                Ok(m) => m,
                Err(e) => {
                    log::error!("clipboard monitor failed: {e}");
                    return;
                }
            };
            loop {
                match monitor.recv() {
                    Ok(true) => on_change(),
                    Ok(false) => break,
                    Err(e) => {
                        log::error!("clipboard monitor error: {e}");
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }
        })
        .expect("spawn clipboard listener");
}

fn fmt(name: &str) -> Option<u32> {
    raw::register_format(name).map(|f| f.get())
}

fn owner_exe() -> Option<String> {
    let owner = unsafe { GetClipboardOwner() }.ok()?;
    if !unsafe { IsWindow(Some(owner)) }.as_bool() {
        return None;
    }
    exe_name_of_pid(pid_of_window(owner)?)
}

/// Program (exe name) that owns a window.
pub fn exe_of_window(h: isize) -> Option<String> {
    exe_name_of_pid(pid_of_window(hwnd(h))?)
}

pub fn foreground_exe() -> Option<String> {
    exe_name_of_pid(pid_of_window(unsafe { GetForegroundWindow() })?)
}

/// Other listeners (Windows clipboard history, Office, RDP, …) open the clipboard right after
/// every change, so retry with backoff for roughly half a second.
fn open_clipboard() -> Result<Clipboard> {
    let mut last = String::new();
    for i in 0..10u64 {
        match Clipboard::new_attempts(10) {
            Ok(c) => return Ok(c),
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(10 + i * 8));
    }
    Err(anyhow!("open clipboard: {last}"))
}

fn sensitive_marker_present() -> bool {
    for name in ["ExcludeClipboardContentFromMonitorProcessing", "Clipboard Viewer Ignore"] {
        if fmt(name).is_some_and(raw::is_format_avail) {
            return true;
        }
    }
    // CanIncludeInClipboardHistory = 0 is the privacy signal (cloud-upload flags alone are not).
    if let Some(f) = fmt("CanIncludeInClipboardHistory").filter(|f| raw::is_format_avail(*f)) {
        let mut v = Vec::new();
        if raw::get_vec(f, &mut v).is_ok() && v.len() >= 4 && u32::from_le_bytes([v[0], v[1], v[2], v[3]]) == 0 {
            return true;
        }
    }
    false
}

fn trim_nul(v: &[u8]) -> String {
    String::from_utf8_lossy(v).trim_end_matches('\0').to_string()
}

/// Reads the current clipboard. Owner and content are both read while we hold the clipboard
/// open, so they always belong to the same clipboard state.
pub fn read_clipboard(limits: Limits) -> Result<Captured> {
    let mut cap = Captured::default();
    let mut bitmap: Option<Vec<u8>> = None;
    {
        let _clip = open_clipboard()?;
        cap.source_exe = owner_exe().or_else(foreground_exe);
        // Content is still read when excluded: the caller only keeps its hash in memory, so the
        // same secret can't sneak in later via apps that restore the previous clipboard.
        cap.excluded = sensitive_marker_present();
        let c = &mut cap.content;

        if raw::is_format_avail(formats::CF_HDROP) {
            let mut files = Vec::new();
            if raw::get_file_list(&mut files).is_ok() {
                c.files = files;
            }
        }

        if raw::is_format_avail(formats::CF_UNICODETEXT) {
            // CF_UNICODETEXT is UTF-16 → roughly half the bytes in UTF-8 for ASCII.
            let size = raw::size(formats::CF_UNICODETEXT).map_or(0, |s| s.get());
            if size / 2 > limits.max_text_bytes {
                cap.too_large = true;
            } else {
                let mut bytes = Vec::new();
                if raw::get_string(&mut bytes).is_ok() {
                    let s = trim_nul(&bytes);
                    if !s.is_empty() {
                        c.text = Some(s);
                    }
                }
            }
        }

        if c.text.is_some() {
            // Keep the complete CF_HTML (header + <head><style>), so Excel/Word formatting survives.
            if let Some(f) = fmt("HTML Format").filter(|f| raw::is_format_avail(*f)) {
                if raw::size(f).map_or(0, |s| s.get()) <= limits.max_text_bytes {
                    let mut v = Vec::new();
                    if raw::get_vec(f, &mut v).is_ok() {
                        c.html = Some(trim_nul(&v)).filter(|h| h.contains("StartFragment"));
                    }
                }
            }
            if let Some(f) = fmt("Rich Text Format").filter(|f| raw::is_format_avail(*f)) {
                if raw::size(f).map_or(0, |s| s.get()) <= limits.max_text_bytes {
                    let mut v = Vec::new();
                    if raw::get_vec(f, &mut v).is_ok() {
                        c.rtf = Some(trim_nul(&v)).filter(|s| !s.is_empty());
                    }
                }
            }
        }

        if c.text.is_none() && c.files.is_empty() {
            if let Some(f) = fmt("PNG").filter(|f| raw::is_format_avail(*f)) {
                if raw::size(f).map_or(0, |s| s.get()) > limits.max_image_bytes {
                    cap.too_large = true;
                } else {
                    let mut v = Vec::new();
                    if raw::get_vec(f, &mut v).is_ok() && v.starts_with(b"\x89PNG") {
                        c.image_png = Some(v);
                    }
                }
            }
            if c.image_png.is_none()
                && !cap.too_large
                && (raw::is_format_avail(formats::CF_DIB) || raw::is_format_avail(formats::CF_DIBV5) || raw::is_format_avail(formats::CF_BITMAP))
            {
                let size = raw::size(formats::CF_DIB).map_or(0, |s| s.get());
                if size > limits.max_image_bytes {
                    cap.too_large = true;
                } else {
                    let mut v = Vec::new();
                    if raw::get_bitmap(&mut v).is_ok() {
                        bitmap = Some(v);
                    }
                }
            }
        }
    }
    // Convert outside the clipboard lock.
    if let Some(bmp) = bitmap {
        match bmp_to_png(&bmp) {
            Ok((png, _, _)) if png.len() <= limits.max_image_bytes => cap.content.image_png = Some(png),
            Ok(_) => cap.too_large = true,
            Err(e) => log::warn!("bitmap conversion failed: {e}"),
        }
    }
    Ok(cap)
}

/// Writes all formats and returns the clipboard sequence number right after this write.
/// (It must be read after CloseClipboard: Windows bumps the number again when closing.)
pub fn write_clipboard(c: &ClipContent) -> Result<u32> {
    // Prepare conversions before taking the clipboard lock.
    let bmp = match &c.image_png {
        Some(png) => Some(png_to_bmp(png)?),
        None => None,
    };
    let clip = open_clipboard()?;
    raw::empty().map_err(|e| anyhow!("empty clipboard: {e}"))?;
    if let Some(t) = &c.text {
        raw::set_string_with(t, NoClear).map_err(|e| anyhow!("set text: {e}"))?;
    }
    if let Some(h) = &c.html {
        if let Some(f) = fmt("HTML Format") {
            if h.starts_with("Version:") {
                // Complete CF_HTML as captured: write back verbatim.
                let mut b = h.as_bytes().to_vec();
                b.push(0);
                let _ = raw::set_without_clear(f, &b);
            } else {
                let _ = raw::set_html_with(f, h, NoClear);
            }
        }
    }
    if let Some(r) = &c.rtf {
        if let Some(f) = fmt("Rich Text Format") {
            let mut b = r.as_bytes().to_vec();
            b.push(0);
            let _ = raw::set_without_clear(f, &b);
        }
    }
    if !c.files.is_empty() {
        raw::set_file_list_with(&c.files, NoClear).map_err(|e| anyhow!("set files: {e}"))?;
    }
    if let (Some(png), Some(bmp)) = (&c.image_png, &bmp) {
        // CF_DIB = BMP file without its 14-byte BITMAPFILEHEADER. Windows synthesizes CF_BITMAP/CF_DIBV5.
        if bmp.len() > 14 {
            raw::set_without_clear(formats::CF_DIB, &bmp[14..]).map_err(|e| anyhow!("set bitmap: {e}"))?;
        }
        if let Some(f) = fmt("PNG") {
            let _ = raw::set_without_clear(f, png);
        }
    }
    if c.private {
        // Windows clipboard history, cloud sync and other clipboard tools skip this content.
        if let Some(f) = fmt("ExcludeClipboardContentFromMonitorProcessing") {
            let _ = raw::set_without_clear(f, &[0u8]);
        }
        for name in ["CanIncludeInClipboardHistory", "CanUploadToCloudClipboard"] {
            if let Some(f) = fmt(name) {
                let _ = raw::set_without_clear(f, &0u32.to_le_bytes());
            }
        }
    }
    drop(clip);
    Ok(clipboard_seq())
}

// ---------------------------------------------------------------- focus & keystrokes

pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow() }.0 as isize
}

/// Windows that must never receive our Ctrl+V: our own windows, the taskbar and tray overflow.
pub fn is_valid_paste_target(h: isize) -> bool {
    let w = hwnd(h);
    if w.0.is_null() || !unsafe { IsWindow(Some(w)) }.as_bool() {
        return false;
    }
    if pid_of_window(w) == Some(unsafe { GetCurrentProcessId() }) {
        return false;
    }
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(w, &mut buf) }.max(0) as usize;
    let class = String::from_utf16_lossy(&buf[..n]);
    !matches!(
        class.as_str(),
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "NotifyIconOverflowWindow" | "TopLevelWindowForOverflowXamlIsland" | "XamlExplorerHostIslandWindow"
    )
}

pub fn set_foreground(h: isize) -> bool {
    let h = hwnd(h);
    if h.0.is_null() || !unsafe { IsWindow(Some(h)) }.as_bool() {
        return false;
    }
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h).as_bool()
    }
}

/// Polls until `h` is the foreground window (or the timeout expires).
pub fn wait_foreground(h: isize, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    loop {
        if foreground_window() == h {
            return true;
        }
        if start.elapsed() >= Duration::from_millis(timeout_ms) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

const VK_NEUTRAL: VIRTUAL_KEY = VIRTUAL_KEY(0xE8); // unassigned; suppresses Alt-menu / Start-menu activation

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if matches!(vk, VK_RMENU | VK_RCONTROL | VK_LEFT | VK_LWIN | VK_RWIN) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    // Scan codes matter for RDP/Citrix/VM consoles and raw-input apps.
    let scan: u16 = if vk == VK_NEUTRAL { 0 } else { (unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) }) as u16 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn is_down(vk: VIRTUAL_KEY) -> bool {
    (unsafe { GetAsyncKeyState(vk.0 as i32) } as u16 & 0x8000) != 0
}

fn send(inputs: &[INPUT]) -> bool {
    if inputs.is_empty() {
        return true;
    }
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) } as usize;
    if sent < inputs.len() {
        log::warn!("SendInput delivered {sent}/{} events (blocked by UIPI or secure desktop?)", inputs.len());
    }
    sent == inputs.len()
}

/// Sends Ctrl+V. Modifiers still held by the user (e.g. from the sequential-paste hotkey) are
/// released for the paste and pressed again afterwards, so holding the modifiers and tapping V
/// repeatedly keeps triggering the hotkey.
pub fn send_paste() -> bool {
    send_ctrl_key(VK_V)
}

/// Sends Ctrl+C (copies the selection of the foreground app). Same modifier handling as paste.
pub fn send_copy() -> bool {
    send_ctrl_key(VK_C)
}

fn send_ctrl_key(letter: VIRTUAL_KEY) -> bool {
    let held: Vec<VIRTUAL_KEY> = [VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN].into_iter().filter(|vk| is_down(*vk)).collect();
    let alt_or_win = held.iter().any(|vk| matches!(*vk, VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN));
    let mut inputs = Vec::new();
    if alt_or_win {
        inputs.push(key(VK_NEUTRAL, false));
        inputs.push(key(VK_NEUTRAL, true));
    }
    for vk in &held {
        inputs.push(key(*vk, true));
    }
    let ctrl_held = is_down(VK_CONTROL);
    if !ctrl_held {
        inputs.push(key(VK_LCONTROL, false));
    }
    inputs.push(key(letter, false));
    inputs.push(key(letter, true));
    if !ctrl_held {
        inputs.push(key(VK_LCONTROL, true));
    }
    for vk in &held {
        inputs.push(key(*vk, false));
    }
    if alt_or_win {
        // The user's later physical Alt/Win release must not open the menu bar / Start menu.
        inputs.push(key(VK_NEUTRAL, false));
        inputs.push(key(VK_NEUTRAL, true));
    }
    send(&inputs)
}

/// Moves the caret left n times (for the `{cursor}` snippet placeholder).
pub fn send_left(n: usize) {
    let n = n.min(5000);
    let mut inputs = Vec::with_capacity(n * 2);
    for _ in 0..n {
        inputs.push(key(VK_LEFT, false));
        inputs.push(key(VK_LEFT, true));
    }
    send(&inputs);
}

// ---------------------------------------------------------------- reserved-hotkey takeover

static HOOK_CB: OnceLock<Box<dyn Fn(u8) + Send + Sync>> = OnceLock::new();
static HOOK_COMBOS: StdMutex<Vec<HookCombo>> = StdMutex::new(Vec::new());
static HOOK_THREAD: StdMutex<Option<u32>> = StdMutex::new(None);

// Modifier state tracked from the hook's own event stream. GetAsyncKeyState is not reliable
// inside a low-level hook (notably for the Win key), so we follow key-down/up ourselves.
static MOD_LWIN: AtomicBool = AtomicBool::new(false);
static MOD_RWIN: AtomicBool = AtomicBool::new(false);
static MOD_CTRL_L: AtomicBool = AtomicBool::new(false);
static MOD_CTRL_R: AtomicBool = AtomicBool::new(false);
static MOD_ALT_L: AtomicBool = AtomicBool::new(false);
static MOD_ALT_R: AtomicBool = AtomicBool::new(false);
static MOD_SHIFT_L: AtomicBool = AtomicBool::new(false);
static MOD_SHIFT_R: AtomicBool = AtomicBool::new(false);
/// vk of a swallowed key-down, so its key-up is swallowed too.
static SWALLOWED_VK: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------- typing expansion (Q28)
//
// While enabled, the hook keeps the last few typed characters in memory (never stored or
// sent anywhere) and reports when they end with a snippet keyword like ";mfg".

static TYPING: AtomicBool = AtomicBool::new(false);
static TYPED: StdMutex<String> = StdMutex::new(String::new());
static TYPED_FG: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static KEYWORDS: StdMutex<Vec<String>> = StdMutex::new(Vec::new());
static TYPING_CB: OnceLock<Box<dyn Fn(String, isize) + Send + Sync>> = OnceLock::new();
const TYPED_MAX: usize = 40;

/// Turns typing expansion on/off. `keywords` are lower-case and include the trigger (";mfg").
pub fn set_typing<F: Fn(String, isize) + Send + Sync + 'static>(enabled: bool, keywords: Vec<String>, on_match: F) {
    let _ = TYPING_CB.set(Box::new(on_match));
    *KEYWORDS.lock().unwrap_or_else(|e| e.into_inner()) = keywords;
    TYPED.lock().unwrap_or_else(|e| e.into_inner()).clear();
    TYPING.store(enabled, AtomicOrdering::Relaxed);
    apply_hook_state();
}

/// Character(s) a key produces in the keyboard layout of window `fg` (empty for none).
fn key_text(fg: HWND, vk: u32, scan: u32, shift: bool, altgr: bool) -> String {
    let mut state = [0u8; 256];
    if shift {
        state[0x10] = 0x80;
    }
    if altgr {
        state[0x11] = 0x80;
        state[0x12] = 0x80;
    }
    if unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x14) } & 1 != 0 {
        state[0x14] = 0x01;
    }
    let tid = unsafe { GetWindowThreadProcessId(fg, None) };
    let hkl = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout(tid) };
    let mut out = [0u16; 8];
    // Flag 4: don't change the keyboard state (keeps dead keys like ^ working for the user).
    let n = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::ToUnicodeEx(vk, scan, &state, &mut out, 4, Some(hkl)) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&out[..n as usize]).chars().filter(|c| !c.is_control()).collect()
}

// ---------------------------------------------------------------- key redirect (emoji picker)
//
// The picker is shown without taking the focus (WS_EX_NOACTIVATE): Windows' own Win+. only
// leaks through when one of our windows is in front, and the program the user types in stays
// in front this way. While the picker is visible, the hook hands key presses to it instead.

/// A key press for the non-activated picker.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirKey {
    /// DOM-style key name ("ArrowDown", "Enter", "a", …) or "__blur" (another window came to the front)
    pub key: String,
    /// DOM-style code for digits ("Digit1") so Alt+1… works
    pub code: String,
    pub text: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

static REDIRECT_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static REDIRECT_TARGET: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static REDIRECT_CB: OnceLock<Box<dyn Fn(RedirKey) + Send + Sync>> = OnceLock::new();
/// Keys whose press went to the picker (their release is swallowed too).
static REDIRECTED: StdMutex<[bool; 256]> = StdMutex::new([false; 256]);

/// Sets the receiver for redirected keys (once).
pub fn init_key_redirect<F: Fn(RedirKey) + Send + Sync + 'static>(f: F) {
    let _ = REDIRECT_CB.set(Box::new(f));
}

/// Sends key presses to window `hwnd` while it is visible (0 = off); `target` is the program
/// that stays in front.
pub fn redirect_keys(hwnd: isize, target: isize) {
    REDIRECT_TARGET.store(target, AtomicOrdering::Relaxed);
    REDIRECT_HWND.store(hwnd, AtomicOrdering::Relaxed);
}

/// Shows a window as a non-activating tool window (clicks don't take the focus either).
pub fn set_no_activate(h: isize) {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE};
    let w = hwnd(h);
    if w.0.is_null() {
        return;
    }
    unsafe {
        let style = GetWindowLongPtrW(w, GWL_EXSTYLE);
        SetWindowLongPtrW(w, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
    }
}

fn dom_key(vk: u32, text: &str) -> String {
    match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x1B => "Escape",
        0x20 => " ",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "ArrowLeft",
        0x26 => "ArrowUp",
        0x27 => "ArrowRight",
        0x28 => "ArrowDown",
        0x2E => "Delete",
        0x70..=0x7B => return format!("F{}", vk - 0x6F),
        0x30..=0x39 if text.is_empty() => return ((b'0' + (vk - 0x30) as u8) as char).to_string(),
        0x41..=0x5A if text.is_empty() => return ((b'a' + (vk - 0x41) as u8) as char).to_string(),
        _ => return text.to_string(),
    }
    .to_string()
}

/// From the hook: true if the key press was handed to the picker (and must be swallowed).
fn redirect_key(vk: u32, scan: u32, down: bool, ctrl: bool, alt: bool, shift: bool, win: bool) -> bool {
    let h = REDIRECT_HWND.load(AtomicOrdering::Relaxed);
    if h == 0 {
        return false;
    }
    if !down {
        if let Ok(mut r) = REDIRECTED.try_lock() {
            if r[(vk & 0xFF) as usize] {
                r[(vk & 0xFF) as usize] = false;
                return true;
            }
        }
        return false;
    }
    if !unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd(h)) }.as_bool() {
        REDIRECT_HWND.store(0, AtomicOrdering::Relaxed);
        return false;
    }
    // Windows shortcuts and app switching stay with Windows.
    if win || (alt && !ctrl && matches!(vk, 0x09 | 0x73 | 0x1B | 0x20)) {
        return false;
    }
    let fg = unsafe { GetForegroundWindow() };
    let text = if !ctrl || alt { key_text(fg, vk, scan, shift, ctrl && alt) } else { String::new() };
    let code = if (0x30..=0x39).contains(&vk) { format!("Digit{}", vk - 0x30) } else { String::new() };
    let key = dom_key(vk, &text);
    if let Ok(mut r) = REDIRECTED.try_lock() {
        r[(vk & 0xFF) as usize] = true;
    }
    if let Some(cb) = REDIRECT_CB.get() {
        cb(RedirKey { key, code, text, ctrl: ctrl && !alt, alt: alt && !ctrl, shift });
    }
    true
}

/// Called from the hook for every real key press that is not a hotkey.
fn on_typed_key(vk: u32, scan: u32, ctrl: bool, alt: bool, shift: bool, win: bool) {
    let Ok(mut buf) = TYPED.try_lock() else { return };
    let fg = unsafe { GetForegroundWindow() };
    if TYPED_FG.swap(fg.0 as isize, AtomicOrdering::Relaxed) != fg.0 as isize {
        buf.clear();
    }
    match vk {
        0x08 => {
            buf.pop();
            return;
        }
        // Enter, Tab, Esc, navigation, Delete: a new "word" starts.
        0x0D | 0x09 | 0x1B | 0x21..=0x28 | 0x2E => {
            buf.clear();
            return;
        }
        _ => {}
    }
    // Shortcuts (Ctrl/Alt/Win alone) are not typing; Ctrl+Alt is AltGr (e.g. "@" on German keyboards).
    if win || (ctrl != alt) {
        buf.clear();
        return;
    }
    let text = key_text(fg, vk, scan, shift, ctrl && alt);
    if text.is_empty() {
        return;
    }
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_control() {
            continue;
        }
        buf.push(c);
    }
    while buf.chars().count() > TYPED_MAX {
        buf.remove(0);
    }
    let hit = KEYWORDS.try_lock().ok().and_then(|k| k.iter().find(|kw| buf.ends_with(kw.as_str())).cloned());
    if let Some(kw) = hit {
        buf.clear();
        if let Some(cb) = TYPING_CB.get() {
            cb(kw, fg.0 as isize);
        }
    }
}
/// Tick count (ms) of the last Win key-down seen by the hook.
static WIN_DOWN_AT: AtomicU32 = AtomicU32::new(0);

fn track_modifier(vk: u32, down: bool) -> bool {
    let slot = match vk {
        0x5B => &MOD_LWIN,
        0x5C => &MOD_RWIN,
        0xA2 => &MOD_CTRL_L,
        0xA3 => &MOD_CTRL_R,
        0x11 => &MOD_CTRL_L,
        0xA4 => &MOD_ALT_L,
        0xA5 => &MOD_ALT_R,
        0x12 => &MOD_ALT_L,
        0xA0 => &MOD_SHIFT_L,
        0xA1 => &MOD_SHIFT_R,
        0x10 => &MOD_SHIFT_L,
        _ => return false,
    };
    slot.store(down, AtomicOrdering::Relaxed);
    if down && (vk == 0x5B || vk == 0x5C) {
        WIN_DOWN_AT.store(unsafe { GetTickCount() }, AtomicOrdering::Relaxed);
    }
    true
}

/// Win state with a safety net: if a Win key-up got lost (some tools or Windows itself
/// re-inject it, or it went to an elevated window we can't see), the tracked flag could stay
/// "down" forever. After a few seconds we therefore also require the async key state.
fn win_down_checked() -> bool {
    let tracked = MOD_LWIN.load(AtomicOrdering::Relaxed) || MOD_RWIN.load(AtomicOrdering::Relaxed);
    if !tracked {
        // The Win press itself may have been swallowed by a hook ahead of ours (or happened
        // while ours was being re-installed); the system key state still knows.
        return is_down(VK_LWIN) || is_down(VK_RWIN);
    }
    let age = unsafe { GetTickCount() }.wrapping_sub(WIN_DOWN_AT.load(AtomicOrdering::Relaxed));
    if age < 3000 || is_down(VK_LWIN) || is_down(VK_RWIN) {
        return true;
    }
    MOD_LWIN.store(false, AtomicOrdering::Relaxed);
    MOD_RWIN.store(false, AtomicOrdering::Relaxed);
    false
}

/// Low-level keyboard hook for combinations that Windows (or another app) has registered,
/// e.g. Win+V. Only exact matches are swallowed; every other key passes untouched.
unsafe extern "system" fn ll_keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let start = unsafe { windows::Win32::System::SystemInformation::GetTickCount64() };
    let r = unsafe { ll_keyboard_inner(code, wparam, lparam) };
    let took = unsafe { windows::Win32::System::SystemInformation::GetTickCount64() }.saturating_sub(start) as u32;
    if took >= 50 {
        HOOK_SLOW.fetch_add(1, AtomicOrdering::Relaxed);
        HOOK_SLOW_MAX.fetch_max(took, AtomicOrdering::Relaxed);
    }
    r
}

/// Hook calls that took ≥ 50 ms (and the slowest), logged from the hook thread's timer.
static HOOK_SLOW: AtomicU32 = AtomicU32::new(0);
static HOOK_SLOW_MAX: AtomicU32 = AtomicU32::new(0);

unsafe fn ll_keyboard_inner(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let injected = (kb.flags.0 & LLKHF_INJECTED.0) != 0;
        let msg = wparam.0 as u32;
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        // Modifiers are tracked from every event, injected ones included (the logical state
        // is what matters), and always pass through.
        if (down || up) && track_modifier(kb.vkCode, down) {
        } else if !injected && down {
            let win = win_down_checked();
            // Ctrl/Alt/Shift come from the system key state: a tracked flag can get stuck when
            // the key-up is never seen (released in an elevated window, which low-level hooks
            // of a normal process don't receive), and then Win+. stops matching.
            let ctrl = is_down(VK_CONTROL);
            let alt = is_down(VK_MENU);
            let shift = is_down(VK_SHIFT);
            // No allocation here: the hook must return within Windows' timeout, or the key goes
            // on to Windows (emoji panel, clipboard history) and repeated misses get the hook dropped.
            let hit = HOOK_COMBOS.try_lock().ok().and_then(|c| c.iter().find(|c| c.vk == kb.vkCode && c.win == win && c.ctrl == ctrl && c.alt == alt && c.shift == shift).copied());
            if hit.is_none() && redirect_key(kb.vkCode, kb.scanCode, true, ctrl, alt, shift, win) {
                return LRESULT(1);
            }
            if hit.is_none() && TYPING.load(AtomicOrdering::Relaxed) {
                on_typed_key(kb.vkCode, kb.scanCode, ctrl, alt, shift, win);
            }
            if let Some(c) = hit {
                if c.win || c.alt {
                    // Mask Win/Alt so releasing them doesn't open the Start menu / menu bar.
                    send(&[key(VK_NEUTRAL, false), key(VK_NEUTRAL, true)]);
                }
                SWALLOWED_VK.store(kb.vkCode, AtomicOrdering::Relaxed);
                if let Some(cb) = HOOK_CB.get() {
                    cb(c.id);
                }
                return LRESULT(1);
            }
        } else if !injected && up && redirect_key(kb.vkCode, kb.scanCode, false, false, false, false, false) {
            return LRESULT(1);
        } else if !injected && up && SWALLOWED_VK.load(AtomicOrdering::Relaxed) == kb.vkCode {
            SWALLOWED_VK.store(0, AtomicOrdering::Relaxed);
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Thread of the hook (for the foreground/focus events to wake it).
static HOOK_TID: AtomicU32 = AtomicU32::new(0);
const WM_REFRESH_HOOK: u32 = 0x8000 + 41; // WM_APP + 41

/// Foreground or focus changed somewhere: ask the hook thread to move our hook to the front.
unsafe extern "system" fn on_win_event(
    _h: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    win: HWND,
    _obj: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    // Another program came to the front while the picker is open: it closes.
    let picker = REDIRECT_HWND.load(AtomicOrdering::Relaxed);
    if event == 0x0003 && picker != 0 && win.0 as isize != REDIRECT_TARGET.load(AtomicOrdering::Relaxed) {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(win, Some(&mut pid)) };
        if pid != std::process::id() {
            REDIRECT_HWND.store(0, AtomicOrdering::Relaxed);
            if let Some(cb) = REDIRECT_CB.get() {
                cb(RedirKey { key: "__blur".into(), code: String::new(), text: String::new(), ctrl: false, alt: false, shift: false });
            }
        }
    }
    let tid = HOOK_TID.load(AtomicOrdering::Relaxed);
    if tid != 0 {
        let _ = unsafe { PostThreadMessageW(tid, WM_REFRESH_HOOK, WPARAM(0), LPARAM(0)) };
    }
}

/// How often the hook is re-installed to stay first in the hook chain.
const HOOK_REFRESH_MS: u32 = 3000;

/// Sets the combinations handled by the hook. The hook thread only runs while the list is non-empty.
pub fn set_hook_combos<F: Fn(u8) + Send + Sync + 'static>(combos: Vec<HookCombo>, on_press: F) {
    let _ = HOOK_CB.set(Box::new(on_press));
    *HOOK_COMBOS.lock().unwrap_or_else(|e| e.into_inner()) = combos;
    apply_hook_state();
}

/// Starts or stops the hook thread: it runs while there are hotkey combos or typing expansion.
fn apply_hook_state() {
    let enabled = !HOOK_COMBOS.lock().unwrap_or_else(|e| e.into_inner()).is_empty() || TYPING.load(AtomicOrdering::Relaxed);
    let mut thread = HOOK_THREAD.lock().unwrap_or_else(|e| e.into_inner());
    match (enabled, *thread) {
        (true, None) => {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("hotkey-hook".into())
                .spawn(move || unsafe {
                    let module = GetModuleHandleW(None).ok().map(|m| HINSTANCE(m.0));
                    let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard), module, 0) {
                        Ok(h) => h,
                        Err(e) => {
                            log::error!("keyboard hook failed: {e}");
                            let _ = tx.send(None);
                            return;
                        }
                    };
                    // Seed the tracked modifier state from the current keyboard state.
                    MOD_LWIN.store(is_down(VK_LWIN), AtomicOrdering::Relaxed);
                    MOD_RWIN.store(is_down(VK_RWIN), AtomicOrdering::Relaxed);
                    MOD_CTRL_L.store(is_down(VK_CONTROL), AtomicOrdering::Relaxed);
                    MOD_CTRL_R.store(false, AtomicOrdering::Relaxed);
                    MOD_ALT_L.store(is_down(VK_MENU), AtomicOrdering::Relaxed);
                    MOD_ALT_R.store(false, AtomicOrdering::Relaxed);
                    MOD_SHIFT_L.store(is_down(VK_SHIFT), AtomicOrdering::Relaxed);
                    MOD_SHIFT_R.store(false, AtomicOrdering::Relaxed);
                    let _ = tx.send(Some(GetCurrentThreadId()));
                    // Low-level hooks run newest first. A program that installs its own hook
                    // later (PowerToys, other clipboard tools, after an update or restart) can
                    // swallow Win+V before it reaches us, and Windows silently drops hooks that
                    // once took too long. Re-installing every few seconds keeps ours in front.
                    let mut hook = hook;
                    // Low-level hook callbacks run on this thread; keep it ahead of busy UI threads.
                    let _ = windows::Win32::System::Threading::SetThreadPriority(
                        windows::Win32::System::Threading::GetCurrentThread(),
                        windows::Win32::System::Threading::THREAD_PRIORITY_TIME_CRITICAL,
                    );
                    let timer = SetTimer(None, 0, HOOK_REFRESH_MS, None);
                    // Windows' text input (emoji panel) can put its own hook in front of ours when a
                    // text field gets focus; later hooks see keys first. So move ours back to the front
                    // right away on every foreground/focus change, and once more shortly after.
                    HOOK_TID.store(GetCurrentThreadId(), AtomicOrdering::Relaxed);
                    use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent};
                    const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
                    const EVENT_OBJECT_FOCUS: u32 = 0x8005;
                    const WINEVENT_OUTOFCONTEXT: u32 = 0;
                    let ev_fg = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_win_event), 0, 0, WINEVENT_OUTOFCONTEXT);
                    let ev_focus = SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, None, Some(on_win_event), 0, 0, WINEVENT_OUTOFCONTEXT);
                    let mut last_refresh = 0u64;
                    // Thread timers get their id from Windows (the id argument is ignored).
                    let mut late: usize = 0;
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let periodic = msg.message == WM_TIMER && msg.wParam.0 == timer;
                        let focus_now = msg.message == WM_REFRESH_HOOK;
                        let focus_later = msg.message == WM_TIMER && late != 0 && msg.wParam.0 == late;
                        if focus_later {
                            let _ = KillTimer(None, late);
                            late = 0;
                        }
                        if focus_now {
                            // Again a little later, after the text input of the new window is set up.
                            if late != 0 {
                                let _ = KillTimer(None, late);
                            }
                            late = SetTimer(None, 0, 250, None);
                            let now = windows::Win32::System::SystemInformation::GetTickCount64();
                            if now.saturating_sub(last_refresh) < 50 {
                                continue;
                            }
                        }
                        if periodic || focus_now || focus_later {
                            last_refresh = windows::Win32::System::SystemInformation::GetTickCount64();
                            if periodic {
                                let slow = HOOK_SLOW.swap(0, AtomicOrdering::Relaxed);
                                if slow > 0 {
                                    log::warn!("keyboard hook: {slow} slow call(s), slowest {} ms", HOOK_SLOW_MAX.swap(0, AtomicOrdering::Relaxed));
                                }
                            }
                            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard), module, 0) {
                                Ok(fresh) => {
                                    let _ = UnhookWindowsHookEx(hook);
                                    hook = fresh;
                                }
                                Err(e) => log::warn!("keyboard hook refresh failed: {e}"),
                            }
                        }
                    }
                    let _ = KillTimer(None, timer);
                    let _ = UnhookWinEvent(ev_fg);
                    let _ = UnhookWinEvent(ev_focus);
                    let _ = UnhookWindowsHookEx(hook);
                    log::info!("keyboard hook removed");
                })
                .ok();
            *thread = rx.recv().ok().flatten();
            if thread.is_some() {
                log::info!("keyboard hook active for reserved hotkeys");
            }
        }
        (false, Some(tid)) => {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            *thread = None;
        }
        _ => {}
    }
}

/// Windows display language as a UI language code ("de", "en", …).
pub fn system_language() -> String {
    let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    match id {
        0x07 => "de",
        0x09 => "en",
        0x10 => "it",
        0x0a => "es",
        0x0c => "fr",
        0x13 => "nl",
        0x15 => "pl",
        0x16 => "pt",
        0x2a => "vi",
        _ => "en",
    }
    .into()
}

/// True if the focused control of window `h` is a classic password edit field.
pub fn focused_is_password(h: isize) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW, GWL_STYLE};
    let w = hwnd(h);
    if w.0.is_null() {
        return false;
    }
    let tid = unsafe { GetWindowThreadProcessId(w, None) };
    let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    if unsafe { GetGUIThreadInfo(tid, &mut gti) }.is_err() || gti.hwndFocus.0.is_null() {
        return false;
    }
    const ES_PASSWORD: i32 = 0x20;
    let style = unsafe { GetWindowLongW(gti.hwndFocus, GWL_STYLE) };
    let mut class = [0u16; 32];
    let n = unsafe { GetClassNameW(gti.hwndFocus, &mut class) };
    let class = String::from_utf16_lossy(&class[..n.max(0) as usize]).to_lowercase();
    class.contains("edit") && style & ES_PASSWORD != 0
}

/// Sends `n` backspaces (to remove a typed snippet keyword).
pub fn send_backspaces(n: usize) {
    let mut v = Vec::with_capacity(n * 2);
    for _ in 0..n {
        v.push(key(VIRTUAL_KEY(0x08), false));
        v.push(key(VIRTUAL_KEY(0x08), true));
    }
    send(&v);
}

/// Opens a web page in the default browser.
pub fn open_url(url: &str) -> Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let r = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL) };
    // Values above 32 mean success.
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(anyhow!("could not open {url}"))
    }
}

// ---------------------------------------------------------------- Explorer hotkeys

const EXPLORER_ADVANCED: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced");

/// Explorer's `DisabledHotkeys` value: characters of Win+<key> shortcuts Explorer skips.
pub fn shell_disabled_hotkeys() -> String {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ};
    let mut buf = [0u16; 256];
    let mut len = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, EXPLORER_ADVANCED, w!("DisabledHotkeys"), RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ, None, Some(buf.as_mut_ptr() as *mut c_void), Some(&mut len))
    };
    if r.is_err() {
        return String::new();
    }
    let n = (len as usize / 2).min(buf.len());
    String::from_utf16_lossy(&buf[..n]).trim_end_matches('\0').to_string()
}

pub fn set_shell_disabled_hotkeys(keys: &str) -> Result<()> {
    use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    let r = if keys.is_empty() {
        let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, EXPLORER_ADVANCED, w!("DisabledHotkeys")) };
        // Already absent is fine.
        if r.0 == 2 {
            return Ok(());
        }
        r
    } else {
        let wide: Vec<u16> = keys.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, EXPLORER_ADVANCED, w!("DisabledHotkeys"), REG_SZ.0, Some(wide.as_ptr() as *const c_void), (wide.len() * 2) as u32) }
    };
    r.ok().map_err(|e| anyhow!("registry: {e}"))
}

/// True if our exe lives on a network drive (slow page-ins can delay the keyboard hook).
pub fn exe_on_network_drive() -> bool {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;
    let Ok(exe) = std::env::current_exe() else { return false };
    let p = exe.display().to_string();
    if p.starts_with("\\\\") {
        return true;
    }
    let root: String = p.chars().take(3).collect();
    const DRIVE_REMOTE: u32 = 4;
    unsafe { GetDriveTypeW(&HSTRING::from(root)) == DRIVE_REMOTE }
}


// ---------------------------------------------------------------- geometry

fn monitor_rect(m: windows::Win32::Graphics::Gdi::HMONITOR) -> Option<Rect> {
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe { GetMonitorInfoW(m, &mut mi) }.as_bool().then(|| {
        let r = mi.rcWork;
        Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top }
    })
}

/// Hides a window directly (also one shown with `show_no_activate`, which Tauri doesn't know
/// as visible and therefore wouldn't hide).
pub fn hide_window(h: isize) {
    let w = hwnd(h);
    if w.0.is_null() {
        return;
    }
    unsafe {
        let _ = ShowWindow(w, windows::Win32::UI::WindowsAndMessaging::SW_HIDE);
    }
}

pub fn is_window_visible(h: isize) -> bool {
    let w = hwnd(h);
    !w.0.is_null() && unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(w) }.as_bool()
}

/// Shows a window without activating it (the user keeps typing where they were).
pub fn show_no_activate(h: isize) {
    let w = hwnd(h);
    if w.0.is_null() {
        return;
    }
    unsafe {
        let _ = ShowWindow(w, SW_SHOWNOACTIVATE);
    }
}

pub fn work_area_for_window(h: isize) -> Option<Rect> {
    let h = hwnd(h);
    if h.0.is_null() {
        return None;
    }
    monitor_rect(unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) })
}

/// Work area of the monitor under the mouse plus the cursor position.
pub fn work_area_at_cursor() -> Option<(Rect, i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok()?;
    let r = monitor_rect(unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) })?;
    Some((r, p.x, p.y))
}

pub fn work_area_at_point(x: i32, y: i32) -> Option<Rect> {
    monitor_rect(unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) })
}

/// Screen position (bottom-left) of the text caret in the given window's thread, if the app
/// uses a system caret (classic Win32/WinForms/WPF/Office do; many Chromium apps don't).
pub fn caret_screen_pos(h: isize) -> Option<(i32, i32)> {
    let w = hwnd(h);
    if w.0.is_null() {
        return None;
    }
    let tid = unsafe { GetWindowThreadProcessId(w, None) };
    let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    unsafe { GetGUIThreadInfo(tid, &mut gti) }.ok()?;
    if gti.hwndCaret.0.is_null() {
        return None;
    }
    let r = gti.rcCaret;
    if r.right - r.left <= 0 && r.bottom - r.top <= 0 {
        return None;
    }
    let mut p = POINT { x: r.left, y: r.bottom };
    unsafe { ClientToScreen(gti.hwndCaret, &mut p) }.as_bool().then_some((p.x, p.y))
}

// ---------------------------------------------------------------- OCR

pub fn ocr_png(png: &[u8]) -> Result<String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages().context("no OCR language installed")?;
    let max = OcrEngine::MaxImageDimension()?;
    let mut img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?;
    if img.width() > max || img.height() > max {
        img = img.resize(max, max, image::imageops::FilterType::Triangle);
    } else if img.width() < 400 && img.height() < 400 {
        // Small crops (single words) are recognized much better when upscaled.
        img = img.resize(img.width() * 2, img.height() * 2, image::imageops::FilterType::CatmullRom);
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut bgra = rgba.into_raw();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(&bgra)?;
    let buffer = writer.DetachBuffer()?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w as i32, h as i32)?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;
    Ok(result.Text()?.to_string())
}

// ---------------------------------------------------------------- credentials

pub fn secret_get(service: &str, user: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(service, user)?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn secret_set(service: &str, user: &str, value: Option<&str>) -> Result<()> {
    let entry = keyring::Entry::new(service, user)?;
    match value {
        Some(v) if !v.is_empty() => entry.set_password(v)?,
        _ => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(e.into()),
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_roundtrip() {
        let wrapped = protect(b"hello vault").unwrap();
        assert_ne!(wrapped, b"hello vault");
        assert_eq!(unprotect(&wrapped).unwrap(), b"hello vault");
    }

    // Clipboard tests touch the real (shared) clipboard; run them serially in one test.
    #[test]
    fn clipboard_roundtrip() {
        let limits = Limits { max_text_bytes: 5 << 20, max_image_bytes: 50 << 20 };

        let text = ClipContent { text: Some("Grüße 👋".into()), html: Some("<b>Grüße</b>".into()), rtf: Some("{\\rtf1 Gr\\'fc\\'dfe}".into()), ..Default::default() };
        write_clipboard(&text).unwrap();
        let got = read_clipboard(limits).unwrap();
        assert!(!got.excluded);
        assert_eq!(got.content.text.as_deref(), Some("Grüße 👋"));
        let html = got.content.html.clone().expect("html read back");
        assert!(html.starts_with("Version:") && html.contains("<b>Grüße</b>"));
        // raw CF_HTML is written back verbatim
        write_clipboard(&got.content).unwrap();
        assert_eq!(read_clipboard(limits).unwrap().content.html.as_deref(), Some(html.as_str()));
        assert!(got.content.rtf.as_deref().unwrap().starts_with("{\\rtf1"));

        let files = ClipContent { files: vec!["C:\\Windows\\notepad.exe".into()], ..Default::default() };
        write_clipboard(&files).unwrap();
        assert_eq!(read_clipboard(limits).unwrap().content.files, files.files);

        let img = image::RgbaImage::from_pixel(40, 20, image::Rgba([200, 10, 10, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        write_clipboard(&ClipContent { image_png: Some(png.into_inner()), ..Default::default() }).unwrap();
        let got = read_clipboard(limits).unwrap();
        let back = image::load_from_memory(got.content.image_png.as_ref().expect("image read back")).unwrap();
        assert_eq!((back.width(), back.height()), (40, 20));

        // Sensitive marker → excluded
        {
            let _c = Clipboard::new_attempts(10).unwrap();
            raw::empty().unwrap();
            raw::set_string_with("password123", NoClear).unwrap();
            raw::set_without_clear(fmt("ExcludeClipboardContentFromMonitorProcessing").unwrap(), &[0]).unwrap();
        }
        assert!(read_clipboard(limits).unwrap().excluded);

        let before = clipboard_seq();
        let seq = write_clipboard(&ClipContent::text_only("seq")).unwrap();
        assert_ne!(before, seq);
        assert_eq!(seq, clipboard_seq());
    }

    #[test]
    fn self_is_not_elevated_target() {
        assert!(!is_window_elevated_for_us(0));
        assert!(!is_valid_paste_target(0));
    }
}
