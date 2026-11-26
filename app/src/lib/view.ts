import type { Quick } from "./quick";
import type { EntryDetail, EntrySummary, Settings, Snippet, Tab } from "./api";
import { locale, t } from "./i18n.svelte";

export type ViewTab = Tab | "snippets";
export type Layout = "fluent" | "board" | "drawer" | "compact";
export const TABS: ViewTab[] = ["all", "pinned", "text", "images", "snippets"];

/** Everything a layout needs to render; owned by Palette.svelte. */
export interface ViewModel {
  tab: ViewTab;
  items: EntrySummary[];
  snippets: Snippet[];
  sel: number;
  detail: EntryDetail | null;
  settings: Settings | null;
  thumbs: Record<number, string>;
  /** File entries whose files no longer exist. */
  missing: Record<number, boolean>;
  toast: string | null;
  query: string;
  /** Quick actions for the selected entry (Q32) */
  quick: Quick[];
  /** Entry ids marked for pasting together, in order (Q34) */
  marked: number[];
}

/** Callbacks a layout may invoke. */
export interface ViewActions {
  select: (i: number, e?: MouseEvent) => void;
  activate: (i: number) => void;
  setTab: (t: ViewTab) => void;
  run: (id: "paste" | "plain" | "copy" | "pin" | "actions" | "ai" | "settings" | "edit" | "new" | "del" | "view-image") => void;
  quick: (q: Quick) => void;
}

export function tabLabel(tb: ViewTab): string {
  return t(tb === "all" ? "tabAll" : tb === "pinned" ? "tabPinned" : tb === "text" ? "tabText" : tb === "images" ? "tabImages" : "tabSnippets");
}

export function appName(exe: string | null): string {
  if (!exe) return "";
  const base = exe.replace(/\.exe$/i, "");
  const known: Record<string, string> = {
    outlook: "Outlook",
    olk: "Outlook",
    excel: "Excel",
    winword: "Word",
    powerpnt: "PowerPoint",
    chrome: "Chrome",
    msedge: "Edge",
    firefox: "Firefox",
    explorer: "Explorer",
    windowsterminal: "Terminal",
    code: "VS Code",
    notepad: "Editor",
    teams: "Teams",
    "ms-teams": "Teams",
    ultimatepaste: "UltimatePaste",
    greenshot: "Greenshot",
    snippingtool: "Snipping Tool",
  };
  return known[base.toLowerCase()] ?? base.charAt(0).toUpperCase() + base.slice(1);
}

const APP_COLORS: Record<string, string> = {
  outlook: "#0f6cbd",
  olk: "#0f6cbd",
  excel: "#107c41",
  winword: "#185abd",
  powerpnt: "#c43e1c",
  chrome: "#e8a200",
  msedge: "#0c8ed9",
  firefox: "#e66000",
  explorer: "#d9a400",
  windowsterminal: "#2b2b2b",
  code: "#007acc",
  teams: "#5b5fc7",
  "ms-teams": "#5b5fc7",
  greenshot: "#3a7d44",
};

/** Stable color per source app (known brands, otherwise hashed). */
export function appColor(exe: string | null): string {
  if (!exe) return "#8a8f98";
  const base = exe.replace(/\.exe$/i, "").toLowerCase();
  if (APP_COLORS[base]) return APP_COLORS[base];
  let h = 0;
  for (const c of base) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return `hsl(${h % 360} 55% 45%)`;
}

export function appInitial(exe: string | null): string {
  const n = appName(exe);
  return n ? n.charAt(0).toUpperCase() : "·";
}

export function entryLabel(it: EntrySummary): string {
  return it.title || contentLabel(it);
}

/** What the entry contains, ignoring a name the user gave it. */
export function contentLabel(it: EntrySummary): string {
  if (it.kind === "image") return `${t("image")} ${it.imageW ?? "?"} × ${it.imageH ?? "?"}`;
  return it.preview || "—";
}

/** Hex color like #5B4CF0 → shown as a swatch. */
export function colorValue(it: EntrySummary): string | null {
  const m = it.preview.trim().match(/^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i);
  return m ? m[0] : null;
}

export function looksLikeCode(text: string): boolean {
  return /[{};$=<>]|^\s*(\$|>|sudo |docker |git |npm |cd )/m.test(text) && !/[.!?]\s*$/.test(text.trim());
}

export function timeOf(ms: number): string {
  return new Date(ms).toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });
}

export function dayLabel(ms: number): string {
  const d = new Date(ms);
  const today = new Date();
  const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((start(today) - start(d)) / 86400000);
  if (diff === 0) return t("today");
  if (diff === 1) return t("yesterday");
  return d.toLocaleDateString(locale(), { weekday: diff < 7 ? "long" : undefined, day: "numeric", month: "long" });
}

/** Groups items by day of last use (only meaningful without a search query). */
export function groupByDay(items: EntrySummary[]): { label: string; rows: { it: EntrySummary; i: number }[] }[] {
  const groups: { label: string; rows: { it: EntrySummary; i: number }[] }[] = [];
  items.forEach((it, i) => {
    const label = dayLabel(it.lastUsedAt);
    const last = groups[groups.length - 1];
    if (last && last.label === label) last.rows.push({ it, i });
    else groups.push({ label, rows: [{ it, i }] });
  });
  return groups;
}

export function formatsLabel(formats: string[]): string {
  const names: Record<string, string> = { text: "Text", html: "HTML", rtf: "RTF", files: t("files"), image: t("image") };
  return formats.map((f) => names[f] ?? f).join(" · ");
}

const DEFAULT_ACTIONS_KEY = "Ctrl+Enter";

function splitCombo(combo: string): { ctrl: boolean; alt: boolean; shift: boolean; key: string } {
  const parts = (combo || DEFAULT_ACTIONS_KEY).split("+").map((p) => p.trim());
  const has = (m: string) => parts.some((p) => p.toLowerCase() === m);
  return { ctrl: has("ctrl"), alt: has("alt"), shift: has("shift"), key: parts[parts.length - 1] };
}

/** Whether a keydown matches an in-window shortcut like "Ctrl+Enter" (format of HotkeyPicker). */
export function matchesShortcut(e: KeyboardEvent, combo: string): boolean {
  const c = splitCombo(combo);
  if (e.ctrlKey !== c.ctrl || e.altKey !== c.alt || e.shiftKey !== c.shift || e.metaKey) return false;
  return e.code === c.key || e.code === `Key${c.key}` || e.code === `Digit${c.key}` || (c.key === "Enter" && e.code === "NumpadEnter");
}

/** Short display form, e.g. "Strg ↵". */
export function shortcutLabel(combo: string): string {
  const c = splitCombo(combo);
  const keys: Record<string, string> = { Enter: "↵", Space: t("spaceShort"), Period: ".", Comma: ",", Backquote: "^" };
  return [c.ctrl && t("ctrlShort"), c.alt && "Alt", c.shift && "⇧", keys[c.key] ?? c.key].filter(Boolean).join(" ");
}

/**
 * Reopening a popup within this time continues where you left off (tab, search, selection);
 * later it starts fresh. Same rule for every window.
 */
export const GRACE_MS = 10_000;
let lastLeft = 0;
if (typeof window !== "undefined") window.addEventListener("blur", () => (lastLeft = Date.now()));
export const withinGrace = () => lastLeft > 0 && Date.now() - lastLeft < GRACE_MS;

/** Alt+1…9 selects a tab – the same in every window (Ctrl+1…9 picks items). */
export function tabByAlt(e: KeyboardEvent): number | null {
  if (!e.altKey || e.ctrlKey || e.metaKey) return null;
  const m = /^Digit([1-9])$/.exec(e.code);
  return m ? Number(m[1]) - 1 : null;
}
