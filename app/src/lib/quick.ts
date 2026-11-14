// Quick actions that depend on what was copied (Q32): JSON, colors, case, links, e-mail,
// phone numbers, addresses and parcel numbers.
import { tr } from "./i18n.svelte";

export type QuickKind = "paste" | "open";
export interface Quick {
  id: string;
  label: string;
  kind: QuickKind;
  /** Text to paste or URL to open */
  value: string;
}

// ---------------------------------------------------------------- colors

type RGB = [number, number, number];

export function parseColor(s: string): RGB | null {
  const t = s.trim();
  let m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(t);
  if (m && (t.startsWith("#") || /[a-f]/i.test(m[1]))) {
    const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join("") : m[1];
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  }
  m = /^rgba?\(\s*(\d{1,3})\s*,?\s*(\d{1,3})\s*,?\s*(\d{1,3})\s*(?:[,/]\s*[\d.]+%?\s*)?\)$/i.exec(t);
  if (m) {
    const v = [Number(m[1]), Number(m[2]), Number(m[3])];
    return v.every((x) => x <= 255) ? (v as RGB) : null;
  }
  m = /^hsla?\(\s*(\d{1,3}(?:\.\d+)?)(?:deg)?\s*,?\s*(\d{1,3}(?:\.\d+)?)%\s*,?\s*(\d{1,3}(?:\.\d+)?)%\s*(?:[,/]\s*[\d.]+%?\s*)?\)$/i.exec(t);
  if (m) return hslToRgb(Number(m[1]), Number(m[2]) / 100, Number(m[3]) / 100);
  return null;
}

function hslToRgb(h: number, s: number, l: number): RGB {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)];
}

function rgbToHsl([r, g, b]: RGB): [number, number, number] {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B);
  const min = Math.min(R, G, B);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, Math.round(l * 100)];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === R ? (G - B) / d + (G < B ? 6 : 0) : max === G ? (B - R) / d + 2 : (R - G) / d + 4;
  return [Math.round(h * 60), Math.round(s * 100), Math.round(l * 100)];
}

export const toHex = (c: RGB) => "#" + c.map((x) => x.toString(16).padStart(2, "0")).join("").toUpperCase();
export const toRgb = (c: RGB) => `rgb(${c.join(", ")})`;
export const toHsl = (c: RGB) => {
  const [h, s, l] = rgbToHsl(c);
  return `hsl(${h}, ${s}%, ${l}%)`;
};

// ---------------------------------------------------------------- case

const titleCase = (s: string) => s.toLocaleLowerCase().replace(/(^|[\s\-–(„"'])(\p{L})/gu, (_, a, b) => a + b.toLocaleUpperCase());
const sentenceCase = (s: string) => s.toLocaleLowerCase().replace(/(^\s*|[.!?]\s+)(\p{L})/gu, (_, a, b) => a + b.toLocaleUpperCase());

// ---------------------------------------------------------------- detection

const EMAIL = /^[\w.+-]+@[\w-]+(\.[\w-]+)+$/;
const URL_RE = /^https?:\/\/\S+$/i;
const PHONE = /^\+?[\d\s/().-]{7,20}$/;
const POSTCODE_ADDR = /\b\d{4,5}\s+\p{L}[\p{L} .-]+/u;
const STREET = /\p{L}(str\.|straße|strasse|weg|gasse|platz|allee|ring|street|st\.|road|rd\.|avenue|ave\.|rue|via|calle|laan|ul\.)\s*\d/iu;

function parcelUrls(t: string): Quick[] {
  const s = t.replace(/\s+/g, "").toUpperCase();
  const out: Quick[] = [];
  const add = (id: string, name: string, url: string) => out.push({ id: `track-${id}`, kind: "open", label: tr("Sendung verfolgen: {0}", "Track parcel: {0}", name), value: url });
  if (/^1Z[0-9A-Z]{16}$/.test(s)) add("ups", "UPS", `https://www.ups.com/track?tracknum=${s}`);
  else if (/^(JJD|JVGL)\d{10,20}$/.test(s) || /^\d{12}$|^\d{20}$|^00\d{18}$/.test(s)) add("dhl", "DHL", `https://www.dhl.de/de/privatkunden/pakete-empfangen/verfolgen.html?piececode=${s}`);
  if (/^\d{14}$/.test(s)) add("dpd", "DPD", `https://tracking.dpd.de/status/de_DE/parcel/${s}`);
  if (/^\d{14}$|^\d{16}$|^H\d{19}$/.test(s)) add("hermes", "Hermes", `https://www.myhermes.de/empfangen/sendungsverfolgung/sendungsinformation#${s}`);
  if (/^\d{11,12}$|^[0-9A-Z]{8}$/.test(s) && /\d/.test(s)) add("gls", "GLS", `https://gls-group.com/DE/de/paketverfolgung?match=${s}`);
  return out;
}

/**
 * Quick actions for a copied text. `cleaned` is the link without trackers (from the backend),
 * if there is anything to remove.
 */
export function quickActions(text: string, cleaned: string | null = null): Quick[] {
  const t = text.trim();
  if (!t || t.length > 20000) return [];
  const out: Quick[] = [];
  const one = !t.includes("\n");

  if ((t.startsWith("{") || t.startsWith("[")) && t.length > 1) {
    try {
      const v = JSON.parse(t);
      out.push({ id: "json-pretty", kind: "paste", label: tr("JSON formatieren", "Format JSON"), value: JSON.stringify(v, null, 2) });
      out.push({ id: "json-min", kind: "paste", label: tr("JSON verkleinern", "Minify JSON"), value: JSON.stringify(v) });
    } catch {
      /* not JSON */
    }
  }

  const color = one ? parseColor(t) : null;
  if (color) {
    for (const [id, v] of [["hex", toHex(color)], ["rgb", toRgb(color)], ["hsl", toHsl(color)]] as const)
      if (v.replace(/\s/g, "").toLowerCase() !== t.replace(/\s/g, "").toLowerCase()) out.push({ id: `color-${id}`, kind: "paste", label: tr("Als {0} einfügen", "Paste as {0}", v), value: v });
    return out;
  }

  if (one && URL_RE.test(t)) {
    if (cleaned) out.push({ id: "clean-link", kind: "paste", label: tr("Ohne Tracking einfügen", "Paste without tracking"), value: cleaned });
    out.push({ id: "open-link", kind: "open", label: tr("Im Browser öffnen", "Open in browser"), value: t });
    return out;
  }
  if (one && EMAIL.test(t)) out.push({ id: "mail", kind: "open", label: tr("E-Mail schreiben", "Write e-mail"), value: `mailto:${t}` });
  const phoneDigits = t.replace(/\D/g, "").length;
  if (one && PHONE.test(t) && (t.startsWith("+") || t.startsWith("0")) && phoneDigits >= 7 && phoneDigits <= 15) out.push({ id: "call", kind: "open", label: tr("Anrufen", "Call"), value: `tel:${t.replace(/[^\d+]/g, "")}` });
  if (one && /^[0-9A-Z\s]{8,25}$/i.test(t)) out.push(...parcelUrls(t));
  const lines = t.split(/\r?\n/).length;
  if (lines <= 4 && t.length <= 160 && (STREET.test(t) || POSTCODE_ADDR.test(t)) && !URL_RE.test(t))
    out.push({ id: "maps", kind: "open", label: tr("In Karten öffnen", "Open in maps"), value: `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(t.replace(/\s*\n\s*/g, ", "))}` });

  if (/\p{L}/u.test(t) && !EMAIL.test(t)) {
    const variants: [string, string, string][] = [
      ["upper", tr("GROSSBUCHSTABEN", "UPPERCASE"), t.toLocaleUpperCase()],
      ["lower", tr("kleinbuchstaben", "lowercase"), t.toLocaleLowerCase()],
      ["sentence", tr("Satzanfang groß", "Sentence case"), sentenceCase(t)],
      ["title", tr("Jedes Wort Groß", "Title Case"), titleCase(t)],
    ];
    for (const [id, label, v] of variants) if (v !== t) out.push({ id: `case-${id}`, kind: "paste", label, value: v });
  }
  return out;
}
