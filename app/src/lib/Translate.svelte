<script lang="ts">
  import LangSelect from "./LangSelect.svelte";
  import { tabByAlt } from "./view";
  // Translate & correct window: original and result side by side. "Translate" changes the
  // language; "Correct" fixes errors or improves the style and shows the changes as a word diff
  // in which every change can be rejected with a click.
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, errText, SERVICE_KEYS, type Language, type Settings, type TranslateOutcome, type TranslateSession } from "./api";
  import { i18n, tr, locale, languageName } from "./i18n.svelte";
  import { applyLook, applyTheme } from "./theme";
  import { applySegs, changeCount, diffWords, type Seg } from "./diff";


  let settings = $state<Settings | null>(null);
  let langs = $state<Language[]>([]);
  let source = $state("auto");
  let target = $state<string | null>(null);
  /** Target follows the rule (pair / fixed) until the user picks one explicitly. */
  let targetAuto = $state(true);
  let engine = $state<"deepl" | "ai">("deepl");
  let formality = $state<"auto" | "less" | "more">("auto");
  let text = $state("");
  let out = $state("");
  let fromSelection = $state(false);
  let outcome = $state<TranslateOutcome | null>(null);
  let busy = $state(false);
  let error = $state("");
  let dirty = $state(false);
  let hasDeepl = $state(false);
  let applyBtn = $state<HTMLButtonElement>();
  let reqNo = 0;

  let tab = $state<"translate" | "correct">("translate");
  let cmode = $state<"correct" | "rephrase">("correct");
  let style = $state("");
  let cengine = $state<"deepl" | "ai">("deepl");
  /** Language of the text to correct ("auto" = detect) */
  let clang = $state("auto");
  let segs = $state<Seg[]>([]);
  let showDel = $state(true);
  let editing = $state(false);
  /** Show the word diff (off = plain, editable result). Remembered on this PC. */
  let showDiff = $state(readShowDiff());
  function readShowDiff(): boolean {
    try {
      return localStorage.getItem("up.showDiff") !== "0";
    } catch {
      return true;
    }
  }
  function setShowDiff(on: boolean) {
    // Edits made in the plain view become the new diff.
    if (on && !editing) segs = diffWords(text.replace(/[\r\n]+$/, ""), out);
    showDiff = on;
    try {
      localStorage.setItem("up.showDiff", on ? "1" : "0");
    } catch {
      /* not essential */
    }
  }
  let liveTimer: ReturnType<typeof setTimeout> | undefined;
  const changes = $derived(changeCount(segs));
  const accepted = $derived(segs.filter((x) => x.kind === "chg" && x.on).length);
  const curEngine = $derived(tab === "translate" ? engine : cengine);

  const styleOptions = $derived([
    ["", tr("Ohne Stilvorgabe", "No style")],
    ["simple", tr("Stil: einfach", "Style: simple")],
    ["business", tr("Stil: geschäftlich", "Style: business")],
    ["academic", tr("Stil: akademisch", "Style: academic")],
    ["casual", tr("Stil: locker", "Style: casual")],
    ["tone:friendly", tr("Ton: freundlich", "Tone: friendly")],
    ["tone:confident", tr("Ton: selbstbewusst", "Tone: confident")],
    ["tone:diplomatic", tr("Ton: diplomatisch", "Tone: diplomatic")],
    ["tone:enthusiastic", tr("Ton: begeistert", "Tone: enthusiastic")],
  ]);

  const bases = $derived.by(() => {
    const seen = new Map<string, string>();
    for (const l of langs) {
      const b = l.code.split("-")[0];
      if (!seen.has(b)) seen.set(b, languageName(b, l.de, l.en).replace(/ \(.*\)$/, ""));
    }
    return [...seen.entries()];
  });
  const langName = (code: string | null | undefined) => {
    if (!code) return "?";
    const l = langs.find((x) => x.code === code) ?? langs.find((x) => x.code.split("-")[0] === code.split("-")[0]);
    return l ? languageName(l.code.split("-")[0], l.de, l.en).replace(/ \(.*\)$/, "") : code;
  };

  const nf = (n: number) => n.toLocaleString(locale());
  const usd = (n: number) => `${n.toLocaleString(locale(), { minimumFractionDigits: n < 0.01 ? 4 : 2, maximumFractionDigits: n < 0.01 ? 4 : 2 })} $`;
  const words = (s: string) => (s.trim() ? s.trim().split(/\s+/).length : 0);
  const chars = (s: string) => [...s].length;

  function errorText(e: string): string {
    if (e === "no-key") return curEngine === "deepl" ? tr("Kein DeepL-Schlüssel hinterlegt.", "No DeepL key set.") : tr("Kein API-Schlüssel für den KI-Anbieter hinterlegt.", "No API key set for the AI provider.");
    if (e === "quota-exceeded") return tr("Das DeepL-Monatskontingent ist aufgebraucht.", "The DeepL monthly quota is used up.");
    if (e === "bad-key") return tr("Der API-Schlüssel wird nicht akzeptiert.", "The API key was rejected.");
    if (e === "rate-limited") return tr("Zu viele Anfragen – bitte kurz warten.", "Too many requests – please wait a moment.");
    if (e === "empty") return tab === "correct" ? tr("Kein Text zum Korrigieren. Markiere Text oder kopiere ihn vorher.", "No text to correct. Select or copy some text first.") : tr("Kein Text zum Übersetzen. Markiere Text oder kopiere ihn vorher.", "No text to translate. Select or copy some text first.");
    return e;
  }

  async function run() {
    const my = ++reqNo;
    if (!text.trim()) {
      error = errorText("empty");
      return;
    }
    clearTimeout(liveTimer);
    busy = true;
    error = "";
    try {
      const original = text;
      const o =
        tab === "translate"
          ? await api.translateRun({ text, source: source === "auto" ? null : source, target: targetAuto ? null : target, engine, formality: formality })
          : await api.improveRun({ text, mode: cmode, style: cmode === "rephrase" ? style : "", engine: cengine, lang: clang === "auto" ? null : clang });
      if (my !== reqNo) return;
      outcome = o;
      if (tab === "translate") {
        out = o.text;
        target = o.target;
        segs = [];
      } else {
        segs = diffWords(original.replace(/[\r\n]+$/, ""), o.text);
        out = applySegs(segs);
        editing = false;
      }
      dirty = false;
      saveSession();
      await tick();
      applyBtn?.focus();
    } catch (e) {
      if (my === reqNo) error = errorText(errText(e));
    } finally {
      if (my === reqNo) busy = false;
    }
  }

  async function load() {
    settings = await api.settings();
    i18n.lang = settings.language;
    applyTheme(settings.theme);
    applyLook(settings.layout, settings.design);
    if (!langs.length) langs = await api.languages();
    hasDeepl = await api.aiHasKey(SERVICE_KEYS.deepl);
    const p = await api.translatePending();
    if (!p) return;
    history = await api.thistList().catch(() => []);
    if (p.restore && history[0]) {
      openSession(history[0]);
      fromSelection = false;
      return;
    }
    sessionId = Date.now();
    text = p.text;
    fromSelection = p.fromSelection;
    out = "";
    outcome = null;
    segs = [];
    editing = false;
    tab = p.mode;
    cmode = "correct";
    clang = "auto";
    style = settings.translate.improveStyle;
    cengine = settings.translate.correctEngine === "deepl" && !hasDeepl ? "ai" : settings.translate.correctEngine;
    source = "auto";
    target = null;
    targetAuto = true;
    engine = settings.translate.engine === "deepl" && !hasDeepl ? "ai" : settings.translate.engine;
    formality = settings.translate.formality;
    if (text.trim()) await run();
    else {
      error = "";
      await tick();
      document.querySelector<HTMLTextAreaElement>('textarea[data-role="source"]')?.focus();
    }
  }

  function swap() {
    if (!outcome?.source && source === "auto") return;
    const from = source === "auto" ? (outcome?.source ?? "") : source;
    const to = target ?? "";
    const s = settings?.translate;
    const variant = [s?.primary, s?.secondary, s?.fixedTarget].find((c) => c && c.split("-")[0] === from.split("-")[0]) ?? langs.find((l) => l.code.split("-")[0] === from.split("-")[0])?.code ?? from;
    source = to.split("-")[0];
    target = variant;
    targetAuto = false;
    if (out.trim()) text = out;
    run();
  }

  // ---------------------------------------------------------------- history (Q20–Q23)

  let sessionId = $state(Date.now());
  let history = $state<TranslateSession[]>([]);
  let showHistory = $state(false);

  /** Stores the current state as one history entry per session (updated on every run). */
  function saveSession() {
    if (!text.trim()) return;
    const s: TranslateSession = {
      id: sessionId,
      updatedAt: 0,
      mode: tab,
      text,
      out,
      source: tab === "translate" ? (source === "auto" ? (outcome?.source ?? null) : source) : clang === "auto" ? (outcome?.source ?? null) : clang,
      target: tab === "translate" ? (target ?? null) : null,
      engine: curEngine,
      kind: tab === "correct" ? cmode : null,
      style: tab === "correct" && cmode === "rephrase" ? style : null,
    };
    api.thistSave(s).then(async () => (history = await api.thistList().catch(() => history)));
  }

  /** Shows a stored session again – without a new (paid) run. */
  function openSession(s: TranslateSession) {
    showHistory = false;
    sessionId = s.id;
    tab = s.mode;
    text = s.text;
    out = s.out;
    outcome = null;
    error = "";
    editing = false;
    dirty = false;
    if (s.mode === "translate") {
      source = "auto";
      if (s.target) {
        target = s.target;
        targetAuto = false;
      }
      if (s.engine === "deepl" || s.engine === "ai") engine = s.engine;
      segs = [];
    } else {
      cmode = s.kind === "rephrase" ? "rephrase" : "correct";
      if (s.style) style = s.style;
      if (s.engine === "deepl" || s.engine === "ai") cengine = s.engine;
      clang = "auto";
      segs = diffWords(s.text.replace(/[\r\n]+$/, ""), s.out);
    }
  }

  async function clearHistory() {
    await api.thistClear();
    history = [];
    showHistory = false;
  }

  const timeLabel = (ms: number) => {
    const d = new Date(ms);
    const today = new Date().toDateString() === d.toDateString();
    return today ? d.toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" }) : d.toLocaleDateString(locale(), { day: "numeric", month: "short" }) + " " + d.toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });
  };
  const kindLabel = (s: TranslateSession) => (s.mode === "translate" ? tr("Übersetzt", "Translated") : s.kind === "rephrase" ? tr("Verbessert", "Improved") : tr("Korrigiert", "Corrected"));
  const langsLabel = (s: TranslateSession) => (s.mode === "translate" ? `${(s.source ?? "?").split("-")[0]} → ${s.target ?? "?"}` : (s.source ?? "").split("-")[0]);

  function switchTab(t: "translate" | "correct") {
    if (t === tab) return;
    sessionId = Date.now();
    tab = t;
    out = "";
    outcome = null;
    segs = [];
    editing = false;
    error = "";
    if (text.trim()) run();
  }

  function onSourceInput() {
    dirty = true;
    clearTimeout(liveTimer);
    if (settings?.translate.live && text.trim()) liveTimer = setTimeout(run, 1000);
  }

  function toggle(i: number) {
    const x = segs[i];
    if (x.kind !== "chg") return;
    x.on = !x.on;
    out = applySegs(segs);
  }

  function setAll(on: boolean) {
    for (const x of segs) if (x.kind === "chg") x.on = on;
    out = applySegs(segs);
  }

  function toggleEdit() {
    if (editing) segs = diffWords(text.replace(/[\r\n]+$/, ""), out);
    editing = !editing;
  }

  async function close() {
    saveSession();
    await getCurrentWindow().hide();
    await api.hide(true);
  }

  async function apply(copyOnly = false) {
    if (!out.trim() || busy) return;
    saveSession();
    try {
      await api.translateApply(
        out,
        copyOnly,
        tab === "correct" ? { kind: "correct" } : { kind: "translate", pair: `${(outcome?.source ?? "?").split("-")[0]} → ${outcome?.target ?? "?"}`, chars: [...text].length },
      );
    } catch (e) {
      error = errText(e);
    }
  }

  function onKey(e: KeyboardEvent) {
    const inText = (e.target as HTMLElement)?.tagName === "TEXTAREA";
    if (e.key === "Escape") {
      e.preventDefault();
      if (showHistory) showHistory = false;
      else close();
    }
    else if (e.key === "Tab" && e.ctrlKey) (e.preventDefault(), switchTab(tab === "translate" ? "correct" : "translate"));
    else if (tabByAlt(e) === 0 || tabByAlt(e) === 1) (e.preventDefault(), switchTab(tabByAlt(e) === 0 ? "translate" : "correct"));
    else if (e.key === "Enter" && e.ctrlKey && inText && e.target && (e.target as HTMLElement).dataset.role === "source") (e.preventDefault(), run());
    else if (e.key === "Enter" && (e.ctrlKey || !inText) && !e.repeat) (e.preventDefault(), apply(e.altKey));
  }

  onMount(() => {
    load();
    const un = [
      listen("translate-request", load),
      listen("settings-changed", async () => {
        settings = await api.settings();
        i18n.lang = settings.language;
        applyTheme(settings.theme);
        applyLook(settings.layout, settings.design);
        hasDeepl = await api.aiHasKey(SERVICE_KEYS.deepl);
      }),
    ];
    return () => un.forEach((p) => p.then((f) => f()));
  });
</script>

<svelte:window onkeydown={onKey} onpointerdown={(e) => showHistory && !(e.target as HTMLElement).closest(".hist") && (showHistory = false)} />

<div class="tw">
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === "translate"} class:on={tab === "translate"} onclick={() => switchTab("translate")}>{tr("Übersetzen", "Translate")}<small>Alt 1</small></button>
    <button role="tab" aria-selected={tab === "correct"} class:on={tab === "correct"} onclick={() => switchTab("correct")}>{tr("Korrigieren", "Correct")}<small>Alt 2</small></button>
    <span class="tsp"></span>
    <div class="hist">
      <button class="histbtn" aria-expanded={showHistory} onclick={() => (showHistory = !showHistory)}>🕘 {tr("Verlauf", "History")}{history.length ? ` (${history.length})` : ""}</button>
      {#if showHistory}
        <div class="histpop" role="menu">
          {#each history as h (h.id)}
            <button role="menuitem" class:cur={h.id === sessionId} onclick={() => openSession(h)}>
              <span class="hmeta">{timeLabel(h.updatedAt)} · {kindLabel(h)}{langsLabel(h) ? ` · ${langsLabel(h)}` : ""}</span>
              <span class="htext">{h.text.slice(0, 120)}</span>
            </button>
          {:else}
            <p class="hempty">{tr("Noch kein Verlauf.", "No history yet.")}</p>
          {/each}
          {#if history.length}<button class="hclear" onclick={clearHistory}>{tr("Verlauf löschen", "Clear history")}</button>{/if}
        </div>
      {/if}
    </div>
  </div>
  {#if tab === "correct"}
    <header>
      <div class="sel">
        <span>{tr("Sprache", "Language")}</span>
        <LangSelect
          label={tr("Sprache", "Language")}
          value={clang}
          options={[{ code: "auto", label: tr("Automatisch", "Detect") + (clang === "auto" && outcome?.source ? ` (${langName(outcome.source)})` : ""), flag: clang === "auto" ? (outcome?.source ?? "") : "" }, ...bases.map(([code, name]) => ({ code, label: name }))]}
          onchange={(v) => ((clang = v), run())}
        />
      </div>
      <div class="seg" role="group" aria-label={tr("Art", "Kind")}>
        <button class:on={cmode === "correct"} onclick={() => ((cmode = "correct"), run())}>{tr("Nur Fehler", "Errors only")}</button>
        <button class:on={cmode === "rephrase"} onclick={() => ((cmode = "rephrase"), run())}>{tr("Verbessern", "Improve")}</button>
      </div>
      {#if cmode === "rephrase"}
        <select class="style" bind:value={style} onchange={run} aria-label={tr("Stil", "Style")}>
          {#each styleOptions as [v, l] (v)}<option value={v}>{l}</option>{/each}
        </select>
      {/if}
      <span class="sp"></span>
      <div class="seg" role="group" aria-label={tr("Dienst", "Service")}>
        <button class:on={cengine === "deepl"} onclick={() => ((cengine = "deepl"), run())}>DeepL</button>
        <button class:on={cengine === "ai"} onclick={() => ((cengine = "ai"), run())}>{tr("KI", "AI")}</button>
      </div>
    </header>
  {:else}
  <header>
    <div class="sel">
      <span>{tr("Von", "From")}</span>
      <LangSelect
        label={tr("Von", "From")}
        value={source}
        options={[{ code: "auto", label: tr("Automatisch", "Detect") + (source === "auto" && outcome?.source ? ` (${langName(outcome.source)})` : ""), flag: source === "auto" ? (outcome?.source ?? "") : "" }, ...bases.map(([code, name]) => ({ code, label: name }))]}
        onchange={(v) => ((source = v), run())}
      />
    </div>
    <button class="icon" title={tr("Richtung tauschen", "Swap direction")} onclick={swap}>⇄</button>
    <div class="sel">
      <span>{tr("Nach", "To")}</span>
      <LangSelect
        label={tr("Nach", "To")}
        value={targetAuto ? "" : (target ?? "")}
        options={[{ code: "", label: tr("Automatisch", "Automatic") + (targetAuto && target ? ` (${langName(target)})` : ""), flag: targetAuto ? (target ?? "") : "" }, ...langs.map((l) => ({ code: l.code, label: languageName(l.code, l.de, l.en) }))]}
        onchange={(v) => {
          targetAuto = !v;
          if (v) target = v;
          run();
        }}
      />
    </div>
    <span class="sp"></span>
    <div class="seg" role="group" aria-label={tr("Dienst", "Service")}>
      <button class:on={engine === "deepl"} onclick={() => ((engine = "deepl"), run())}>DeepL</button>
      <button class:on={engine === "ai"} onclick={() => ((engine = "ai"), run())}>{tr("KI", "AI")}</button>
    </div>
    <div class="seg" role="group" aria-label={tr("Anrede", "Address form")}>
      <button class:on={formality === "auto"} onclick={() => ((formality = "auto"), run())}>{tr("Auto", "Auto")}</button>
      <button class:on={formality === "less"} onclick={() => ((formality = "less"), run())}>{tr("Du", "Informal")}</button>
      <button class:on={formality === "more"} onclick={() => ((formality = "more"), run())}>{tr("Sie", "Formal")}</button>
    </div>
  </header>
  {/if}

  <main>
    <div class="pane">
      <textarea data-role="source" bind:value={text} oninput={onSourceInput} spellcheck="false" placeholder={tab === "correct" ? tr("Text zum Korrigieren", "Text to correct") : tr("Text zum Übersetzen", "Text to translate")}></textarea>
      <div class="meta">
        <span>{nf(words(text))} {tr("Wörter", "words")} · {nf(chars(text))} {tr("Zeichen", "characters")}</span>
        {#if dirty}<button class="small" onclick={run}>{tab === "correct" ? tr("Neu korrigieren", "Correct again") : tr("Neu übersetzen", "Translate again")} <kbd>{tr("Strg", "Ctrl")} ↵</kbd></button>{/if}
      </div>
    </div>
    <div class="pane">
      {#if tab === "translate" || editing || !showDiff}
        <textarea bind:value={out} spellcheck="false" class:busy placeholder={busy ? tr("Übersetze …", "Translating …") : ""}></textarea>
      {:else}
        <div class="diff" class:busy class:hide-del={!showDel} role="region" aria-label={tr("Änderungen", "Changes")}>
          {#each segs as x, i (i)}
            {#if x.kind === "eq"}{x.text}{:else}<button
                class="chg"
                class:off={!x.on}
                title={x.on ? tr("Klicken: Änderung ablehnen", "Click: reject change") : tr("Klicken: Änderung übernehmen", "Click: accept change")}
                onclick={() => toggle(i)}
                >{#if x.on}{#if x.del}<del>{x.del}</del>{/if}{#if x.add}<ins>{x.add}</ins>{/if}{:else}<span class="rej">{x.del || "·"}</span>{/if}</button
              >{/if}
          {/each}
          {#if !segs.length && !busy}<span class="ph">{outcome ? "" : tr("Hier erscheint der korrigierte Text.", "The corrected text appears here.")}</span>{/if}
        </div>
      {/if}
      <div class="meta">
        {#if tab === "correct" && outcome}
          <span>
            {#if changes === 0}{tr("Keine Änderungen", "No changes")}{:else}{accepted === changes ? "" : `${accepted} / `}{changes} {changes === 1 ? tr("Änderung", "change") : tr("Änderungen", "changes")}{/if}
          </span>
          <span class="tools">
            {#if showDiff && !editing && changes > 0}
              <label class="tick"><input type="checkbox" bind:checked={showDel} /> {tr("Gestrichenes", "Removed")}</label>
              {#if accepted < changes}<button class="small" onclick={() => setAll(true)}>{tr("Alle übernehmen", "Accept all")}</button>{:else}<button class="small" onclick={() => setAll(false)}>{tr("Alle ablehnen", "Reject all")}</button>{/if}
            {/if}
            {#if showDiff}<button class="small" onclick={toggleEdit}>{editing ? tr("Fertig", "Done") : tr("Bearbeiten", "Edit")}</button>{/if}
          </span>
        {:else}
          <span>{nf(words(out))} {tr("Wörter", "words")} · {nf(chars(out))} {tr("Zeichen", "characters")}</span>
        {/if}
        {#if busy}<span class="spin">{tab === "correct" ? tr("Prüfe …", "Checking …") : tr("Übersetze …", "Translating …")}</span>{/if}
      </div>
    </div>
  </main>

  {#if error}
    <div class="err" role="alert">
      <span>{error}</span>
      {#if error === errorText("quota-exceeded") || (curEngine === "deepl" && !hasDeepl)}
        {#if tab === "translate"}<button onclick={() => ((engine = "ai"), run())}>{tr("Mit KI übersetzen", "Translate with AI")}</button>
        {:else}<button onclick={() => ((cengine = "ai"), run())}>{tr("Mit KI korrigieren", "Correct with AI")}</button>{/if}
      {/if}
      <button onclick={() => api.openSettings()}>{tr("Einstellungen", "Settings")}</button>
    </div>
  {/if}

  <footer>
    <div class="status">
      {#if outcome}
        {#if outcome.engine === "deepl"}
          <span>{outcome.via === "write" ? "DeepL Write" : outcome.via === "roundtrip" ? tr("DeepL (Doppelübersetzung)", "DeepL (double translation)") : "DeepL"}{#if outcome.deeplBilled != null}{" · "}{nf(outcome.deeplBilled)} {tr("Zeichen berechnet", "characters billed")}{/if}</span>
          {#if outcome.deeplUsed != null && outcome.deeplLimit}<span>{tr("Monat", "Month")}: {nf(outcome.deeplUsed)} / {nf(outcome.deeplLimit)}</span>{/if}
        {:else}
          <span>{outcome.model ?? tr("KI", "AI")} · {nf(outcome.tokensIn)} → {nf(outcome.tokensOut)} Tokens{#if outcome.costUsd != null} · {usd(outcome.costUsd)}{/if}</span>
          <span>{tr("Monat", "Month")}: {usd(outcome.month.aiCostUsd)} · {nf(outcome.month.aiTokensIn + outcome.month.aiTokensOut)} Tokens</span>
        {/if}
      {/if}
    </div>
    {#if tab === "correct" && !editing}
      <label class="tick foot"><input type="checkbox" checked={showDiff} onchange={(e) => setShowDiff((e.target as HTMLInputElement).checked)} /> {tr("Änderungen anzeigen", "Show changes")}</label>
    {/if}
    <button onclick={() => apply(true)} disabled={!out.trim() || busy}>{tr("Kopieren", "Copy")} <kbd>Alt ↵</kbd></button>
    <button class="primary" bind:this={applyBtn} onclick={() => apply(false)} disabled={!out.trim() || busy}>
      {fromSelection ? tr("Ersetzen", "Replace") : tr("Einfügen", "Paste")} <kbd>↵</kbd>
    </button>
  </footer>
</div>

<style>
  .tw {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-solid);
    color: var(--text);
  }
  .tabs {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 8px 14px 0;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    background: transparent;
    padding: 6px 12px;
    color: var(--muted);
    font-weight: 600;
  }
  .tabs button {
    display: flex;
    flex-direction: column;
    align-items: center;
    line-height: 1.2;
  }
  .tabs small {
    font: 500 10px var(--mono);
    color: var(--faint);
  }
  .tabs button.on {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .style {
    font-size: 13px;
    min-width: 180px;
  }
  .diff {
    flex: 1;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font: 14px/1.7 var(--font);
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    user-select: text;
  }
  .diff.busy {
    opacity: 0.6;
  }
  .diff .ph {
    color: var(--faint);
  }
  .chg {
    all: unset;
    cursor: pointer;
    border-radius: 3px;
    white-space: pre-wrap;
  }
  .chg:hover {
    outline: 1px solid var(--accent);
  }
  .chg:focus-visible {
    outline: 2px solid var(--accent);
  }
  /* Removed text: clearly marked red; inserted text: normal color with a green underline. */
  del {
    color: var(--diff-del);
    background: color-mix(in srgb, var(--diff-del) 16%, transparent);
    text-decoration: line-through 1.5px color-mix(in srgb, var(--diff-del) 75%, transparent);
    padding: 1px 3px;
    border-radius: 4px;
  }
  del + ins {
    margin-left: 4px;
  }
  .hide-del del {
    display: none;
  }
  ins {
    color: var(--text);
    text-decoration: underline 2.5px var(--diff-add);
    text-underline-offset: 4px;
    background: color-mix(in srgb, var(--diff-add) 10%, transparent);
    border-radius: 3px;
  }
  .rej {
    text-decoration: underline dotted var(--faint);
    text-underline-offset: 3px;
  }
  .tools {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .tick.foot {
    font-size: 12.5px;
    color: var(--muted);
    margin-right: 4px;
  }
  .meta > span {
    white-space: nowrap;
  }
  .tick {
    white-space: nowrap;
    display: flex;
    gap: 4px;
    align-items: center;
  }
  header {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    flex-wrap: wrap;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }
  .sel {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .icon {
    padding: 5px 10px;
  }
  .sp {
    flex: 1;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2px;
    gap: 2px;
    background: var(--panel);
  }
  .seg button {
    border: 0;
    background: transparent;
    padding: 3px 10px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--on-accent);
  }
  main {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 10px;
    padding: 10px 14px;
  }
  .pane {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
  }
  textarea {
    flex: 1;
    resize: none;
    font: 14px/1.5 var(--font);
    padding: 10px 12px;
    user-select: text;
  }
  textarea.busy {
    opacity: 0.6;
  }
  .meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    font-size: 11.5px;
    color: var(--faint);
    min-height: 24px;
  }
  .small {
    font-size: 11.5px;
    padding: 1px 8px;
  }
  .spin {
    color: var(--accent);
  }
  .err {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 0 14px 8px;
    padding: 8px 10px;
    border: 1px solid var(--danger);
    border-radius: 8px;
    color: var(--danger);
    font-size: 13px;
  }
  .err span {
    flex: 1;
  }
  footer {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 14px;
    border-top: 1px solid var(--border);
  }
  .status {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 12px;
    color: var(--muted);
  }
  kbd {
    font: 600 10.5px var(--mono);
    opacity: 0.8;
  }
  .tsp {
    flex: 1;
  }
  .hist {
    position: relative;
    align-self: center;
  }
  .histbtn {
    font-size: 12.5px;
    padding: 3px 10px;
  }
  .histpop {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 40;
    width: min(440px, 80vw);
    max-height: 360px;
    overflow-y: auto;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--bg-solid);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 12px 30px -10px rgba(0, 0, 0, 0.45);
  }
  .histpop > button {
    all: unset;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 6px 8px;
    border-radius: 6px;
    cursor: pointer;
  }
  .histpop > button:hover,
  .histpop > button:focus-visible {
    background: var(--accent-soft, var(--panel));
  }
  .histpop > button.cur .hmeta {
    color: var(--accent);
  }
  .hmeta {
    font-size: 11.5px;
    color: var(--muted);
  }
  .htext {
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .histpop > .hclear {
    margin-top: 4px;
    border-top: 1px solid var(--border);
    border-radius: 0;
    font-size: 12px;
    color: var(--danger);
    text-align: center;
  }
  .hempty {
    margin: 8px;
    font-size: 12.5px;
    color: var(--muted);
  }
</style>
