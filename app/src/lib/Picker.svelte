<script lang="ts">
  import { tabByAlt, withinGrace } from "./view";
  // Emoji / GIF / symbol picker (own popup window at the text caret).
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, errText, SERVICE_KEYS, type GifItem, type PasteResult, type PickerState, type Settings } from "./api";
  import { i18n, tr, deOrEn } from "./i18n.svelte";
  import { applyLook, applyTheme } from "./theme";
  import { visibleEmoji, emojiName, groupLabel, loadEmojiNames, GROUPS, SYMBOLS, SYMBOL_CATS, emojiOf, isSymbol, search, searchSymbols, withTone, codepoint, type Emoji } from "./picker/data";

  type TabId = "emoji" | "gif" | "symbols";
  interface Cell {
    /** What gets pasted */
    value: string;
    label: string;
    emoji?: Emoji;
  }
  interface Section {
    key: string;
    title: string;
    cells: Cell[];
  }


  let settings = $state<Settings | null>(null);
  let pstate = $state<PickerState>({ emojiRecent: [], symbolRecent: [], gifRecent: [], favorites: [] });
  let tab = $state<TabId>(import.meta.env.DEV && new URLSearchParams(location.search).get("ptab") === "gif" ? "gif" : "emoji");
  let query = $state("");
  let searchEl = $state<HTMLInputElement>();
  let gridEl = $state<HTMLDivElement>();
  let sel = $state(0);
  let collected = $state<string[]>([]);
  let skinFor = $state<{ emoji: Emoji; index: number } | null>(null);
  let notice = $state("");

  const tone = $derived(settings?.picker.skinTone ?? 0);
  const recentMax = $derived(settings?.picker.recentCount ?? 24);
  const gifOn = $derived(settings?.picker.gifEnabled ?? true);
  const TABS = $derived<TabId[]>(gifOn ? ["emoji", "gif", "symbols"] : ["emoji", "symbols"]);
  const COLS = $derived(tab === "symbols" ? 10 : tab === "gif" ? 3 : 8);

  const groupName = (id: number) => {
    const g = GROUPS.find((x) => x.id === id);
    return g ? groupLabel(g, i18n.lang) : "";
  };
  const GROUP_ICON: Record<string, string> = {
    "smileys-emotion": "😀",
    "people-body": "👋",
    "animals-nature": "🐻",
    "food-drink": "🍔",
    "travel-places": "✈️",
    activities: "⚽",
    objects: "💡",
    symbols: "🔣",
    flags: "🏁",
  };

  function emojiCell(e: Emoji, value?: string): Cell {
    return { value: value ?? withTone(e, tone), label: emojiName(e, i18n.lang), emoji: e };
  }
  function anyCell(v: string): Cell {
    const e = emojiOf(v);
    if (e) return { value: v, label: emojiName(e, i18n.lang), emoji: e };
    const s = SYMBOLS.find((x) => x.char === v);
    return { value: v, label: s ? deOrEn(s.de || s.en, s.en) : codepoint(v) };
  }

  /** Most used first; ties keep the given order (favorites: as pinned, recent: newest first). */
  function byUse(list: string[]): string[] {
    const u = pstate.uses ?? {};
    return list.map((v, i) => [v, i] as const).sort((a, b) => (u[b[0]] ?? 0) - (u[a[0]] ?? 0) || a[1] - b[1]).map((x) => x[0]);
  }

  let namesReady = $state(0);
  const sections = $derived.by<Section[]>(() => {
    void namesReady;
    if (tab === "emoji") {
      if (query.trim()) return [{ key: "results", title: tr("Treffer", "Results"), cells: search(visibleEmoji(), query).map((e) => emojiCell(e)) }];
      const out: Section[] = [];
      const fav = pstate.favorites.filter((f) => !isSymbol(f));
      if (fav.length) out.push({ key: "fav", title: tr("Favoriten", "Favorites"), cells: byUse(fav).map(anyCell) });
      // Favorites are always on top: not repeated under "recently used".
      const recentE = pstate.emojiRecent.filter((v) => !pstate.favorites.includes(v)).slice(0, recentMax);
      if (recentE.length) out.push({ key: "recent", title: tr("Zuletzt verwendet", "Recently used"), cells: byUse(recentE).map(anyCell) });
      for (const g of GROUPS) out.push({ key: g.key, title: groupName(g.id), cells: visibleEmoji().filter((e) => e.group === g.id).map((e) => emojiCell(e)) });
      return out;
    }
    if (tab === "symbols") {
      if (query.trim()) return [{ key: "results", title: tr("Treffer", "Results"), cells: searchSymbols(query).map((s) => ({ value: s.char, label: deOrEn(s.de || s.en, s.en) })) }];
      const out: Section[] = [];
      const fav = pstate.favorites.filter((f) => isSymbol(f));
      if (fav.length) out.push({ key: "fav", title: tr("Favoriten", "Favorites"), cells: byUse(fav).map(anyCell) });
      const recentS = pstate.symbolRecent.filter((v) => !pstate.favorites.includes(v)).slice(0, recentMax);
      if (recentS.length) out.push({ key: "recent", title: tr("Zuletzt verwendet", "Recently used"), cells: byUse(recentS).map(anyCell) });
      for (const c of SYMBOL_CATS) out.push({ key: c.key, title: tr(c.de, c.en), cells: SYMBOLS.filter((s) => s.cat === c.key).map((s) => ({ value: s.char, label: deOrEn(s.de || s.en, s.en) })) });
      return out;
    }
    return [];
  });

  /** Grid rows across sections, for arrow-key navigation: [sectionIndex, firstCellIndexInFlat] */
  const flat = $derived(sections.flatMap((s) => s.cells));
  const rowStarts = $derived.by(() => {
    const starts: number[] = [];
    let i = 0;
    for (const s of sections) {
      for (let r = 0; r < s.cells.length; r += COLS) starts.push(i + r);
      i += s.cells.length;
    }
    return starts;
  });
  const current = $derived<Cell | null>(tab === "gif" ? null : (flat[sel] ?? null));

  // ---------------------------------------------------------------- GIFs
  let gifProvider = $state<"giphy" | "klipy">("giphy");
  let gifSticker = $state(false);
  let gifItems = $state<GifItem[]>([]);
  let gifMode = $state<"recent" | "search" | "trending">("recent");
  let gifLoading = $state(false);
  let gifError = $state("");
  let gifPage = $state(0);
  let gifMore = $state(true);
  let hasKey = $state<Record<string, boolean>>({ giphy: false, klipy: false });
  let gifTimer: ReturnType<typeof setTimeout> | undefined;
  let gifReq = 0;
  const gifCurrent = $derived(tab === "gif" ? (gifItems[sel] ?? null) : null);
  const bothKeys = $derived(hasKey.giphy && hasKey.klipy);

  async function checkKeys() {
    hasKey = { giphy: await api.aiHasKey(SERVICE_KEYS.giphy), klipy: await api.aiHasKey(SERVICE_KEYS.klipy) };
    const preferred = settings?.picker.gifProvider ?? "giphy";
    gifProvider = hasKey[preferred] || !hasKey[preferred === "giphy" ? "klipy" : "giphy"] ? preferred : preferred === "giphy" ? "klipy" : "giphy";
  }

  function gifErrorText(e: string): string {
    if (e === "no-key") return "";
    if (e === "rate-limited")
      return tr(
        "Stundenlimit erreicht (100 Abfragen pro Stunde beim kostenlosen Schlüssel). Spätestens in einer Stunde geht es weiter.",
        "Hourly limit reached (100 requests per hour on the free key). It works again within an hour.",
      );
    if (e === "bad-key") return tr("Der API-Schlüssel wird nicht akzeptiert. Bitte in den Einstellungen prüfen.", "The API key was rejected. Please check it in the settings.");
    return e;
  }

  async function loadGifs(reset = true) {
    if (!hasKey[gifProvider]) {
      gifItems = [];
      return;
    }
    const my = ++gifReq;
    const q = query.trim();
    gifLoading = true;
    gifError = "";
    if (reset) {
      gifPage = 0;
      gifMore = true;
    }
    try {
      let items: GifItem[];
      if (gifMode === "recent" && !q) {
        items = (await api.gifRecent()).filter((g) => g.sticker === gifSticker);
        gifMore = false;
      } else {
        items = await api.gifSearch(gifProvider, gifMode === "trending" && !q ? "" : q, gifSticker, gifPage);
        gifMore = items.length > 0;
      }
      if (my !== gifReq) return;
      gifItems = reset ? items : [...gifItems, ...items];
      if (reset) sel = 0;
    } catch (e) {
      if (my === gifReq) gifError = gifErrorText(errText(e));
    } finally {
      if (my === gifReq) gifLoading = false;
    }
  }

  function onGifScroll(e: Event) {
    const el = e.target as HTMLElement;
    if (tab !== "gif" || gifLoading || !gifMore || gifMode === "recent") return;
    if (el.scrollTop + el.clientHeight > el.scrollHeight - 120) {
      gifPage++;
      loadGifs(false);
    }
  }

  // ---------------------------------------------------------------- actions
  async function deliver(p: Promise<PasteResult>) {
    try {
      const r = await p;
      if (r.delivery === "copiedElevated") {
        notice = tr("Zielprogramm läuft als Administrator – mit Strg + V einfügen.", "Target runs as administrator – paste with Ctrl + V.");
        setTimeout(() => api.hide(true), 2200);
      }
    } catch (e) {
      notice = errText(e);
    }
  }

  function kindOf(v: string): "emoji" | "symbol" {
    return isSymbol(v) && !emojiOf(v) ? "symbol" : "emoji";
  }

  function paste(cell: Cell | null, copyOnly = false) {
    const items = collected.length ? [...collected] : cell ? [cell.value] : [];
    if (!items.length) return;
    collected = [];
    deliver(api.pickerPaste(kindOf(items[0]), items, copyOnly));
  }

  function collect(cell: Cell | null) {
    if (cell) collected = [...collected, cell.value];
  }

  async function pasteGif(g: GifItem | null, copyOnly = false) {
    if (!g) return;
    notice = tr("GIF wird geladen …", "Loading GIF …");
    await deliver(api.gifPaste(g, query.trim(), copyOnly));
    notice = "";
  }

  async function toggleFavorite(cell: Cell | null) {
    if (!cell) return;
    const on = !pstate.favorites.includes(cell.value);
    pstate = await api.pickerFavorite(cell.value, on);
  }

  async function setTone(t: number) {
    if (!settings) return;
    settings.picker.skinTone = t;
    await api.saveSettings($state.snapshot(settings) as Settings);
  }

  function setTab(t: TabId) {
    tab = t;
    sel = 0;
    skinFor = null;
    gridEl?.scrollTo({ top: 0 });
    if (t === "gif") {
      gifMode = query.trim() ? "search" : "recent";
      loadGifs();
    }
    searchEl?.focus();
  }

  function onSearch() {
    sel = 0;
    gridEl?.scrollTo({ top: 0 });
    if (tab === "gif") {
      clearTimeout(gifTimer);
      const q = query.trim();
      if (!q) {
        gifMode = "recent";
        loadGifs();
      } else if (q.length >= 2) {
        gifMode = "search";
        gifTimer = setTimeout(() => loadGifs(), 400);
      }
    }
  }

  function jumpTo(key: string) {
    query = "";
    // Scroll only the grid: scrollIntoView would also move the window's own layout and put
    // an emoji under the mouse pointer.
    const el = gridEl?.querySelector<HTMLElement>(`[data-section="${key}"]`);
    // Section headers are sticky: one above the current position "sticks" at the top and
    // would measure as already there. Measure the (non-sticky) cells below it instead.
    const cells = el?.nextElementSibling as HTMLElement | null;
    if (gridEl && el && cells) gridEl.scrollTop += cells.getBoundingClientRect().top - gridEl.getBoundingClientRect().top - el.offsetHeight - 4;
    let i = 0;
    for (const s of sections) {
      if (s.key === key) break;
      i += s.cells.length;
    }
    sel = i;
  }

  async function scrollSel() {
    await tick();
    gridEl?.querySelector(".cell.on, .gif.on")?.scrollIntoView({ block: "nearest" });
  }

  function move(dx: number, dy: number) {
    const n = tab === "gif" ? gifItems.length : flat.length;
    if (!n) return;
    if (tab === "gif" || dy === 0) {
      sel = Math.max(0, Math.min(n - 1, sel + dx + dy * COLS));
    } else {
      // Rows can be short at the end of a section: move by visual row, keep the column.
      let r = 0;
      while (r + 1 < rowStarts.length && rowStarts[r + 1] <= sel) r++;
      const col = sel - rowStarts[r];
      const nr = Math.max(0, Math.min(rowStarts.length - 1, r + dy));
      const rowEnd = (rowStarts[nr + 1] ?? n) - 1;
      sel = Math.min(rowStarts[nr] + col, rowEnd);
    }
    scrollSel();
  }

  function onKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    const k = e.key;
    if (skinFor) {
      if (k === "Escape") (e.preventDefault(), (skinFor = null));
      else if (k === "ArrowLeft") (e.preventDefault(), (skinFor.index = Math.max(0, skinFor.index - 1)));
      else if (k === "ArrowRight") (e.preventDefault(), (skinFor.index = Math.min(5, skinFor.index + 1)));
      else if (k === "Enter") {
        e.preventDefault();
        const v = withTone(skinFor.emoji, skinFor.index);
        const cell = emojiCell(skinFor.emoji, v);
        skinFor = null;
        if (e.shiftKey) collect(cell);
        else paste(cell);
      }
      return;
    }
    if (k === "Escape") {
      e.preventDefault();
      if (collected.length) collected = [];
      else if (query) {
        query = "";
        onSearch();
      } else api.hide(true);
    } else if (tabByAlt(e) !== null && TABS[tabByAlt(e)!]) {
      e.preventDefault();
      setTab(TABS[tabByAlt(e)!]);
    } else if (e.ctrlKey && k === "Tab") {
      e.preventDefault();
      const i = TABS.indexOf(tab);
      setTab(TABS[(i + (e.shiftKey ? TABS.length - 1 : 1)) % TABS.length]);
    } else if (k === "Tab" && !e.ctrlKey) {
      e.preventDefault();
      if (current?.emoji?.skins) skinFor = { emoji: current.emoji, index: tone };
    } else if (k === "ArrowRight" && !inputHasCaretRoom(k)) (e.preventDefault(), move(1, 0));
    else if (k === "ArrowLeft" && !inputHasCaretRoom(k)) (e.preventDefault(), move(-1, 0));
    else if (k === "ArrowDown") (e.preventDefault(), move(0, 1));
    else if (k === "ArrowUp") (e.preventDefault(), move(0, -1));
    else if (k === "Enter") {
      e.preventDefault();
      if (e.repeat) return;
      if (tab === "gif") pasteGif(gifCurrent, e.altKey);
      else if (e.shiftKey) collect(current);
      else paste(current, e.altKey);
    } else if (e.ctrlKey && k.toLowerCase() === "p" && tab !== "gif") (e.preventDefault(), toggleFavorite(current));
  }

  /** Left/right edit the search text while the caret can still move inside it. */
  /**
   * Key presses the keyboard hook hands over: the picker doesn't take the focus (so Windows'
   * own Win+. can't get through), the program you type in stays in front.
   */
  /** The search field really has the keyboard focus (only after clicking into it). */
  let realFocus = $state(false);

  type RedirKey = { key: string; code: string; text: string; ctrl: boolean; alt: boolean; shift: boolean };
  function onRedirectedKey(k: RedirKey) {
    const plainText = !!k.text && !k.ctrl && !k.alt && !["Enter", "Tab", "Escape", "Backspace"].includes(k.key);
    if (plainText && !skinFor) {
      query += k.text;
      onSearch();
      return;
    }
    if (k.key === "Backspace" && !skinFor) {
      query = k.ctrl ? query.replace(/\S+\s*$/u, "") : [...query].slice(0, -1).join("");
      onSearch();
      return;
    }
    onKey(new KeyboardEvent("keydown", { key: k.key, code: k.code, ctrlKey: k.ctrl, altKey: k.alt, shiftKey: k.shift }));
  }

  function inputHasCaretRoom(k: string): boolean {
    const el = searchEl;
    // Without real focus (picker not activated) arrows always move the selection.
    if (!el || !realFocus || document.activeElement !== el || !query) return false;
    return k === "ArrowLeft" ? (el.selectionStart ?? 0) > 0 : (el.selectionEnd ?? 0) < query.length;
  }

  /** Cell the mouse went down on: a click only pastes when it started on the same cell. */
  let downCell = -1;
  function onCellClick(e: MouseEvent, cell: Cell, i: number) {
    if (downCell !== i) return;
    downCell = -1;
    sel = i;
    if (e.shiftKey) collect(cell);
    else paste(cell);
  }

  function onCellMenu(e: MouseEvent, cell: Cell, i: number) {
    e.preventDefault();
    sel = i;
    if (cell.emoji?.skins) skinFor = { emoji: cell.emoji, index: tone };
  }

  async function reload() {
    settings = await api.settings();
    i18n.lang = settings.language;
    loadEmojiNames(settings.language).then(() => namesReady++);
    applyTheme(settings.theme);
    applyLook(settings.layout, settings.design);
    pstate = await api.pickerState();
    await checkKeys();
  }

  onMount(() => {
    const un: Promise<() => void>[] = [];
    reload().then(() => {
      searchEl?.focus();
      if (tab === "gif") loadGifs();
    });
    un.push(
      listen<RedirKey>("picker-key", (e) => onRedirectedKey(e.payload)),
    );
    un.push(
      listen("picker-shown", async () => {
        skinFor = null;
        notice = "";
        const keep = withinGrace();
        if (!keep) {
          query = "";
          collected = [];
          sel = 0;
          tab = "emoji";
        }
        await reload();
        if (tab === "gif" && !keep) {
          gifMode = "recent";
          loadGifs();
        }
        if (!keep) gridEl?.scrollTo({ top: 0 });
        searchEl?.focus();
      }),
    );
    un.push(
      listen("settings-changed", async () => {
        settings = await api.settings();
        i18n.lang = settings.language;
        loadEmojiNames(settings.language).then(() => namesReady++);
        applyTheme(settings.theme);
        applyLook(settings.layout, settings.design);
      }),
    );
    return () => un.forEach((p) => p.then((f) => f()));
  });

  const TONES = ["#ffcc4d", "#f7dece", "#f3d2a2", "#d5ab88", "#af7e57", "#7c533e"];
  const placeholder = $derived(
    tab === "gif"
      ? gifProvider === "klipy"
        ? tr("KLIPY durchsuchen", "Search KLIPY")
        : tr("GIPHY durchsuchen", "Search GIPHY")
      : tab === "symbols"
        ? tr("Sonderzeichen suchen (auch U+2192)", "Search symbols (also U+2192)")
        : tr("Emoji suchen", "Search emoji"),
  );
  const tabLabel = (t: TabId) => (t === "emoji" ? "Emoji" : t === "gif" ? "GIF" : tr("Sonderzeichen", "Symbols"));
</script>

<svelte:window onkeydown={onKey} onfocus={() => (realFocus = document.activeElement === searchEl)} onblur={() => (realFocus = false)} />

<div class="scene">
  <div class="pk">
    <div class="top" data-tauri-drag-region>
      <div class="search">
        <input type="text" bind:this={searchEl} bind:value={query} oninput={onSearch} {placeholder} spellcheck="false" autocomplete="off" onfocus={() => (realFocus = document.hasFocus())} onblur={() => (realFocus = false)} />
        <!-- The window never takes the focus, so Windows draws no caret: draw one after the text. -->
        {#if !realFocus}<div class="fakecaret" aria-hidden="true"><span class="ghost">{query}</span><span class="bar"></span></div>{/if}
      </div>
      {#if tab === "emoji"}
        <div class="tones" role="radiogroup" aria-label={tr("Hautfarbe", "Skin tone")}>
          {#each TONES as c, i (i)}
            <button class="tone" class:on={tone === i} style:background={c} role="radio" aria-checked={tone === i} tabindex="-1" title={i === 0 ? tr("Standard", "Default") : tr("Hautton {0}", "Skin tone {0}", i)} onclick={() => setTone(i)}></button>
          {/each}
        </div>
      {/if}
    </div>
    <nav>
      {#each TABS as t, i (t)}
        <button class="tabbtn" class:on={tab === t} tabindex="-1" onclick={() => setTab(t)}>{tabLabel(t)}<small>Alt {i + 1}</small></button>
      {/each}
      {#if tab === "gif"}
        <span class="sp"></span>
        {#if bothKeys}
          <div class="seg">
            <button class:on={gifProvider === "giphy"} tabindex="-1" onclick={() => ((gifProvider = "giphy"), loadGifs())}>GIPHY</button>
            <button class:on={gifProvider === "klipy"} tabindex="-1" onclick={() => ((gifProvider = "klipy"), loadGifs())}>KLIPY</button>
          </div>
        {/if}
      {/if}
    </nav>

    {#if tab !== "gif" && !query.trim()}
      <div class="cats">
        {#each sections as s (s.key)}
          {@const icon = s.key === "fav" ? "★" : s.key === "recent" ? "🕘" : tab === "emoji" ? (GROUP_ICON[s.key] ?? "•") : (s.cells[0]?.value ?? "•")}
          <button tabindex="-1" title={s.title} onclick={() => jumpTo(s.key)}>{icon.trim() || "␣"}</button>
        {/each}
      </div>
    {/if}

    <div class="grid" bind:this={gridEl} onscroll={onGifScroll}>
      {#if tab === "gif"}
        {#if !hasKey[gifProvider]}
          <div class="empty">
            <p>{tr("Für die GIF-Suche brauchst du einen kostenlosen API-Schlüssel von GIPHY oder KLIPY.", "GIF search needs a free API key from GIPHY or KLIPY.")}</p>
            <p class="small">developers.giphy.com · partner.klipy.com</p>
            <button onclick={() => api.openSettings()}>{tr("Zu den Einstellungen", "Open settings")}</button>
          </div>
        {:else}
          <div class="gifbar">
            <button class:on={gifMode === "recent" && !query.trim()} tabindex="-1" onclick={() => ((query = ""), (gifMode = "recent"), loadGifs())}>{tr("Zuletzt verwendet", "Recent")}</button>
            <button class:on={gifMode === "trending" && !query.trim()} tabindex="-1" onclick={() => ((query = ""), (gifMode = "trending"), loadGifs())}>{tr("Angesagt", "Trending")}</button>
            <span class="sp"></span>
            <div class="seg">
              <button class:on={!gifSticker} tabindex="-1" onclick={() => ((gifSticker = false), loadGifs())}>GIFs</button>
              <button class:on={gifSticker} tabindex="-1" onclick={() => ((gifSticker = true), loadGifs())}>Sticker</button>
            </div>
          </div>
          {#if gifError}
            <div class="empty"><p>{gifError}</p>{#if bothKeys}<button onclick={() => ((gifProvider = gifProvider === "giphy" ? "klipy" : "giphy"), loadGifs())}>{tr("Zu {0} wechseln", "Switch to {0}", gifProvider === "giphy" ? "KLIPY" : "GIPHY")}</button>{/if}</div>
          {:else if !gifItems.length && !gifLoading}
            <div class="empty"><p>{gifMode === "recent" && !query.trim() ? tr("Noch keine GIFs verwendet. Einfach etwas suchen oder „Angesagt“ öffnen.", "No GIFs used yet. Search for something or open “Trending”.") : tr("Nichts gefunden.", "Nothing found.")}</p></div>
          {/if}
          <div class="gifs">
            {#each gifItems as g, i (g.provider + g.id + i)}
              <button class="gif" class:on={i === sel} class:sticker={g.sticker} tabindex="-1" title={g.title} onclick={() => ((sel = i), pasteGif(g))}>
                <img src={g.preview} alt={g.title} loading="lazy" referrerpolicy="no-referrer" />
                {#if g.sticker && gifMode === "recent"}<span class="badge">S</span>{/if}
              </button>
            {/each}
          </div>
          {#if gifLoading}<p class="small center">{tr("Lädt …", "Loading …")}</p>{/if}
        {/if}
      {:else}
        {#each sections as s, si (s.key)}
          {@const offset = sections.slice(0, si).reduce((n, x) => n + x.cells.length, 0)}
          <div class="sec" data-section={s.key}>{s.title}</div>
          <div class="cells" class:sym={tab === "symbols"}>
            {#each s.cells as c, ci (s.key + ci)}
              {@const i = offset + ci}
              <button class="cell" class:on={i === sel} class:fav={pstate.favorites.includes(c.value)} tabindex="-1" title={c.label} onpointerdown={() => (downCell = i)} onclick={(e) => onCellClick(e, c, i)} oncontextmenu={(e) => onCellMenu(e, c, i)}>{c.value.trim() ? c.value : "␣"}</button>
            {/each}
          </div>
        {:else}
          <div class="empty"><p>{tr("Nichts gefunden.", "Nothing found.")}</p></div>
        {/each}
        {#if !sections.length || (sections.length === 1 && !sections[0].cells.length)}
          <div class="empty"><p>{tr("Nichts gefunden.", "Nothing found.")}</p></div>
        {/if}
      {/if}
    </div>

    {#if skinFor}
      <div class="skins" role="dialog" aria-label={tr("Hautfarbe wählen", "Choose skin tone")}>
        {#each [0, 1, 2, 3, 4, 5] as t (t)}
          {@const v = withTone(skinFor.emoji, t)}
          <button class:on={skinFor.index === t} tabindex="-1" onclick={(e) => { const c = emojiCell(skinFor!.emoji, v); skinFor = null; if (e.shiftKey) collect(c); else paste(c); }}>{v}</button>
        {/each}
      </div>
    {/if}

    <footer>
      {#if notice}
        <span class="notice">{notice}</span>
      {:else if collected.length}
        <span class="coll">{collected.join("")}</span>
        <span class="hint"><kbd>↵</kbd> {tr("alle einfügen", "paste all")} · <kbd>Esc</kbd> {tr("verwerfen", "discard")}</span>
      {:else if tab === "gif"}
        <span class="hint"><kbd>↵</kbd> {tr("einfügen", "paste")} · <kbd>Alt ↵</kbd> {tr("kopieren", "copy")}</span>
        <span class="attr">{gifProvider === "klipy" ? "Powered by KLIPY" : "Powered by GIPHY"}</span>
      {:else if current}
        <span class="big">{current.value.trim() ? current.value : "␣"}</span>
        <span class="name">{current.label}{#if tab === "symbols"} · <small>{codepoint(current.value)}</small>{/if}</span>
        <span class="hint"><kbd>⇧↵</kbd> {tr("sammeln", "collect")}{#if current.emoji?.skins} · <kbd>Tab</kbd> {tr("Hautton", "tone")}{/if} · <kbd>{tr("Strg", "Ctrl")} P</kbd> ★</span>
      {/if}
    </footer>
  </div>
</div>

<style>
  .scene {
    height: 100%;
    padding: 16px;
    color: var(--text);
    font: 13.5px/1.4 var(--font);
  }
  .pk {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-solid);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 16px 36px -12px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }
  .top {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 10px 6px;
  }
  .search {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
  }
  .fakecaret {
    position: absolute;
    inset: 0;
    /* same box as the input: 1px border + 6px/8px padding */
    padding: 7px 9px;
    font: inherit;
    font-size: inherit;
    white-space: pre;
    overflow: hidden;
    pointer-events: none;
    display: flex;
    align-items: center;
  }
  .fakecaret .ghost {
    visibility: hidden;
  }
  .fakecaret .bar {
    flex: none;
    width: 1.5px;
    height: 1.15em;
    background: var(--text);
    animation: blink 1.06s steps(1) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .top input {
    flex: 1;
    min-width: 0;
    /* The window is never activated, so show the field as active: typing goes here. */
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .tones {
    display: flex;
    gap: 3px;
  }
  .tone {
    width: 16px;
    height: 16px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
  }
  .tone.on {
    border-color: var(--accent);
  }
  nav {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 0 10px 6px;
  }
  nav > button {
    border: 0;
    background: transparent;
    padding: 3px 10px;
    border-radius: 6px;
    color: var(--muted);
  }
  nav > button.on {
    background: var(--sel);
    color: var(--accent);
    font-weight: 600;
  }
  .sp {
    flex: 1;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px;
    gap: 2px;
  }
  .seg button {
    border: 0;
    background: transparent;
    padding: 1px 7px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--on-accent);
  }
  .cats {
    display: flex;
    gap: 2px;
    padding: 0 8px 4px;
    border-bottom: 1px solid var(--border);
    overflow-x: auto;
  }
  .cats button {
    border: 0;
    background: transparent;
    padding: 2px 5px;
    font-size: 16px;
    line-height: 1.2;
    color: var(--muted);
  }
  .cats button:hover {
    background: var(--kbd);
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 4px 8px 8px;
  }
  .sec {
    position: sticky;
    top: -4px;
    z-index: 1;
    background: var(--bg-solid);
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
    padding: 6px 2px 3px;
  }
  .cells {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 2px;
  }
  .cells.sym {
    grid-template-columns: repeat(10, 1fr);
  }
  .cell {
    aspect-ratio: 1;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    font: 24px/1 "Segoe UI Emoji", "Noto Color Emoji", "Apple Color Emoji", "Segoe UI Symbol", var(--font);
    display: grid;
    place-items: center;
  }
  .cells.sym .cell {
    font: 19px/1 var(--font), "Segoe UI Symbol";
  }
  .cell:hover {
    background: var(--kbd);
  }
  .cell.on {
    background: var(--sel);
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .cell.fav {
    position: relative;
  }
  .gifbar {
    display: flex;
    gap: 6px;
    align-items: center;
    padding: 4px 0 8px;
  }
  .gifbar > button {
    font-size: 12px;
    padding: 2px 10px;
    border-radius: 999px;
  }
  .gifbar > button.on {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }
  .gifs {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
  }
  .gif {
    position: relative;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 8px;
    overflow: hidden;
    height: 96px;
    background: var(--kbd);
  }
  .gif.sticker {
    background: repeating-conic-gradient(var(--kbd) 0 25%, transparent 0 50%) 0 0 / 12px 12px;
  }
  .gif img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .gif.sticker img {
    object-fit: contain;
  }
  .gif.on {
    border-color: var(--accent);
  }
  .badge {
    position: absolute;
    right: 4px;
    top: 4px;
    font-size: 10px;
    background: var(--accent);
    color: var(--on-accent);
    border-radius: 4px;
    padding: 0 4px;
  }
  .empty {
    padding: 28px 16px;
    text-align: center;
    color: var(--muted);
  }
  .small {
    font-size: 11.5px;
    color: var(--faint);
  }
  .center {
    text-align: center;
  }
  .skins {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    gap: 4px;
    padding: 8px;
    border-radius: 12px;
    background: var(--bg-solid);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    z-index: 3;
  }
  .skins button {
    font: 26px/1 "Segoe UI Emoji", "Noto Color Emoji", "Apple Color Emoji", var(--font);
    padding: 6px;
    border: 2px solid transparent;
    background: transparent;
    border-radius: 8px;
  }
  .skins button.on {
    border-color: var(--accent);
    background: var(--sel);
  }
  footer {
    display: flex;
    gap: 10px;
    align-items: center;
    min-height: 40px;
    padding: 6px 12px;
    border-top: 1px solid var(--border);
    font-size: 12px;
    color: var(--muted);
  }
  .big {
    font: 22px/1 "Segoe UI Emoji", "Noto Color Emoji", "Apple Color Emoji", var(--font);
    color: var(--text);
  }
  .name {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text);
  }
  .hint {
    white-space: nowrap;
  }
  .hint kbd {
    font: 600 10.5px var(--mono);
  }
  .coll {
    flex: 1;
    font: 18px/1 "Segoe UI Emoji", "Noto Color Emoji", "Apple Color Emoji", var(--font);
    color: var(--text);
    overflow: hidden;
  }
  .attr {
    margin-left: auto;
    font-weight: 600;
    font-size: 11px;
    letter-spacing: 0.02em;
    color: var(--text);
  }
  .notice {
    color: var(--text);
  }
  .tabbtn {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    line-height: 1.15;
  }
  .tabbtn small {
    font: 500 9.5px var(--mono);
    color: var(--faint);
  }
  .tabbtn.on small {
    color: inherit;
    opacity: 0.75;
  }
</style>
