<script lang="ts">
  // Hotkey picker: click modifier keycaps + choose the key, or record by pressing.
  // Value format matches the backend parser, e.g. "Ctrl+Shift+V" (empty string = disabled).
  import { tr } from "./i18n.svelte";

  // `local`: shortcut inside our own window (no Win key, Enter allowed).
  let { value, onchange, allowEmpty = false, local = false }: { value: string; onchange: (v: string) => void; allowEmpty?: boolean; local?: boolean } = $props();

  const ALL_MODS = ["Ctrl", "Alt", "Shift", "Super"] as const;
  type Mod = (typeof ALL_MODS)[number];
  const MODS = $derived(local ? ALL_MODS.filter((m) => m !== "Super") : ALL_MODS);
  const KEYS = $derived([
    ...(local ? ["Enter"] : []),
    ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ".split(""),
    ..."0123456789".split(""),
    ...Array.from({ length: 12 }, (_, i) => `F${i + 1}`),
    "Space",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Backquote",
    "Period",
    "Comma",
  ]);

  const label = (m: Mod) => (m === "Ctrl" ? tr("Strg", "Ctrl") : m === "Shift" ? tr("Umschalt", "Shift") : m === "Super" ? "Win" : "Alt");
  const keyLabel = (k: string) =>
    ({ Space: tr("Leertaste", "Space"), Insert: tr("Einfg", "Insert"), Home: tr("Pos1", "Home"), End: tr("Ende", "End"), PageUp: tr("Bild ↑", "Page Up"), PageDown: tr("Bild ↓", "Page Down"), Backquote: "^ / `", Period: ".", Comma: ",", Enter: tr("Eingabe ↵", "Enter ↵") })[k] ?? k;

  function parse(v: string): { mods: Set<Mod>; key: string } {
    const parts = v.split("+").map((p) => p.trim()).filter(Boolean);
    const mods = new Set<Mod>();
    let key = "";
    for (const p of parts) {
      const u = p.toLowerCase();
      if (u === "ctrl" || u === "control" || u === "commandorcontrol") mods.add("Ctrl");
      else if (u === "alt" || u === "option") mods.add("Alt");
      else if (u === "shift") mods.add("Shift");
      else if (u === "super" || u === "win" || u === "meta" || u === "cmd") mods.add("Super");
      else key = p.replace(/^Key/, "").replace(/^Digit/, "");
    }
    return { mods, key };
  }

  const parsed = $derived(parse(value));
  let recording = $state(false);

  function emit(mods: Set<Mod>, key: string) {
    if (!key) return onchange(allowEmpty && mods.size === 0 ? "" : value);
    onchange([...ALL_MODS.filter((m) => mods.has(m)), key].join("+"));
  }

  function toggle(m: Mod) {
    const mods = new Set(parsed.mods);
    mods.has(m) ? mods.delete(m) : mods.add(m);
    if (mods.size === 0) return; // a global hotkey needs at least one modifier
    emit(mods, parsed.key || (local ? "Enter" : "V"));
  }

  function onRecordKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = false;
      return;
    }
    if (["Control", "Shift", "Alt", "Meta", "OS"].includes(e.key)) return;
    const mods = new Set<Mod>();
    if (e.ctrlKey) mods.add("Ctrl");
    if (e.altKey) mods.add("Alt");
    if (e.shiftKey) mods.add("Shift");
    if (e.metaKey && !local) mods.add("Super");
    if (!mods.size) return;
    const key = e.code === "NumpadEnter" ? "Enter" : e.code.replace(/^Key/, "").replace(/^Digit/, "");
    recording = false;
    emit(mods, key);
  }
</script>

<svelte:window onkeydown={onRecordKey} />

<div class="picker">
  <div class="row">
    {#each MODS as m (m)}
      <button type="button" class="cap" class:on={parsed.mods.has(m)} aria-pressed={parsed.mods.has(m)} onclick={() => toggle(m)}>{label(m)}</button>
    {/each}
    <span class="plus">+</span>
    <select value={parsed.key} onchange={(e) => emit(parsed.mods.size ? parsed.mods : new Set<Mod>(["Ctrl"]), (e.target as HTMLSelectElement).value)}>
      {#if parsed.key && !KEYS.includes(parsed.key)}<option value={parsed.key}>{parsed.key}</option>{/if}
      {#each KEYS as k (k)}<option value={k}>{keyLabel(k)}</option>{/each}
    </select>
  </div>
  <div class="row">
    <button type="button" class="rec" class:active={recording} onclick={() => (recording = !recording)}>
      {recording ? tr("Tasten drücken … (Esc bricht ab)", "Press keys … (Esc cancels)") : tr("● Aufnehmen", "● Record")}
    </button>
    {#if allowEmpty && value}
      <button type="button" class="rec" onclick={() => onchange("")}>{tr("Deaktivieren", "Disable")}</button>
    {/if}
    {#if !value}<span class="off">{tr("deaktiviert", "disabled")}</span>{/if}
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .cap {
    border: 1px solid var(--border);
    border-bottom-width: 3px;
    border-radius: 8px;
    padding: 5px 12px;
    font-weight: 600;
    color: var(--muted);
    background: var(--panel);
  }
  .cap.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .plus,
  .off {
    color: var(--faint);
  }
  select {
    font-weight: 600;
    min-width: 110px;
  }
  .rec {
    border-style: dashed;
    color: var(--muted);
  }
  .rec.active {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
