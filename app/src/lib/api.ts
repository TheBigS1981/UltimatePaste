import { tr, type Lang } from "./i18n.svelte";
import { invoke } from "@tauri-apps/api/core";

export type Kind = "text" | "link" | "image" | "files";
export type Tab = "all" | "pinned" | "text" | "images";
export type PasteMode = "default" | "plain" | "html" | "rtf" | "copy";
export type Delivery = "pasted" | "copied" | "copiedElevated";

export interface EntrySummary {
  id: number;
  kind: Kind;
  title: string | null;
  preview: string;
  sourceApp: string | null;
  createdAt: number;
  lastUsedAt: number;
  pinned: boolean;
  formats: string[];
  imageW: number | null;
  imageH: number | null;
  fileCount: number;
  byteSize: number;
  hasOcr: boolean;
}

export interface EntryDetail {
  summary: EntrySummary;
  text: string | null;
  textTruncated: boolean;
  files: string[];
  /** Paths from `files` that no longer exist. */
  missingFiles: string[];
  image: string | null;
  ocrText: string | null;
}

export interface Snippet {
  id: number;
  name: string;
  keyword: string;
  body: string;
  updatedAt: number;
}

export interface AiProvider {
  id: string;
  name: string;
  kind: "anthropic" | "openai";
  baseUrl: string;
  model: string;
}

export interface AiAction {
  id: string;
  name: string;
  prompt: string;
  provider: string;
  builtin: boolean;
}

export interface Settings {
  language: Lang;
  theme: "system" | "light" | "dark";
  layout: "fluent" | "board" | "drawer" | "compact";
  design: string;
  hotkey: string;
  sequentialHotkey: string;
  position: "center" | "cursor";
  autostart: boolean;
  paused: boolean;
  retentionDays: number | null;
  maxTextMb: number;
  maxImageMb: number;
  preferPlainText: boolean;
  primaryAction: "paste" | "copy";
  moveUsedToTop: boolean;
  blockedApps: string[];
  ocrEnabled: boolean;
  inboxDir: string | null;
  inboxTags: string[];
  sequentialTimeoutSecs: number;
  sequentialNewline: boolean;
  ai: { enabled: boolean; providers: AiProvider[]; defaultProvider: string; actions: AiAction[] };
  /** In-window shortcut for the actions menu, e.g. "Ctrl+Enter". */
  actionsKey: string;
  pickerHotkey: string;
  translateHotkey: string;
  translateAltHotkey: string;
  correctHotkey: string;
  notifiedConflicts: string[];
  appRules: AppRule[];
  sensitive: { otp: boolean; cards: boolean; keys: boolean; iban: boolean; minutes: number };
  cleanLinks: boolean;
  snippetExpand: boolean;
  tourDone: boolean;
  updateCheck: boolean;
  picker: PickerSettings;
  translate: TranslateSettings;
}

export interface PickerSettings {
  recentCount: number;
  skinTone: number;
  gifEnabled: boolean;
  gifProvider: "giphy" | "klipy";
  gifRating: "g" | "pg" | "pg-13" | "r";
  gifOriginalSize: boolean;
  gifFeedback: boolean;
}

export interface TranslateSettings {
  mode: "window" | "direct";
  rule: "pair" | "fixed";
  primary: string;
  secondary: string;
  fixedTarget: string;
  engine: "deepl" | "ai";
  aiProvider: string;
  aiModel: string;
  formality: "auto" | "less" | "more";
  customPrices: Record<string, [number, number]>;
  correctEngine: "deepl" | "ai";
  improveStyle: string;
  live: boolean;
}

export interface GifRef {
  provider: string;
  id: string;
  sticker: boolean;
}

export interface PickerState {
  emojiRecent: string[];
  symbolRecent: string[];
  gifRecent: GifRef[];
  favorites: string[];
}

export interface GifItem {
  provider: "giphy" | "klipy";
  id: string;
  title: string;
  sticker: boolean;
  preview: string;
  width: number;
  height: number;
  gifUrl: string;
  ping: string;
}

export interface MonthUsage {
  month: string;
  aiTokensIn: number;
  aiTokensOut: number;
  aiCostUsd: number;
  deeplChars: number;
}

export interface TranslateOutcome {
  text: string;
  source: string | null;
  target: string;
  engine: "deepl" | "ai";
  model: string | null;
  tokensIn: number;
  tokensOut: number;
  costUsd: number | null;
  deeplBilled: number | null;
  deeplUsed: number | null;
  deeplLimit: number | null;
  month: MonthUsage;
  via: "write" | "roundtrip" | null;
}

export interface Conflict {
  id: number;
  hotkey: string;
  kind: "windows-v" | "windows-emoji" | "powertoys";
  module: string | null;
  resolved: boolean;
  key: string;
}

export interface SnippetField {
  name: string;
  default: string;
  options: string[];
}

export interface AppRule {
  app: string;
  mode: "plain" | "formatted" | "oneline" | "trim";
}

export interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
  notes: string;
  published: string;
  page: string;
  asset: string;
  assetUrl: string;
  sumsUrl: string;
  installed: boolean;
}

export interface TransferInfo {
  app: string;
  exported: string;
  snippets: number;
  hasKeys: boolean;
}

export interface TranslateSession {
  id: number;
  updatedAt: number;
  mode: "translate" | "correct";
  text: string;
  out: string;
  source: string | null;
  target: string | null;
  engine: string;
  kind: string | null;
  style: string | null;
}

export interface StatsDay {
  paste: number;
  sequential: number;
  translate: number;
  correct: number;
  emoji: number;
  symbol: number;
  gif: number;
}

export interface Stats {
  since: string;
  days: Record<string, StatsDay>;
  emoji: Record<string, number>;
  symbols: Record<string, number>;
  apps: Record<string, number>;
  pairs: Record<string, number>;
  aiCost: Record<string, number>;
}

export interface Language {
  code: string;
  de: string;
  en: string;
}

export type PasteResult = { delivery: Delivery; missing?: string[] };

/** Keys of the external services in Windows Credential Manager. */
export const SERVICE_KEYS = { giphy: "svc:giphy", klipy: "svc:klipy", deepl: "svc:deepl" } as const;

export const api = {
  search: (query: string, tab: Tab) => invoke<EntrySummary[]>("search", { query, tab }),
  detail: (id: number) => invoke<EntryDetail>("get_detail", { id }),
  thumb: (id: number) => invoke<string | null>("get_thumb", { id }),
  paste: (id: number, mode: PasteMode) => invoke<{ delivery: Delivery; missing?: string[] }>("paste_entry", { id, mode }),
  missingFiles: (ids: number[]) => invoke<number[]>("missing_files", { ids }),
  pasteText: (text: string, save: boolean, copyOnly = false) => invoke<{ delivery: Delivery }>("paste_text", { text, save, copyOnly }),
  hide: (restoreFocus = true) => invoke<void>("hide_palette", { restoreFocus }),
  togglePin: (id: number) => invoke<boolean>("toggle_pin", { id }),
  rename: (id: number, title: string | null) => invoke<void>("rename_entry", { id, title }),
  updateText: (id: number, text: string) => invoke<number>("update_text", { id, text }),
  remove: (id: number) => invoke<void>("delete_entry", { id }),
  bulkDelete: (minutes: number | null) => invoke<number>("bulk_delete", { minutes }),
  copyOcr: (id: number) => invoke<string>("copy_ocr", { id }),
  runOcr: (id: number) => invoke<string>("run_ocr", { id }),
  exportInbox: (id: number) => invoke<string[]>("export_inbox", { id }),
  snippets: (query: string) => invoke<Snippet[]>("list_snippets", { query }),
  saveSnippet: (snippet: Snippet) => invoke<number>("save_snippet", { snippet }),
  deleteSnippet: (id: number) => invoke<void>("delete_snippet", { id }),
  pasteSnippet: (id: number, copyOnly = false, values: Record<string, string> | null = null) => invoke<{ delivery: Delivery }>("paste_snippet", { id, copyOnly, values }),
  takeSnippetForm: () => invoke<number | null>("take_snippet_form"),
  snippetFields: (id: number) => invoke<SnippetField[]>("snippet_fields", { id }),
  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<{ hotkeyErrors: string[]; hotkey: string; sequentialHotkey: string; pickerHotkey: string; translateHotkey: string; translateAltHotkey: string; correctHotkey: string }>("save_settings", { settings }),
  setPaused: (paused: boolean) => invoke<void>("set_paused", { paused }),
  openSettings: () => invoke<void>("open_settings"),
  appInfo: () => invoke<{ version: string; dataDir: string; entries: number }>("app_info"),
  aiRun: (actionId: string, id: number | null, text: string | null = null) => invoke<string>("ai_run", { args: { actionId, id, text } }),
  aiSetKey: (providerId: string, key: string | null) => invoke<void>("ai_set_key", { providerId, key }),
  aiHasKey: (providerId: string) => invoke<boolean>("ai_has_key", { providerId }),
  pickerState: () => invoke<PickerState>("picker_state"),
  pickerPaste: (kind: "emoji" | "symbol", items: string[], copyOnly = false) => invoke<PasteResult>("picker_paste", { kind, items, copyOnly }),
  pickerFavorite: (value: string, on: boolean) => invoke<PickerState>("picker_favorite", { value, on }),
  gifSearch: (provider: string, q: string, sticker: boolean, page: number) => invoke<GifItem[]>("gif_search", { query: { provider, q, sticker, page } }),
  gifRecent: () => invoke<GifItem[]>("gif_recent"),
  gifPaste: (item: GifItem, query: string, copyOnly = false) => invoke<PasteResult>("gif_paste", { item, query, copyOnly }),
  translatePending: () => invoke<{ text: string; fromSelection: boolean; error: string | null; mode: "translate" | "correct"; restore: boolean } | null>("translate_pending"),
  improveRun: (args: { text: string; mode: "correct" | "rephrase"; style?: string | null; engine?: string | null; lang?: string | null }) => invoke<TranslateOutcome>("improve_run", { args }),
  translateRun: (args: { text: string; source?: string | null; target?: string | null; engine?: string | null; formality?: string | null }) =>
    invoke<TranslateOutcome>("translate_run", { args }),
  translateApply: (text: string, copyOnly = false, stat?: { kind: "translate" | "correct"; pair?: string; chars?: number }) =>
    invoke<PasteResult>("translate_apply", { text, copyOnly, kind: stat?.kind ?? null, pair: stat?.pair ?? null, chars: stat?.chars ?? null }),
  thistList: () => invoke<TranslateSession[]>("thist_list"),
  thistSave: (session: TranslateSession) => invoke<void>("thist_save", { session }),
  thistClear: () => invoke<void>("thist_clear"),
  settingsExport: (path: string, password: string | null) => invoke<{ snippets: number; keys: number }>("settings_export", { path, password }),
  settingsInspect: (path: string) => invoke<TransferInfo>("settings_inspect", { path }),
  settingsImport: (path: string, password: string | null, replace: boolean) =>
    invoke<{ report: { snippets: number; keys: number }; save: { hotkeyErrors: string[] } }>("settings_import", { path, password, replace }),
  versionInfo: () => invoke<{ version: string; buildDate: string; portable: boolean }>("version_info"),
  updateCheck: () => invoke<UpdateInfo>("update_check"),
  updateInfo: () => invoke<UpdateInfo | null>("update_info"),
  openUpdate: () => invoke<void>("open_update"),
  updateInstall: () => invoke<void>("update_install"),
  statsGet: () => invoke<Stats>("stats_get"),
  statsReset: () => invoke<void>("stats_reset"),
  languages: () => invoke<Language[]>("translate_languages"),
  translateUsage: () => invoke<{ month: MonthUsage; deeplUsed: number | null; deeplLimit: number | null; deeplError: string | null }>("translate_usage"),
  toastText: () => invoke<string>("toast_text"),
  hotkeyConflicts: () => invoke<Conflict[]>("hotkey_conflicts"),
  setWindowsV: (off: boolean) => invoke<Conflict[]>("set_windows_v", { off }),
  cleanLink: (text: string) => invoke<string | null>("clean_link", { text }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  recentApps: () => invoke<string[]>("recent_apps"),
  openTour: () => invoke<void>("open_tour"),
  finishTour: () => invoke<void>("finish_tour"),
  openData: (which: "data" | "log") => invoke<void>("open_data", { which }),
  openLink: (url: string) => invoke<void>("open_link", { url }),
  noticeConflicts: () => invoke<Conflict[]>("notice_conflicts"),
  dismissConflicts: (keys: string[]) => invoke<void>("dismiss_conflicts", { keys }),
  translateEntry: (id: number, mode: "translate" | "correct" = "translate") => invoke<void>("translate_entry", { id, mode }),
  detectLanguage: (text: string) => invoke<string | null>("detect_language", { text }),
};

export function errText(e: unknown): string {
  const msg = typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
  // Error codes from the backend that read the same everywhere.
  if (msg === "insecure-url")
    return tr("Der API-Schlüssel wird nicht unverschlüsselt (http://) an einen anderen Rechner geschickt. Bitte eine https://-Adresse eintragen.", "The API key is not sent unencrypted (http://) to another computer. Please use an https:// address.");
  if (msg === "no-provider")
    return tr("Kein KI-Anbieter gefunden. Lege unter Einstellungen → KI einen Anbieter an und wähle ihn als Standard.", "No AI provider found. Add one under Settings → AI and make it the default.");
  return msg;
}
