import { mount, type Component } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./app.css";
import "./designs.css";
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-600.css";
import "@fontsource/plus-jakarta-sans/latin-400.css";
import "@fontsource/plus-jakarta-sans/latin-600.css";
import "@fontsource/plus-jakarta-sans/latin-700.css";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-600.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-600.css";

const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;

// Block the browser context menu and reload shortcuts – this is an app, not a page.
window.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement;
  if (!el.closest("input, textarea")) e.preventDefault();
});
window.addEventListener("keydown", (e) => {
  if (e.key === "F5" || (e.ctrlKey && e.key.toLowerCase() === "r")) e.preventDefault();
});

// WebView2 freezes hidden pages after ~5 minutes and rebuilds them on the next show (slow,
// visible flicker). A lock that is never released keeps the page alive; it costs nothing.
try {
  navigator.locks?.request("ultimatepaste-keep-alive", () => new Promise<void>(() => {}));
} catch {
  /* not available */
}

// Each window loads only its own view (keeps e.g. the emoji data out of the palette).
const views: Record<string, () => Promise<{ default: Component }>> = {
  settings: () => import("./lib/Settings.svelte"),
  picker: () => import("./lib/Picker.svelte"),
  translate: () => import("./lib/Translate.svelte"),
  toast: () => import("./lib/Toast.svelte"),
  notice: () => import("./lib/Notice.svelte"),
  tour: () => import("./lib/Tour.svelte"),
  update: () => import("./lib/Update.svelte"),
};
const target = document.getElementById("app")!;
(views[label] ?? (() => import("./lib/Palette.svelte")))().then((m) => mount(m.default, { target }));
