// Shortcuts that well-known programs use inside their own windows. Windows can't report
// these, so a picked combination that matches gets a gentle warning.
import { tr } from "./i18n.svelte";

const norm = (combo: string) =>
  combo
    .split("+")
    .map((p) => p.trim().toLowerCase().replace("control", "ctrl").replace(/^(win|meta|super)$/, "super"))
    .filter(Boolean)
    .sort()
    .join("+");

export function appShortcutHint(combo: string): string | null {
  switch (norm(combo)) {
    case "ctrl+period":
      return tr("Strg + . nutzen auch VS Code und Visual Studio (Schnellkorrektur) – dort greift dann UltimatePaste.", "Ctrl + . is also used by VS Code and Visual Studio (quick fix) – UltimatePaste takes it over there.");
    case "ctrl+shift+v":
      return tr("Strg + Umschalt + V fügt in Chrome, Edge, Teams und Word als reinen Text ein – dort greift dann UltimatePaste.", "Ctrl + Shift + V pastes plain text in Chrome, Edge, Teams and Word – UltimatePaste takes it over there.");
    case "alt+ctrl+v":
      return tr("Strg + Alt + V öffnet in Word und Excel „Inhalte einfügen“ – dort greift dann UltimatePaste.", "Ctrl + Alt + V opens “Paste special” in Word and Excel – UltimatePaste takes it over there.");
    case "ctrl+shift+t":
      return tr("Strg + Umschalt + T öffnet im Browser den zuletzt geschlossenen Tab wieder.", "Ctrl + Shift + T reopens the last closed tab in browsers.");
    case "ctrl+space":
      return tr("Strg + Leertaste nutzen Eingabemethoden und Code-Editoren (Vorschläge).", "Ctrl + Space is used by input methods and code editors (suggestions).");
    case "alt+space":
      return tr("Alt + Leertaste öffnet das Fenstermenü bzw. PowerToys Run.", "Alt + Space opens the window menu or PowerToys Run.");
    case "ctrl+shift+c":
      return tr("Strg + Umschalt + C öffnet im Browser die Entwicklertools.", "Ctrl + Shift + C opens the developer tools in browsers.");
    default:
      return null;
  }
}
