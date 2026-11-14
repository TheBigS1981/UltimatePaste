# UltimatePaste – Specification

A fast, lightweight, keyboard-driven clipboard & paste manager for **Windows 11 x64**.
Not a launcher. Everything stays local; AI only on explicit request.

## Decisions

| Topic | Decision |
|---|---|
| Stack | Rust + Tauri 2 (WebView2), Svelte 5 frontend. Window is pre-warmed and hidden. |
| Platform | Windows 11 x64 only. Built on Linux via Docker + cargo-xwin; CI on GitHub Actions `windows-latest`. |
| Scope | Clipboard history + paste only. No launcher. |
| Sync | None. Local only. |
| Storage | SQLite (`%LOCALAPPDATA%\UltimatePaste`). Payloads AES-256-GCM encrypted; data key protected by DPAPI (bound to the Windows user). Images as encrypted files. |
| Search | In-memory index, multi-word AND, accent/case-insensitive substring + fuzzy matching on titles/first line, ranked by relevance and recency. |
| Hotkey | `Ctrl+Shift+V` (configurable). Win+V untouched. Sequential paste: `Ctrl+Shift+Alt+V`. |
| Window | Center of the active monitor (option: at mouse cursor). List left, preview right, `Ctrl+Enter` action menu (configurable), follows Windows light/dark, Mica. |
| Privacy | Honors `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory=0`, `Clipboard Viewer Ignore`; app block list by process (password managers). Source app is captured at copy time; hashes of excluded content are remembered so "clipboard restore" tricks can't sneak them in. |
| Limits | Text ≤ 5 MB, images ≤ 50 MB per entry; default retention 3 months (configurable up to unlimited); pinned entries are kept forever. |
| Elevated targets | App runs unelevated. Pasting into elevated windows is blocked by Windows (UIPI) → content is copied and a hint is shown. |
| AI | Anthropic API + OpenAI-compatible endpoints (Ollama, LM Studio, OpenRouter, own server). Keys in Windows Credential Manager. Only on explicit action; disabled for content from blocked apps. Built-in actions + custom prompt templates. Result is pasted and stored in history. |
| Snippets | Own tab, placeholders `{date}`, `{time}`, `{datetime}`, `{date:%Y-%m-%d}`, `{clipboard}`, `{cursor}`. Text expansion later (opt-in). |
| Updates | Manual until v1.0, then GitHub Releases + signed auto-update with confirmation. |
| License | MIT. Repo private until v1.0. |
| Language | Code/docs English; UI German (default) + English. |

## Roadmap

- **v0.1** history (text/images/files), search, hotkey, paste, paste as plain text, pin, tray, autostart, privacy filters, encrypted storage
- **v0.2** paste as… (rich text/HTML/RTF/plain), edit & rename, paste sequentially, bulk delete by time range, pause, retention setting
- **v0.3** OCR (Windows.Media.Ocr), images tab, inbox export (Markdown + YAML front matter)
- **v0.4** snippets with placeholders
- **v0.5** AI actions
- **v1.0** polish, complete settings UI, public release, auto-update
