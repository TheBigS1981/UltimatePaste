export type ThemeMode = "system" | "light" | "dark";

export function applyTheme(mode: ThemeMode | undefined) {
  const root = document.documentElement;
  if (mode === "light" || mode === "dark") root.dataset.theme = mode;
  else delete root.dataset.theme;
}

/** Design that belongs to each window style when the design is set to "auto". */
export const DEFAULT_DESIGN: Record<string, string> = { fluent: "windows", board: "coral", drawer: "sage", compact: "violet" };

export function resolveDesign(layout: string | undefined, design: string | undefined): string {
  return !design || design === "auto" ? (DEFAULT_DESIGN[layout ?? "fluent"] ?? "windows") : design;
}

export function applyLook(layout: string | undefined, design: string | undefined) {
  const root = document.documentElement;
  root.dataset.layout = layout || "fluent";
  root.dataset.design = resolveDesign(layout, design);
}
