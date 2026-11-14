# Security

## Reporting a vulnerability

Please **don't open a public issue** for security problems. Use GitHub's private reporting instead: *Security → Report a vulnerability* in this repository. You'll get an answer as soon as possible. Please include the version (*Settings → About*) and the steps to reproduce.

## What UltimatePaste does with your data

- **Clipboard history, snippets, statistics and translation history** stay on your PC in `%LOCALAPPDATA%\UltimatePaste`, encrypted with AES-256-GCM. The key is protected with Windows DPAPI and bound to your Windows account. Other programs running as the same Windows user could still unwrap it, as with any DPAPI-protected data.
- **Content from password managers** (marked as confidential by the source) is never recorded, and when UltimatePaste temporarily restores such content to the clipboard it keeps the "don't record / don't sync" markers. 2FA codes, card numbers and API keys are removed from the history after a few minutes (configurable).
- **Network access only on your action:**
  - Text goes to DeepL or your AI provider when you translate, correct or run an AI action.
  - Search terms go to GIPHY/KLIPY when you search for GIFs.
  - The daily update check only asks GitHub for the latest release number (can be switched off).
  - API keys are kept in the Windows Credential Manager and are never sent over unencrypted `http://` to another computer.
- **Keyboard hook:** UltimatePaste uses a low-level keyboard hook for shortcuts Windows reserves (e.g. `Win+V`, `Win+.`). With *snippets while typing* turned on (off by default) it keeps the last ~40 typed characters **in memory only** to recognize snippet keywords. Nothing typed is ever stored or sent.
- **Updates** are downloaded only after you confirm, and are verified against the SHA-256 checksums published with the release.

## Code signing

The releases are **not code-signed yet**, so SmartScreen and Microsoft Defender may warn about them (see the README). Every release lists the SHA-256 checksums of its files, so you can verify a download with `Get-FileHash <file> -Algorithm SHA256` in PowerShell.
