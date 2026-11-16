<script lang="ts">
  // Small popup at the text caret for a typed snippet's fields (choices and inputs).
  // It never has the focus: keys arrive from the keyboard hook as "picker-key" events.
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type Settings, type SnippetField } from "./api";
  import { i18n, tr } from "./i18n.svelte";
  import { applyLook, applyTheme } from "./theme";

  type RedirKey = { key: string; code: string; text: string; ctrl: boolean; alt: boolean; shift: boolean };

  let fields = $state<SnippetField[]>([]);
  let step = $state(0);
  let values = $state<Record<string, string>>({});
  let query = $state("");
  let sel = $state(0);
  let popEl = $state<HTMLDivElement>();

  const field = $derived(fields[step]);
  const isChoice = $derived(!!field && field.options.length > 0);
  const filtered = $derived.by(() => {
    if (!field) return [] as { value: string; n: number }[];
    const q = query.trim().toLowerCase();
    return field.options.map((value, i) => ({ value, n: i + 1 })).filter((o) => !q || o.value.toLowerCase().includes(q));
  });
  const showFilter = $derived(isChoice && (field!.options.length > 5 || query.length > 0));
  const last = $derived(step >= fields.length - 1);

  function enterStep(i: number) {
    step = i;
    query = "";
    const f = fields[i];
    if (!f) return;
    if (f.options.length) {
      const cur = values[f.name] ?? f.default;
      sel = Math.max(0, f.options.indexOf(cur));
    } else if (values[f.name] === undefined) {
      values[f.name] = f.default;
    }
    fit();
  }

  async function fit() {
    await tick();
    if (!popEl) return;
    const r = popEl.getBoundingClientRect();
    // 16 px transparent margin on every side for the shadow
    api.fillFit(Math.ceil(r.width) + 32, Math.ceil(r.height) + 32).catch(() => {});
  }

  function choose(value: string) {
    values[field!.name] = value;
    next();
  }

  function next() {
    if (last) {
      api.fillSubmit($state.snapshot(values)).catch(() => {});
      fields = [];
    } else enterStep(step + 1);
  }

  function back() {
    if (step > 0) enterStep(step - 1);
  }

  function onKey(k: RedirKey) {
    if (!field) return;
    if (k.key === "Escape") return void api.fillCancel().catch(() => {});
    if (k.key === "Tab") return k.shift ? back() : isChoice ? (filtered[sel] ? choose(filtered[sel].value) : undefined) : next();
    if (isChoice) {
      if (k.key === "ArrowDown") sel = filtered.length ? (sel + 1) % filtered.length : 0;
      else if (k.key === "ArrowUp") sel = filtered.length ? (sel - 1 + filtered.length) % filtered.length : 0;
      else if (k.key === "Home") sel = 0;
      else if (k.key === "End") sel = Math.max(0, filtered.length - 1);
      else if (k.key === "Enter") {
        if (filtered[sel]) choose(filtered[sel].value);
      } else if (k.key === "Backspace") {
        query = [...query].slice(0, -1).join("");
        sel = 0;
        fit();
      } else if (!query && /^[1-9]$/.test(k.text) && +k.text <= field.options.length && !k.ctrl && !k.alt) {
        choose(field.options[+k.text - 1]);
      } else if (k.text && !k.ctrl && !k.alt) {
        query += k.text;
        sel = 0;
        fit();
      }
      return;
    }
    const v = values[field.name] ?? "";
    if (k.key === "Enter") next();
    else if (k.key === "Backspace") values[field.name] = k.ctrl ? v.replace(/\S+\s*$/u, "") : [...v].slice(0, -1).join("");
    else if (k.text && !k.ctrl && !k.alt) values[field.name] = v + k.text;
    else return;
    fit();
  }

  async function load() {
    const job = await api.fillJob().catch(() => null);
    fields = job?.fields ?? [];
    values = {};
    enterStep(0);
  }

  onMount(() => {
    const un = [
      listen<RedirKey>("picker-key", (e) => onKey(e.payload)),
      listen("fill-shown", load),
      listen("settings-changed", async () => applySettings(await api.settings())),
    ];
    api.settings().then(applySettings);
    load();
    return () => un.forEach((p) => p.then((f) => f()));
  });

  function applySettings(s: Settings) {
    i18n.lang = s.language;
    applyTheme(s.theme);
    applyLook(s.layout, s.design);
  }
</script>

{#if field}
  <div class="pop" bind:this={popEl}>
    <div class="title">
      <b>{field.name}</b>
      {#if fields.length > 1}<span class="step">{step + 1} / {fields.length}</span>{/if}
    </div>
    {#each fields.slice(0, step) as f (f.name)}
      <div class="done"><span>{f.name}</span><span>{values[f.name]}</span></div>
    {/each}
    {#if isChoice}
      {#if showFilter}
        <div class="filter">🔍 <span class="q">{query}</span><span class="caret"></span></div>
      {/if}
      {#each filtered as o, i (o.n)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="opt" class:on={i === sel} role="option" tabindex="-1" aria-selected={i === sel} onmouseenter={() => (sel = i)} onclick={() => choose(o.value)}>
          <span class="n">{o.n <= 9 ? o.n : ""}</span><span class="v">{o.value}</span>{#if o.value === field.default && field.options.length > 1}<span class="defl">{tr("Standard", "Default")}</span>{/if}
        </div>
      {:else}
        <div class="none">{tr("Nichts gefunden.", "Nothing found.")}</div>
      {/each}
    {:else}
      <div class="field"><div class="input">{values[field.name] ?? ""}<span class="caret"></span></div></div>
    {/if}
    <div class="hints">
      {#if isChoice}<span><kbd>↑↓</kbd> {tr("wählen", "choose")}</span>{/if}
      <span><kbd>Enter</kbd> {last ? tr("einfügen", "paste") : tr("weiter", "next")}</span>
      {#if step > 0}<span><kbd>⇧ Tab</kbd> {tr("zurück", "back")}</span>{/if}
      <span><kbd>Esc</kbd> {tr("abbrechen", "cancel")}</span>
    </div>
  </div>
{/if}

<style>
  .pop {
    position: absolute;
    left: 16px;
    top: 16px;
    width: max-content;
    min-width: 200px;
    max-width: 680px;
    background: var(--bg-solid);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 10px 28px -8px rgba(0, 0, 0, 0.35), 0 2px 8px -2px rgba(0, 0, 0, 0.2);
    padding: 6px;
    font-size: 13.5px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 6px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .title b {
    color: var(--text);
    font-weight: 600;
  }
  .step {
    margin-left: auto;
    padding-left: 16px;
    font-variant-numeric: tabular-nums;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 6px;
    white-space: nowrap;
    cursor: default;
  }
  .opt .n {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    background: var(--kbd);
    font-size: 11px;
    display: grid;
    place-items: center;
    color: var(--muted);
    flex: none;
  }
  .opt.on {
    background: var(--accent);
    color: var(--on-accent);
  }
  .opt.on .n {
    background: rgba(255, 255, 255, 0.22);
    color: var(--on-accent);
  }
  .defl {
    margin-left: auto;
    padding-left: 14px;
    font-size: 11px;
    color: var(--muted);
  }
  .opt.on .defl {
    color: var(--on-accent);
    opacity: 0.8;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 2px 6px;
    padding: 5px 8px;
    border-radius: 6px;
    background: var(--kbd);
    color: var(--muted);
    font-size: 12.5px;
  }
  .q {
    color: var(--text);
  }
  .caret {
    display: inline-block;
    width: 1.5px;
    height: 1.05em;
    background: var(--text);
    vertical-align: text-bottom;
    animation: blink 1s steps(1) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .field {
    padding: 2px 6px 6px;
  }
  .input {
    min-width: 220px;
    border: 1.5px solid var(--accent);
    border-radius: 6px;
    padding: 6px 8px;
    white-space: pre;
  }
  .done {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 4px 8px;
    font-size: 12.5px;
    opacity: 0.6;
  }
  .none {
    padding: 6px 8px;
    color: var(--muted);
  }
  .hints {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    padding: 6px 8px 2px;
    margin-top: 4px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--muted);
  }
  kbd {
    font: 10.5px/1 var(--font);
    background: var(--kbd);
    border-radius: 4px;
    padding: 2px 5px;
    color: var(--text);
  }
</style>
