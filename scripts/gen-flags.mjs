// Copies the few flag SVGs the language pickers need from flag-icons (MIT) into
// app/src/lib/flags/. Run inside the builder container:
//   docker compose run --rm -T -w /work/app builder node ../scripts/gen-flags.mjs
import { copyFileSync, mkdirSync } from "node:fs";

const COUNTRIES = ["sa", "bg", "cz", "dk", "de", "gr", "gb", "us", "es", "ee", "fi", "fr", "hu", "id", "it", "jp", "kr", "lt", "lv", "no", "nl", "pl", "pt", "br", "ro", "ru", "sk", "si", "se", "tr", "ua", "cn", "tw", "vn"];
const src = new URL("../app/node_modules/flag-icons/flags/4x3/", import.meta.url);
const dst = new URL("../app/src/lib/flags/", import.meta.url);
mkdirSync(dst, { recursive: true });
for (const c of COUNTRIES) copyFileSync(new URL(`${c}.svg`, src), new URL(`${c}.svg`, dst));
console.log(`${COUNTRIES.length} flags`);
