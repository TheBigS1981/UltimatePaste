# Changelog

## 1.1.2

- Named entries are easier to tell apart: the list shows the name together with the start of the content. `F2` puts the caret straight into the name field, with the old name selected.
- Pasting only goes to windows you can see. A program hidden in the tray (e.g. Slack) could still count as the active window and receive the paste; now the content just stays on the clipboard. Right before pasting, UltimatePaste checks again that the target is still in front.
- Snippets while typing in terminals (Windows Terminal, WezTerm, PuTTY, …): the `;` of the keyword is no longer left behind, especially with remote shells.

## 1.1.1

- Images from the history open in your default image viewer: `Ctrl+O`, the actions menu, or a double-click on the preview. The viewer gets a temporary copy that is deleted after 10 minutes and at the next start.
- Correction view: changes are much easier to read, especially in dark designs. Removed text is marked red and struck through, inserted text is underlined in green.
- Snippet popup in browsers (e.g. Firefox): choosing with the arrow keys and `Enter` now replaces the keyword reliably. The `Enter` that confirms the popup no longer reaches the web page, and pasting waits until the key is released. The emoji picker benefits too.
- Snippets while typing: the keyword is now removed key by key, so script-heavy web forms no longer leave a stray `;` behind.
- Emoji & symbols: favorites are no longer repeated under “recently used”, and both lists can be tidied up – `Del` or the small × on hover removes an entry.
- Paste window: typing in the actions menu (`Ctrl+Enter`) filters the actions again instead of searching the history behind it.
- The log file shows local time instead of UTC.
- Screenshot tools like Greenshot work while the emoji picker is open (Print and the media keys are no longer captured).
- Emoji picker, snippet popup and translate window always appear in front, even over other “always on top” windows such as WhatsApp.

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
