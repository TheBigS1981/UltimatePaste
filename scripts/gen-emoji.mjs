// Builds app/src/lib/picker/emoji.json from emojibase-data (MIT): German + English labels and
// tags, groups, and the five skin-tone variants. Run inside the builder container:
//   docker compose run --rm -T -w /work/app builder node ../scripts/gen-emoji.mjs
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";

const require = createRequire(new URL("../app/package.json", import.meta.url));
const de = require("emojibase-data/de/compact.json");
const en = require("emojibase-data/en/compact.json");
const deMsg = require("emojibase-data/de/messages.json");
const enMsg = require("emojibase-data/en/messages.json");
const version = require("emojibase-data/package.json").version;

const enByHex = new Map(en.map((e) => [e.hexcode, e]));
const TONES = ["1F3FB", "1F3FC", "1F3FD", "1F3FE", "1F3FF"];

const groups = deMsg.groups
  .filter((g) => g.key !== "component")
  .map((g) => ({ id: g.order, key: g.key, de: g.message, en: enMsg.groups.find((x) => x.order === g.order)?.message ?? g.key }));

const out = [];
for (const e of de.sort((a, b) => (a.order ?? 0) - (b.order ?? 0))) {
  if (e.group === undefined || e.group === 2) continue; // components (bare skin tones, hair)
  const x = enByHex.get(e.hexcode);
  const keys = [...new Set([...(e.tags ?? []), ...(x?.tags ?? []), ...(x?.shortcodes ?? [])].map((t) => t.toLowerCase()))].join(" ");
  let skins;
  if (e.skins) {
    // One variant per tone; for multi-person emoji use the "same tone for everyone" variant.
    skins = TONES.map((t) => {
      const one = e.skins.find((s) => s.hexcode.split("-").filter((p) => TONES.includes(p)).every((p) => p === t));
      return one ? one.unicode : null;
    });
    if (skins.some((s) => !s)) skins = undefined;
  }
  const row = [e.unicode, e.label, x?.label ?? "", keys, e.group];
  if (skins) row.push(skins);
  out.push(row);
}

const data = { source: `emojibase-data ${version} (MIT)`, groups, emoji: out };
writeFileSync(new URL("../app/src/lib/picker/emoji.json", import.meta.url), JSON.stringify(data));
console.log(`${out.length} emoji, ${groups.length} groups`);

// Names, keywords and group titles for the other UI languages, loaded on demand by the picker.
const deByHex = new Set(out.map((r) => r[0]));
for (const lang of ["it", "es", "fr", "nl", "pl", "pt", "vi"]) {
  const list = require(`emojibase-data/${lang}/compact.json`);
  const msg = require(`emojibase-data/${lang}/messages.json`);
  const emoji = {};
  for (const e of list) {
    if (!deByHex.has(e.unicode)) continue;
    emoji[e.unicode] = [e.label, (e.tags ?? []).join(" ").toLowerCase()];
  }
  const groupNames = Object.fromEntries(msg.groups.filter((g) => g.key !== "component").map((g) => [g.order, g.message]));
  writeFileSync(new URL(`../app/src/lib/picker/names/${lang}.json`, import.meta.url), JSON.stringify({ groups: groupNames, emoji }));
  console.log(`${lang}: ${Object.keys(emoji).length} names`);
}
