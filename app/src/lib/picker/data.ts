// Emoji and symbol data for the picker, plus a small search over German and English terms.
import emojiData from "./emoji.json";
import symbolData from "./symbols.json";

export interface Emoji {
  char: string;
  de: string;
  en: string;
  /** Lower-case search haystack: labels, tags, shortcodes */
  hay: string;
  group: number;
  /** Variants for skin tones 1..5 */
  skins?: string[];
  /** Name in the current UI language (other than German / English) */
  local?: string;
}

export interface Group {
  id: number;
  key: string;
  de: string;
  en: string;
}

export interface Sym {
  char: string;
  en: string;
  de: string;
  hay: string;
  cat: string;
}

export interface SymbolCat {
  key: string;
  de: string;
  en: string;
}

type EmojiRow = [string, string, string, string, number, string[]?];

export const EMOJI_SOURCE: string = emojiData.source;
export const GROUPS: Group[] = emojiData.groups;
export const EMOJI: Emoji[] = (emojiData.emoji as EmojiRow[]).map(([char, de, en, keys, group, skins]) => ({
  char,
  de,
  en,
  hay: `${de} ${en} ${keys}`.toLowerCase(),
  group,
  skins,
}));

/**
 * Windows' emoji font lags behind Unicode: newer emoji show up as an empty box, newer ZWJ
 * sequences as several glyphs side by side. Both are detected by drawing on a canvas and
 * hidden from the picker. Skipped where the reference emoji itself does not render
 * (e.g. headless preview without an emoji font).
 */
function renderable(): (s: string) => boolean {
  try {
    const c = document.createElement("canvas");
    c.width = 64;
    c.height = 24;
    const ctx = c.getContext("2d", { willReadFrequently: true });
    if (!ctx) return () => true;
    ctx.font = '18px "Segoe UI Emoji", "Segoe UI Symbol", sans-serif';
    ctx.textBaseline = "top";
    const pixels = (s: string) => {
      ctx.clearRect(0, 0, c.width, c.height);
      ctx.fillText(s, 0, 2);
      return ctx.getImageData(0, 0, c.width, c.height).data;
    };
    const same = (a: Uint8ClampedArray, b: Uint8ClampedArray) => {
      for (let i = 3; i < a.length; i += 4) if (a[i] !== b[i]) return false;
      return true;
    };
    const tofu = pixels("\u{10FFFD}");
    const ref = pixels("😀");
    const refWidth = ctx.measureText("😀").width;
    if (same(ref, tofu) || refWidth <= 0) return () => true;
    return (s: string) => !same(pixels(s), tofu) && ctx.measureText(s).width < refWidth * 1.5;
  } catch {
    return () => true;
  }
}

let shown: Emoji[] | null = null;
/** Emoji that this system can display (computed once). */
export function visibleEmoji(): Emoji[] {
  if (shown) return shown;
  const ok = renderable();
  shown = EMOJI.filter((e) => ok(e.char));
  for (const e of shown) if (e.skins && !e.skins.every(ok)) e.skins = undefined;
  return shown;
}

export const SYMBOL_CATS: SymbolCat[] = (symbolData as { key: string; de: string; en: string }[]).map(({ key, de, en }) => ({ key, de, en }));
export const SYMBOLS: Sym[] = (symbolData as unknown as { key: string; items: [string, string, string][] }[]).flatMap((c) =>
  c.items.map(([char, en, de]) => ({ char, en, de, cat: c.key, hay: `${de} ${en} ${codepoint(char).toLowerCase()}`.toLowerCase() })),
);

// Names and keywords for the other UI languages, loaded on demand (one file per language).
const NAME_FILES = import.meta.glob<{ groups: Record<string, string>; emoji: Record<string, [string, string]> }>("./names/*.json", { import: "default" });
let localGroups: Record<string, string> = {};
let loadedLang = "";

/** Loads emoji names for a UI language other than de/en (adds them to the search). */
export async function loadEmojiNames(lang: string): Promise<void> {
  if (lang === loadedLang || lang === "de" || lang === "en") return;
  const f = NAME_FILES[`./names/${lang}.json`];
  if (!f) return;
  const d = await f();
  loadedLang = lang;
  localGroups = d.groups;
  for (const e of EMOJI) {
    const n = d.emoji[e.char];
    e.local = n?.[0];
    e.hay = `${e.de} ${e.en} ${n ? `${n[0]} ${n[1]}` : ""} ${e.hay}`.toLowerCase();
  }
}

export function emojiName(e: Emoji, lang: string): string {
  return lang === "de" ? e.de : lang === "en" ? e.en : (e.local ?? e.en);
}

export function groupLabel(g: Group, lang: string): string {
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  return lang === "de" ? g.de : lang === "en" ? cap(g.en) : cap(localGroups[String(g.id)] ?? g.en);
}

/** "U+2192" style code point(s). */
export function codepoint(s: string): string {
  return [...s].map((c) => "U+" + c.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")).join(" ");
}

const SYMBOL_SET = new Set(SYMBOLS.map((s) => s.char));
export const isSymbol = (s: string) => SYMBOL_SET.has(s);

/** Emoji by any of its variants (base or skin tone). */
const BY_CHAR = new Map<string, Emoji>();
for (const e of EMOJI) {
  BY_CHAR.set(e.char, e);
  e.skins?.forEach((s) => BY_CHAR.set(s, e));
}
export const emojiOf = (s: string) => BY_CHAR.get(s);

/** Variant for a skin tone (0 = default yellow). */
export function withTone(e: Emoji, tone: number): string {
  return tone > 0 && e.skins ? e.skins[tone - 1] : e.char;
}

function subsequence(needle: string, hay: string): boolean {
  let i = 0;
  for (const ch of hay) if (ch === needle[i] && ++i === needle.length) return true;
  return false;
}

/**
 * Every word of the query must appear in the haystack. Results whose label starts with the
 * query come first. Without substring hits, a fuzzy (subsequence) match is used.
 */
export function search<T extends { hay: string; de: string; en: string }>(items: T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  if (!q) return items;
  const words = q.split(/\s+/);
  const hits = items.filter((it) => words.every((w) => it.hay.includes(w)));
  const score = (it: T) => (it.de.toLowerCase().startsWith(q) || it.en.toLowerCase().startsWith(q) ? 0 : 1);
  if (hits.length) return hits.map((it, i) => ({ it, i, s: score(it) })).sort((a, b) => a.s - b.s || a.i - b.i).map((x) => x.it);
  if (q.length < 3) return [];
  return items.filter((it) => subsequence(q.replace(/\s+/g, ""), it.de.toLowerCase()) || subsequence(q.replace(/\s+/g, ""), it.en.toLowerCase())).slice(0, 80);
}

/** Symbol search also understands "U+2192", "u2192" or "2192". */
export function searchSymbols(query: string): Sym[] {
  const q = query.trim();
  const m = q.match(/^(?:u\+?)?([0-9a-f]{4,6})$/i);
  if (m) {
    const cp = parseInt(m[1], 16);
    const known = SYMBOLS.filter((s) => s.char.codePointAt(0) === cp);
    if (known.length) return known;
    if (cp <= 0x10ffff) {
      const char = String.fromCodePoint(cp);
      return [{ char, en: codepoint(char), de: codepoint(char), hay: "", cat: "" }];
    }
  }
  return search(SYMBOLS, q);
}
