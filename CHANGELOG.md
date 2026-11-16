# Changelog

## 1.1.0

- The paste window can be moved by its whole header and its edges again
- Snippets while typing: input fields and choices now open a small popup right at the text cursor – choose with the arrow keys, 1–9 or by typing, Enter pastes, Esc keeps the keyword as typed
- Emoji, symbols and GIFs no longer replace what you copied: after pasting them, the previous clipboard content is put back
- Window position can be chosen for each window – paste window, emoji picker and translate window: center of the screen, at the text cursor or at the mouse
- The text cursor is now also found in browsers and apps like Teams, WhatsApp, Slack, VS Code and the new Outlook (via the Windows accessibility interfaces)
- Autostart: an autostart entry that still pointed to another copy of the app (an old portable exe, a network drive or a deleted file) is now corrected, so UltimatePaste starts reliably after a reboot

## 1.0.1

- Emoji & symbols: favorites and recently used are now sorted by how often you use them (most used first)

## 1.0.0 – first public release

**Copy & paste**
- Encrypted clipboard history for text, links, images (with local OCR), files and colors; instant, typo-tolerant search
- Four window styles (Fluent, Board, Drawer, Compact), eight designs, light/dark
- Paste as rich text, plain text, HTML or RTF; paste sequentially; pin, rename, edit; delete by time range
- Snippets with placeholders and input fields, snippets while typing (`;keyword`)
- Quick actions by content (JSON, colors, case, links without tracking, mail, phone, maps, parcel tracking)
- Paste several entries together with a chosen separator; rules per program
- AI actions with Claude or any OpenAI-compatible endpoint; inbox export as Markdown

**Translate & correct**
- Translate the selection with DeepL (Free/Pro) or AI: language detection, pairs, formality, direct replace
- Correct or improve (style/tone) with DeepL Write, DeepL double translation or AI; word-by-word diff, accept/reject per change
- History of the last 20 runs; costs per run and month

**Emoji, GIF & symbols**
- Picker at the text cursor that never steals the focus; search in 9 languages, skin tones, favorites, recently used
- GIFs and stickers via GIPHY or KLIPY with your own key; ~500 special characters

**Everything else**
- Any shortcut, including `Win+V` and `Win+.`; conflict detection for Windows and PowerToys
- Privacy: password managers ignored, short-lived secrets auto-removed, tracking stripped from links
- Settings export/import (API keys optionally sealed with a password), statistics, welcome tour
- Update check with verified self-update (installer and portable)
- 9 UI languages: English, German, Spanish, French, Italian, Dutch, Polish, Portuguese (Brazil), Vietnamese
- Security audit with fixes before release
