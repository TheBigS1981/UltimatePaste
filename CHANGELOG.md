# Changelog

## 1.1.1

- Snippet popup in browsers (e.g. Firefox): choosing with the arrow keys and `Enter` now replaces the keyword reliably. The `Enter` that confirms the popup no longer reaches the web page, and pasting waits until the key is released. The emoji picker benefits too.

## 1.1.0 – Less mouse, more flow

**✨ Snippets with choices, right where you type.** Type `;mail` and a small popup appears directly below your text cursor: pick with `↑` `↓` or `1`–`9`, type to filter, `Enter` pastes, `Esc` keeps the keyword as typed. Several fields come one after the other. The program you type in keeps the focus, and your clipboard stays untouched.

**😊 Emoji no longer eat your clipboard.** After pasting emoji, symbols or GIFs, whatever you copied before is back on the clipboard.

**🎯 Windows open where you want them.** Paste window, emoji picker and translate window: at the text cursor, at the mouse or in the middle of the screen. The text cursor is now also found in browsers, Teams, WhatsApp, Slack, VS Code and the new Outlook.

**🛟 No more lost edits.** `Esc` or a click beside the snippet/entry editor asks before discarding changes, and switching to another program keeps your draft.

**🔧 Smaller things**
- Autostart heals itself when its entry still pointed to an old copy, a network drive or a deleted file
- The paste window can be moved by its whole header and its edges again

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
