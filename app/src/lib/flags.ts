// Small flag images for language pickers (flag-icons, MIT). Windows can't draw flag emoji.
const FILES = import.meta.glob<string>("./flags/*.svg", { eager: true, query: "?url", import: "default" });

const COUNTRY: Record<string, string> = {
  AR: "sa", BG: "bg", CS: "cz", DA: "dk", DE: "de", EL: "gr", EN: "gb", "EN-GB": "gb", "EN-US": "us",
  ES: "es", ET: "ee", FI: "fi", FR: "fr", HU: "hu", ID: "id", IT: "it", JA: "jp", KO: "kr", LT: "lt",
  LV: "lv", NB: "no", NL: "nl", PL: "pl", PT: "pt", "PT-BR": "br", "PT-PT": "pt", RO: "ro", RU: "ru",
  SK: "sk", SL: "si", SV: "se", TR: "tr", UK: "ua", ZH: "cn", "ZH-HANS": "cn", "ZH-HANT": "tw", VI: "vn",
};

/** Flag image URL for a language code ("DE", "EN-US", UI codes like "pt" or "vi"). */
export function flagUrl(code: string): string | undefined {
  const up = code.toUpperCase();
  const c = up === "PT" && code === "pt" ? "br" : COUNTRY[up] ?? COUNTRY[up.split("-")[0]];
  return c ? FILES[`./flags/${c}.svg`] : undefined;
}
