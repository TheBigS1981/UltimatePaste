// Dev-only harness: renders the UI in a normal browser with mocked backend data.
// Usage: /dev-preview.html?layout=board&theme=dark&window=main|settings
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const q = new URLSearchParams(location.search);
const win = q.get("window") ?? "main";
mockWindows(win);

const now = Date.now();
const min = 60_000;
const png1 = "data:image/svg+xml;utf8," + encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='360' height='195'><rect width='360' height='195' fill='#1e2230'/><rect x='0' y='0' width='100' height='195' fill='#2b3042'/>${Array.from({ length: 11 }, (_, i) => `<rect x='115' y='${14 + i * 16}' width='${120 + ((i * 37) % 110)}' height='7' rx='3' fill='#8fa3c8' opacity='.7'/>`).join("")}</svg>`);

// Dummy content only (used for README screenshots): de or en depending on ?lang=
const EN = (q.get("lang") ?? "de") !== "de";
const D = (de: string, en: string) => (EN ? en : de);
const entries = [
  { id: 1, kind: "text", title: null, preview: D("Hallo Frau Berger, ⏎ anbei das Angebot für die neue Website von Nordlicht Design.", "Hi Ms. Parker, ⏎ please find attached the quote for the new Northwind Studio website."), sourceApp: "outlook.exe", t: 2, formats: ["text", "html", "rtf"], text: D("Hallo Frau Berger,\n\nanbei das Angebot für die neue Website von Nordlicht Design. Die Preise gelten bis 31.12.2026.\n\nViele Grüße\nAlex", "Hi Ms. Parker,\n\nplease find attached the quote for the new Northwind Studio website. Prices are valid until Dec 31, 2026.\n\nBest regards\nAlex") },
  { id: 2, kind: "image", title: null, preview: "", sourceApp: "snippingtool.exe", t: 5, formats: ["image"], w: 1228, h: 665, ocr: "npm run build && npm run deploy" },
  { id: 3, kind: "text", title: null, preview: "docker compose up -d --build", sourceApp: "windowsterminal.exe", t: 31, formats: ["text"], text: "docker compose up -d --build\ndocker compose logs -f api" },
  { id: 4, kind: "text", title: D("Firmen-IBAN (Beispiel)", "Company IBAN (example)"), preview: "DE02 1203 0000 0000 2020 51", sourceApp: "ultimatepaste.exe", t: 55, pinned: true, formats: ["text"], text: "DE02 1203 0000 0000 2020 51" },
  { id: 5, kind: "link", title: null, preview: "https://example.com/blog/clipboard-tips?utm_source=newsletter", sourceApp: "chrome.exe", t: 70, formats: ["text"], text: "https://example.com/blog/clipboard-tips?utm_source=newsletter" },
  { id: 6, kind: "text", title: null, preview: "#5B4CF0", sourceApp: "figma.exe", t: 125, formats: ["text"], text: "#5B4CF0" },
  { id: 7, kind: "files", title: null, preview: D("Angebot_2026-11.pdf, Logo.svg", "Quote_2026-11.pdf, Logo.svg"), sourceApp: "explorer.exe", t: 60 * 20, formats: ["files"], files: [D("C:\\Users\\alex\\Documents\\Angebot_2026-11.pdf", "C:\\Users\\alex\\Documents\\Quote_2026-11.pdf"), "C:\\Users\\alex\\Pictures\\Logo.svg"] },
  { id: 8, kind: "text", title: null, preview: D("Angebotssumme 12.480,00 €", "Quote total €12,480.00"), sourceApp: "excel.exe", t: 60 * 22, formats: ["text", "html"], text: D("Angebotssumme 12.480,00 €", "Quote total €12,480.00") },
  { id: 9, kind: "text", title: null, preview: D("Können wir den Termin auf Donnerstag 14 Uhr verschieben? ⏎ Danke dir!", "Could we move the meeting to Thursday at 2 pm? ⏎ Thanks!"), sourceApp: "ms-teams.exe", t: 60 * 26, formats: ["text"], text: D("Können wir den Termin auf Donnerstag 14 Uhr verschieben?\nDanke dir!", "Could we move the meeting to Thursday at 2 pm?\nThanks!") },
  { id: 10, kind: "image", title: D("Architektur-Skizze", "Architecture sketch"), preview: "", sourceApp: "snippingtool.exe", t: 60 * 50, formats: ["image"], w: 919, h: 575 },
];
const summary = (e: (typeof entries)[number]) => ({
  id: e.id, kind: e.kind, title: e.title, preview: e.preview, sourceApp: e.sourceApp,
  createdAt: now - e.t * min, lastUsedAt: now - e.t * min, pinned: !!e.pinned, formats: e.formats,
  imageW: e.w ?? null, imageH: e.h ?? null, fileCount: e.files?.length ?? 0, byteSize: 1834, hasOcr: !!e.ocr,
});
const snippets = [
  { id: 1, name: D("Grußformel", "Sign-off"), keyword: D("mfg", "br"), body: D("Mit freundlichen Grüßen\nAlex Morgan\nNordlicht Design", "Best regards\nAlex Morgan\nNorthwind Studio"), updatedAt: now },
  { id: 2, name: D("Angebot nachfassen", "Quote follow-up"), keyword: "follow", body: D("Hallo {auswahl:Anrede|Frau|Herr} {eingabe:Name},\n\nich wollte kurz nachfragen, ob Sie unser Angebot vom {date} schon prüfen konnten.", "Hi {input:Name},\n\njust checking in whether you had a chance to review our quote from {date}."), updatedAt: now },
];
const settings = {
  language: q.get("lang") ?? "de", theme: q.get("theme") ?? "system", layout: q.get("layout") ?? "fluent", design: q.get("design") ?? "auto",
  hotkey: q.get("section") === "hotkeys" ? "Super+V" : "Ctrl+Shift+V", sequentialHotkey: "Ctrl+Shift+Alt+V", position: "center", autostart: true, paused: false,
  retentionDays: 90, maxTextMb: 5, maxImageMb: 50, preferPlainText: false, primaryAction: "paste", moveUsedToTop: true, actionsKey: "Ctrl+Enter",
  blockedApps: ["keepass.exe", "1password.exe"], ocrEnabled: true, inboxDir: null, inboxTags: ["clipboard"],
  sequentialTimeoutSecs: 60, sequentialNewline: false,
  pickerHotkey: "Ctrl+Period", translateHotkey: "Ctrl+Alt+T", translateAltHotkey: "", correctHotkey: "Ctrl+Alt+K", notifiedConflicts: [], appRules: [{ app: "outlook.exe", mode: "plain" }], sensitive: { otp: true, cards: true, keys: true, iban: false, minutes: 5 }, cleanLinks: false, snippetExpand: false, tourDone: true, updateCheck: true,
  picker: { recentCount: 24, skinTone: Number(q.get("tone") ?? 0), gifEnabled: true, gifProvider: "giphy", gifRating: "pg", gifOriginalSize: false, gifFeedback: true, position: "caret" },
  translate: { mode: "window", rule: "pair", primary: "DE", secondary: "EN-US", fixedTarget: "EN-US", engine: q.get("engine") ?? "deepl", aiProvider: "", aiModel: "claude-sonnet-5-5", formality: "auto", customPrices: {}, correctEngine: "deepl", improveStyle: "", live: true, position: "center" },
  ai: { enabled: true, defaultProvider: "claude", providers: [{ id: "claude", name: "Claude", kind: "anthropic", baseUrl: "https://api.anthropic.com", model: "claude-opus-5-5" }], actions: [{ id: "fix", name: "Korrigieren", prompt: "Fix", provider: "", builtin: true }] },
};

mockIPC((cmd, args: any) => {
  if (q.get("trace")) console.log("IPC", cmd, JSON.stringify(args ?? {}).slice(0, 120));
  switch (cmd) {
    case "get_settings": return settings;
    case "search": {
      let list = entries.map(summary);
      if (args.tab === "pinned") list = list.filter((e) => e.pinned);
      if (args.tab === "images") list = list.filter((e) => e.kind === "image");
      if (args.query) list = list.filter((e) => (e.title + " " + e.preview).toLowerCase().includes(String(args.query).toLowerCase()));
      return list;
    }
    case "get_detail": {
      const e = entries.find((x) => x.id === args.id)!;
      return { summary: summary(e), text: e.text ?? null, textTruncated: false, files: e.files ?? [], missingFiles: (e.files ?? []).filter((f) => f.endsWith(".svg")), image: e.kind === "image" ? png1 : null, ocrText: e.ocr ?? null };
    }
    case "get_thumb": return png1;
    case "missing_files": return [];
    case "picker_state": return { emojiRecent: ["👍", "🎉", "😂", "🙏🏽", "❤️", "🚀", "✅", "🤔"], symbolRecent: ["→", "€", "–", "„"], gifRecent: q.get("ptab") === "gif" ? Array.from({ length: 9 }, (_, i) => ({ provider: "giphy", id: "p" + i, sticker: false })) : [], favorites: ["☕", "✓"] };
    case "picker_favorite": return { emojiRecent: [], symbolRecent: [], gifRecent: [], favorites: [] };
    case "gif_recent": case "gif_search": {
      // Colourful placeholders (no real GIFs in screenshots)
      const pal = [["#ff6b6b", "#ffd166"], ["#4ecdc4", "#556270"], ["#a18cd1", "#fbc2eb"], ["#f093fb", "#f5576c"], ["#43e97b", "#38f9d7"], ["#fa709a", "#fee140"], ["#30cfd0", "#330867"], ["#667eea", "#764ba2"], ["#f6d365", "#fda085"]];
      return pal.map(([a, b], i) => {
        const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='200' height='${130 + (i % 3) * 30}'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='${a}'/><stop offset='1' stop-color='${b}'/></linearGradient></defs><rect width='100%' height='100%' fill='url(#g)'/><circle cx='${60 + i * 11}' cy='70' r='${22 + (i % 4) * 6}' fill='white' opacity='.35'/></svg>`;
        return { provider: "giphy", id: "p" + i, title: "GIF", sticker: false, preview: "data:image/svg+xml;utf8," + encodeURIComponent(svg), width: 200, height: 130 + (i % 3) * 30, gifUrl: "", ping: "" };
      });
    }
    case "thist_list": return [
      { id: 2, updatedAt: Date.now() - 600000, mode: "correct", text: D("Hallo Tom, können wir das Termin verschieben?", "Hi Tom, could we moved the meeting?"), out: D("Hallo Tom, können wir den Termin verschieben?", "Hi Tom, could we move the meeting?"), source: "DE", target: null, engine: "ai", kind: "correct", style: null },
      { id: 1, updatedAt: Date.now() - 90000000, mode: "translate", text: "Können wir den Termin auf Donnerstag verschieben?", out: "Could we move the meeting to Thursday?", source: "DE", target: "EN-US", engine: "deepl", kind: null, style: null },
    ];
    case "version_info": return { version: "1.1.0", buildDate: "2026-11-16", portable: false };
    case "update_info": return q.get("update") ? { current: "1.0.0", latest: "1.1.0", newer: true, notes: "- New: …\n- Fixed: …", published: "2026-12-01T10:00:00Z", page: "https://github.com/TheBigS1981/UltimatePaste/releases", asset: "x", assetUrl: "x", sumsUrl: "x", installed: true } : null;
    case "stats_get": {
      const days: Record<string, unknown> = {};
      for (let i = 0; i < 30; i++) {
        const d = new Date(); d.setDate(d.getDate() - i);
        const k = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
        const w = (i * 7) % 11;
        days[k] = { paste: 6 + w * 2, sequential: w % 3, translate: (w % 4) + 1, correct: w % 2, emoji: (w + 3) % 6, symbol: w % 2, gif: i % 5 === 0 ? 1 : 0 };
      }
      return { since: "2026-10-12", days, emoji: { "👍": 42, "😂": 31, "🙏": 18, "❤️": 15, "🎉": 12, "🤷‍♂️": 9, "😅": 7, "👀": 6, "🔥": 5, "✅": 4 }, symbols: { "→": 23, "€": 12, "–": 9, "©": 2 }, apps: { "outlook.exe": 212, "teams.exe": 143, "chrome.exe": 98, "winword.exe": 41, "code.exe": 17 }, pairs: { "DE → EN-US": 18420, "EN → DE": 7310, "FR → DE": 940 }, aiCost: { "2026-11": 0.42, "2026-10": 1.18 } };
    }
    case "notice_conflicts": return [{ id: 0, hotkey: "Super+V", kind: "windows-v", module: null, resolved: false, key: "k" }];
    case "hotkey_conflicts": return [{ id: 0, hotkey: "Super+V", kind: "windows-v", module: null, resolved: false }, { id: 2, hotkey: "Super+Period", kind: "windows-emoji", module: null, resolved: false }];
    case "translate_pending":
      return q.get("tab") === "correct"
        ? { text: D("Hallo Tom,\n\nkönnen wir das Termin auf Donnerstag verschieben? Ich schick dir forher noch die Unterlagen, damit du dich in ruhe vorbereiten kannst.\n\nViele Grüße\nAlex", "Hi Tom,\n\ncould we move the meeting to thursday? I'll send you the documents befor, so you can prepare in peace.\n\nBest regards\nAlex"), fromSelection: true, error: null, mode: "correct" }
        : { text: "Können wir den Termin auf Donnerstag 14 Uhr verschieben? Ich schicke dir vorher noch die Unterlagen.", fromSelection: true, error: null, mode: "translate" };
    case "improve_run": return { text: D("Hallo Tom,\n\nkönnen wir den Termin auf Donnerstag verschieben? Ich schicke dir vorher noch die Unterlagen, damit du dich in Ruhe vorbereiten kannst.\n\nViele Grüße\nAlex", "Hi Tom,\n\ncould we move the meeting to Thursday? I'll send you the documents beforehand, so you can prepare in peace.\n\nBest regards\nAlex"), source: D("DE", "EN"), target: D("DE", "EN-US"), engine: args.args.engine ?? "deepl", model: "claude-sonnet-5-5", tokensIn: 240, tokensOut: 52, costUsd: 0.001, deeplBilled: 412, deeplUsed: 12892, deeplLimit: 500000, via: "roundtrip", month: { month: "2026-11", aiTokensIn: 5400, aiTokensOut: 1200, aiCostUsd: 0.42, deeplChars: 12892 } };
    case "translate_run": if (q.get("tab") === "correct") return { text: D("Hallo Tom,\n\nkönnen wir den Termin auf Donnerstag verschieben? Ich schicke dir vorher noch die Unterlagen, damit du dich in Ruhe vorbereiten kannst.\n\nViele Grüße\nAlex", "Hi Tom,\n\ncould we move the meeting to Thursday? I'll send you the documents beforehand, so you can prepare in peace.\n\nBest regards\nAlex"), source: "DE", target: null, engine: "deepl", model: null, tokensIn: 0, tokensOut: 0, costUsd: null, deeplBilled: 120, deeplUsed: 12480, deeplLimit: 500000, month: { month: "2026-11", aiTokensIn: 5400, aiTokensOut: 1200, aiCostUsd: 0.42, deeplChars: 12480 } };
      return { text: "Could we move the meeting to Thursday at 2 pm? I'll send you the documents beforehand.", source: "DE", target: "EN-US", engine: args.args.engine ?? "deepl", model: "claude-sonnet-5-5", tokensIn: 182, tokensOut: 31, costUsd: 0.00067, deeplBilled: 104, deeplUsed: 12480, deeplLimit: 500000, month: { month: "2026-11", aiTokensIn: 5400, aiTokensOut: 1200, aiCostUsd: 0.42, deeplChars: 12480 } };
    case "translate_languages": return [["DE","Deutsch","German"],["EN-GB","Englisch (britisch)","English (British)"],["EN-US","Englisch (amerikanisch)","English (American)"],["FR","Französisch","French"],["ES","Spanisch","Spanish"],["IT","Italienisch","Italian"]].map(([code, de, en]) => ({ code, de, en }));
    case "translate_usage": return { month: { month: "2026-11", aiTokensIn: 5400, aiTokensOut: 1200, aiCostUsd: 0.42, deeplChars: 12480 }, deeplUsed: 12480, deeplLimit: 500000, deeplError: null };
    case "toast_text": return "DE → EN · 17 Wörter · 104 Zeichen";
    case "list_snippets": return snippets;
    case "app_info": return { version: "1.0.0", dataDir: "C:\\Users\\alex\\AppData\\Local\\UltimatePaste", entries: 128 };
    case "ai_has_key": return args.providerId === "svc:deepl" || (q.get("ptab") === "gif" && String(args.providerId).startsWith("svc:"));
    case "fill_job": return q.get("fill") === "multi"
      ? { fields: [{ name: D("Name", "Name"), default: "Tom", options: [] }, { name: D("Anrede", "Greeting"), default: D("Hallo", "Hi"), options: [D("Hallo", "Hi"), D("Liebe Grüße", "Dear")] }] }
      : { fields: [{ name: D("E-Mail-Adresse", "E-mail address"), default: "alex@northwind.studio", options: ["alex@northwind.studio", "alex.morgan@nordlicht.design", "a.morgan@example.org"] }] };
    case "fill_fit": return null;
    case "plugin:event|listen": return 1;
    default: return null;
  }
});

await import("../src/main");
if (q.get("sel")) {
  setTimeout(() => {
    for (let i = 0; i < Number(q.get("sel")); i++) window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
  }, 300);
}
// Actions menu (Ctrl+Enter) for the selected entry, optionally filtered (release notes).
if (q.get("actions") != null) {
  setTimeout(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", ctrlKey: true })), 700);
  setTimeout(() => {
    const el = document.activeElement as HTMLInputElement | null;
    if (el && el.tagName === "INPUT" && q.get("actions")) {
      el.value = q.get("actions")!;
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
  }, 1000);
}
// Fake desktop behind the transparent palette window.
if (win === "main") document.documentElement.style.background = "linear-gradient(135deg, #3d5a8a, #8e6aa8)";

// Snippet fields popup over a fake text editor (release notes / README).
if (win === "fill") {
  const de = !EN;
  const typed = q.get("fill") === "multi" ? ";follow" : ";mail";
  const line = q.get("fill") === "multi" ? "" : de ? "Hi Tom, schick die Unterlagen bitte an " : "Hi Tom, please send the documents to ";
  const bg = document.createElement("div");
  bg.innerHTML = `<div style="position:fixed;inset:0;background:linear-gradient(135deg,#3d5a8a,#8e6aa8);font:15px/1.5 'Segoe UI',system-ui,sans-serif">
    <div style="position:absolute;left:28px;top:24px;right:28px;bottom:24px;background:#fff;border-radius:10px;box-shadow:0 20px 50px -20px rgba(0,0,0,.6);overflow:hidden">
      <div style="height:34px;background:#f3f3f6;border-bottom:1px solid #e3e3e8;display:flex;align-items:center;padding:0 14px;font-size:12.5px;color:#666">${de ? "Neue Nachricht" : "New message"}</div>
      <div style="padding:18px 20px;color:#222">${line}<span style="background:rgba(91,76,240,.14);border-radius:3px;padding:0 2px">${typed}</span><span id="fake-caret" style="display:inline-block;width:1.5px;height:1.15em;background:#222;vertical-align:text-bottom"></span></div>
    </div></div>`;
  document.body.prepend(bg);
  const app = document.getElementById("app")!;
  // Like the app: the popup's top-left (16 px shadow margin) just below the caret.
  const r = document.getElementById("fake-caret")!.getBoundingClientRect();
  Object.assign(app.style, { position: "absolute", left: Math.round(r.left - 24) + "px", top: Math.round(r.bottom - 12) + "px", height: "auto" });
}

// Snippet editor with unsaved changes after Esc (release notes).
if (q.get("discard")) {
  setTimeout(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "e", ctrlKey: true })), 400);
  setTimeout(() => {
    const ta = document.querySelector(".card.editor textarea") as HTMLTextAreaElement | null;
    if (ta) {
      ta.value += EN ? "\nP.S. See you on Thursday!" : "\nPS: Bis Donnerstag!";
      ta.dispatchEvent(new Event("input", { bubbles: true }));
    }
  }, 800);
  setTimeout(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })), 1100);
}

