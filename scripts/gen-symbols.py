#!/usr/bin/env python3
"""Builds app/src/lib/picker/symbols.json: a curated set of special characters in categories.
English search names come from the Unicode database, German keywords from the table below."""
import json, pathlib, unicodedata

def rng(a, b):
    return [chr(c) for c in range(a, b + 1)]

CATS = [
    ("typography", "Typografie", "Typography",
     list("–—‑…·•‣◦‰′″‹›«»„“”‚‘’") + [" ", " ", " ", "​"] + list("¶§†‡※⁂¦¡¿©®™℠°")),
    ("arrows", "Pfeile", "Arrows", rng(0x2190, 0x21FF) + list("➔➜➝➞➟➠➡⟵⟶⟷⟸⟹⟺")),
    ("math", "Mathematik", "Math",
     list("±×÷−∓≈≠≡≢≤≥≪≫∞√∛∜∑∏∫∬∮∂∆∇∈∉∋∅∩∪⊂⊃⊆⊇⊕⊗⊥∥∠∡∴∵∝∀∃∄¬∧∨⊻⇒⇔ℕℤℚℝℂπℯ‰‱µ")
     + list("⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿ₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎")
     + list("½⅓⅔¼¾⅕⅖⅗⅘⅙⅚⅛⅜⅝⅞⅐⅑⅒")),
    ("currency", "Währungen", "Currency", list("€$£¥¢₿₽₹₩₺₴₪₫₦₱₲₵₸₡₭₮₼₾฿¤")),
    ("office", "Büro & Recht", "Office & legal",
     list("©®™℗§¶№℡℅℃℉Ω℮✓✔✗✘☐☑☒★☆✦✧✩✪✫✰◆◇■□▪▫●○◉◎▲△▼▽◀▶►◄♠♣♥♦♤♧♡♢☎☏✉✎✏✐✒✂☛☞⌘⌥⇧⌫⌦⏎⎋⏻")),
    ("greek", "Griechisch", "Greek", rng(0x03B1, 0x03C9) + [c for c in rng(0x0391, 0x03A9) if c != "΢"]),
    ("latin", "Buchstaben", "Letters", list("ÄÖÜäöüßẞÀÁÂÃÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕØŒÙÚÛÝÞàáâãåæçèéêëìíîïðñòóôõøœùúûýþÿŁłŠšŽžČčĆćĐđĞğİıŞşŘřŇňĚěŮů")),
]

DE = {
    "–": "halbgeviertstrich gedankenstrich bis-strich", "—": "geviertstrich", "‑": "geschützter bindestrich",
    "…": "auslassungspunkte drei punkte ellipse", "·": "mittelpunkt", "•": "aufzählungspunkt bullet",
    "„": "anführungszeichen unten deutsch", "“": "anführungszeichen oben deutsch", "”": "anführungszeichen englisch",
    "‚": "einfaches anführungszeichen unten", "‘": "einfaches anführungszeichen", "’": "apostroph",
    "«": "guillemets französisch", "»": "guillemets", " ": "geschütztes leerzeichen nbsp",
    " ": "schmales geschütztes leerzeichen", " ": "schmales leerzeichen", "​": "nullbreite leerzeichen",
    "¶": "absatzzeichen pilcrow", "§": "paragraf paragraph", "†": "kreuz gestorben dolch", "©": "copyright",
    "®": "eingetragene marke registered", "™": "marke trademark", "°": "grad", "‰": "promille", "′": "minute fuß",
    "″": "sekunde zoll", "¿": "umgekehrtes fragezeichen", "¡": "umgekehrtes ausrufezeichen",
    "±": "plusminus", "×": "mal multiplikation", "÷": "geteilt division", "−": "minus", "≈": "ungefähr circa",
    "≠": "ungleich", "≡": "identisch", "≤": "kleiner gleich", "≥": "größer gleich", "∞": "unendlich",
    "√": "wurzel", "∑": "summe", "∏": "produkt", "∫": "integral", "∆": "delta differenz", "∈": "element von",
    "∅": "leere menge", "∩": "schnittmenge", "∪": "vereinigung", "⊂": "teilmenge", "∠": "winkel", "∴": "daher",
    "∀": "für alle", "∃": "es existiert", "¬": "nicht", "∧": "und", "∨": "oder", "⇒": "daraus folgt implikation",
    "⇔": "genau dann äquivalent", "π": "pi", "µ": "mikro my", "½": "einhalb hälfte", "¼": "viertel", "¾": "dreiviertel",
    "⅓": "drittel", "⅔": "zweidrittel", "²": "hoch zwei quadrat", "³": "hoch drei kubik", "¹": "hoch eins",
    "€": "euro", "$": "dollar", "£": "pfund", "¥": "yen yuan", "¢": "cent", "₿": "bitcoin", "₽": "rubel",
    "₹": "rupie", "₩": "won", "₺": "lira", "₴": "hrywnja", "฿": "baht",
    "№": "nummer", "℃": "grad celsius", "℉": "grad fahrenheit", "Ω": "ohm omega",
    "✓": "haken häkchen check erledigt", "✔": "haken fett check", "✗": "kreuz falsch", "✘": "kreuz fett",
    "☐": "kästchen checkbox leer", "☑": "kästchen abgehakt", "☒": "kästchen angekreuzt", "★": "stern voll",
    "☆": "stern leer", "■": "quadrat schwarz", "□": "quadrat weiß", "●": "kreis punkt", "○": "kreis leer",
    "▲": "dreieck oben", "▼": "dreieck unten", "▶": "dreieck rechts play", "◀": "dreieck links",
    "♥": "herz", "♠": "pik", "♣": "kreuz kartenspiel", "♦": "karo", "☎": "telefon", "✉": "brief umschlag mail",
    "✎": "stift bleistift", "✂": "schere", "☛": "zeigefinger", "⌘": "befehlstaste command mac", "⌥": "wahltaste option",
    "⇧": "umschalt shift", "⌫": "rücktaste backspace", "⌦": "entfernen delete", "⏎": "eingabe return enter", "⎋": "escape",
    "⏻": "einschalten power",
    "→": "pfeil rechts", "←": "pfeil links", "↑": "pfeil hoch oben", "↓": "pfeil runter unten", "↔": "pfeil links rechts beidseitig",
    "↕": "pfeil hoch runter", "⇒": "doppelpfeil rechts folgt", "⇐": "doppelpfeil links", "⇔": "doppelpfeil beidseitig",
    "↗": "pfeil rechts oben", "↘": "pfeil rechts unten", "↙": "pfeil links unten", "↖": "pfeil links oben",
    "↻": "pfeil kreis im uhrzeigersinn neu laden", "↺": "pfeil kreis gegen uhrzeigersinn", "↵": "eingabe pfeil",
    "ß": "eszett scharfes s", "ẞ": "großes eszett", "Ä": "a umlaut", "Ö": "o umlaut", "Ü": "u umlaut",
    "ä": "a umlaut", "ö": "o umlaut", "ü": "u umlaut",
}
GREEK_DE = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda my ny xi omikron pi rho sigma sigma tau ypsilon phi chi psi omega".split()

def en_name(c):
    try:
        return unicodedata.name(c).lower()
    except ValueError:
        return ""

out = []
seen = set()
for key, de, en, chars in CATS:
    items = []
    for c in chars:
        if c in seen:
            continue
        seen.add(c)
        name = en_name(c)
        if not name:
            continue
        kw = DE.get(c, "")
        if key == "greek":
            low = c.lower()
            idx = ord(low) - 0x03B1
            if 0 <= idx < len(GREEK_DE):
                kw = GREEK_DE[idx]
        items.append([c, name, kw])
    out.append({"key": key, "de": de, "en": en, "items": items})

path = pathlib.Path(__file__).resolve().parent.parent / "app/src/lib/picker/symbols.json"
path.write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")
print(sum(len(c["items"]) for c in out), "symbols in", len(out), "categories")
