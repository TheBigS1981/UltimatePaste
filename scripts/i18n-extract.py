#!/usr/bin/env python3
"""Collects every UI text (German + English) from the frontend and backend and reports which
entries are missing in the other language catalogs (app/src/lib/locales/<lang>.json).

  python3 scripts/i18n-extract.py            # writes app/src/lib/locales/_source.json, prints counts
  python3 scripts/i18n-extract.py --missing it   # prints the missing entries for one language as JSON
"""
import json, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "app"
LOC = ROOT / "src/lib/locales"
LANGS = ["it", "es", "fr", "nl", "pl", "pt", "vi"]
S = r'"((?:[^"\\]|\\.)*)"'

def unescape(s: str) -> str:
    return json.loads(f'"{s}"')

pairs: dict[str, str] = {}

def add(de: str, en: str):
    de, en = unescape(de), unescape(en)
    if de and de != en and de not in pairs:
        pairs[de] = en
    elif de and de == en:
        pass

# tr("de", "en") in Svelte/TS; tr(app, "de", "en"), trf(…), tr_lang(lang, …), t("de", "en", …) in Rust
js_call = re.compile(r'\btr\(\s*' + S + r'\s*,\s*' + S)
rs_call = re.compile(r'\b(?:tr|trf|tr_lang|t)\(\s*(?:[&\w.]+\s*,\s*)?' + S + r'\s*,\s*' + S)
data_pair = re.compile(r'\bde:\s*' + S + r'\s*,\s*en:\s*' + S)
for f in list((ROOT / "src").rglob("*.svelte")) + list((ROOT / "src").rglob("*.ts")):
    if "locales" in f.parts:
        continue
    txt = f.read_text(encoding="utf-8")
    for m in js_call.finditer(txt):
        add(m.group(1), m.group(2))
    for m in data_pair.finditer(txt):
        if not re.fullmatch(r"[a-z]{2}-[A-Z]{2}", unescape(m.group(1))):  # locale tables
            add(m.group(1), m.group(2))
for f in (ROOT / "src-tauri/src").rglob("*.rs"):
    txt = f.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    for m in rs_call.finditer(txt):
        add(m.group(1), m.group(2))

# Keyed texts in i18n.svelte.ts ("@key")
i18n = (ROOT / "src/lib/i18n.svelte.ts").read_text(encoding="utf-8")
def block(name):
    m = re.search(r'const ' + name + r'(?::[^=]*)?\s*=\s*\{(.*?)\n\};', i18n, re.S)
    return dict((k, unescape(v)) for k, v in re.findall(r'^\s*(\w+):\s*' + S + r',?\s*$', m.group(1), re.M))
de_keys, en_keys = block("de"), block("en")
for k, v in de_keys.items():
    pairs["@" + k] = en_keys.get(k, v) + "\u0000" + v  # en + NUL + de, split below

# Symbol categories (symbols.json)
for c in json.loads((ROOT / "src/lib/picker/symbols.json").read_text(encoding="utf-8")):
    if c["de"] != c["en"]:
        pairs.setdefault(c["de"], c["en"])

source = []
for key, v in pairs.items():
    if key.startswith("@"):
        en, de = v.split("\u0000")
        source.append({"key": key, "de": de, "en": en})
    else:
        source.append({"key": key, "de": key, "en": v})

if len(sys.argv) == 2 and sys.argv[1] == "--prune":
    for l in LANGS:
        p = LOC / f"{l}.json"
        have = json.loads(p.read_text(encoding="utf-8"))
        keep = {k: v for k, v in have.items() if k in pairs}
        p.write_text(json.dumps(keep, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print("pruned")
    sys.exit()

if len(sys.argv) == 3 and sys.argv[1] == "--missing":
    have = json.loads((LOC / f"{sys.argv[2]}.json").read_text(encoding="utf-8"))
    print(json.dumps([e for e in source if e["key"] not in have], ensure_ascii=False, indent=1))
    sys.exit()

(LOC / "_source.json").write_text(json.dumps(source, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
print(f"{len(source)} texts")
for l in LANGS:
    have = json.loads((LOC / f"{l}.json").read_text(encoding="utf-8"))
    missing = [e for e in source if e["key"] not in have]
    stale = [k for k in have if k not in pairs]
    print(f"{l}: {len(have)} translated, {len(missing)} missing, {len(stale)} unused")
