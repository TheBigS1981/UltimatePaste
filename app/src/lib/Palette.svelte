<script lang="ts">
  import { quickActions, type Quick } from "./quick";
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, errText, type Delivery, type EntryDetail, type EntrySummary, type PasteMode, type Settings, type Snippet, type SnippetField, type Tab } from "./api";
  import { i18n, t, tr } from "./i18n.svelte";
  import { applyLook, applyTheme } from "./theme";
  import { matchesShortcut, tabByAlt, withinGrace, TABS, type Layout, type ViewActions, type ViewModel, type ViewTab } from "./view";
  import Fluent from "./layouts/Fluent.svelte";
  import Board from "./layouts/Board.svelte";
  import Drawer from "./layouts/Drawer.svelte";
  import Compact from "./layouts/Compact.svelte";

  type Action = { id: string; label: string; hint?: string; group?: string; run: () => void | Promise<void> };
  type Overlay =
    | { kind: "actions" }
    | { kind: "edit"; id: number; text: string; orig: string; ask?: boolean }
    | { kind: "rename"; id: number; title: string }
    | { kind: "snippet"; snippet: Snippet; orig: string; ask?: boolean }
    | { kind: "busy"; label: string }
    | { kind: "confirm"; label: string; run: () => void }
    | { kind: "fields"; id: number; name: string; copy: boolean; fields: SnippetField[]; values: Record<string, string> };


  let settings = $state<Settings | null>(null);
  let query = $state("");
  let tab = $state<ViewTab>(import.meta.env.DEV && new URLSearchParams(location.search).get("tab") === "snippets" ? "snippets" : "all");
  let items = $state<EntrySummary[]>([]);
  let snippets = $state<Snippet[]>([]);
  let sel = $state(0);
  let detail = $state<EntryDetail | null>(null);
  let overlay = $state<Overlay | null>(null);
  let toast = $state<string | null>(null);
  let actionQuery = $state("");
  let actionSel = $state(0);

  let searchEl: HTMLInputElement | undefined = $state();
  let thumbs = $state<Record<number, string>>({});
  let missing = $state<Record<number, boolean>>({});
  const actionsKey = $derived(settings?.actionsKey || "Ctrl+Enter");
  let reqNo = 0;
  let aiToken = 0;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  const detailCache = new Map<number, EntryDetail>();

  const count = $derived(tab === "snippets" ? snippets.length : items.length);
  const current = $derived(tab === "snippets" ? null : (items[sel] ?? null));
  const currentSnippet = $derived(tab === "snippets" ? (snippets[sel] ?? null) : null);
  const layout = $derived<Layout>((settings?.layout as Layout) ?? "fluent");
  /** Grid columns for arrow-key navigation (board layout is a 4-column grid). */
  const cols = $derived(layout === "board" ? 4 : 1);

  // Quick actions for the selected text entry (Q32); a link may have trackers to remove.
  let cleanedLink = $state<string | null>(null);
  $effect(() => {
    const txt = detail?.text?.trim() ?? "";
    cleanedLink = null;
    if (/^https?:\/\/\S+$/i.test(txt)) api.cleanLink(txt).then((c) => (cleanedLink = c));
  });
  const quick = $derived(currentSnippet || !detail?.text || detail.files.length || detail.summary.id !== current?.id ? [] : quickActions(detail.text, cleanedLink));

  function runQuick(q: Quick) {
    overlay = null;
    if (q.kind === "paste") deliver(api.pasteText(q.value, false));
    else
      api
        .openExternal(q.value)
        .then(() => api.hide(true))
        .catch((e) => showToast(errText(e)));
  }

  // ---------------------------------------------------------------- several entries (Q34)

  let marked = $state<number[]>([]);
  function toggleMark(i: number) {
    const it = items[i];
    if (!it) return;
    marked = marked.includes(it.id) ? marked.filter((x) => x !== it.id) : [...marked, it.id];
  }

  type Joiner = "newline" | "blank" | "comma" | "bullets" | "numbered";
  async function pasteMarked(how: Joiner = "newline", copy = false) {
    const ids = [...marked];
    const texts: string[] = [];
    for (const id of ids) {
      const d = detailCache.get(id) ?? (await api.detail(id).catch(() => null));
      const t = d?.text ?? (d?.files.length ? d.files.join("\r\n") : null);
      if (t != null) texts.push(t.replace(/\s+$/, ""));
    }
    if (!texts.length) return;
    const joined =
      how === "blank" ? texts.join("\r\n\r\n") : how === "comma" ? texts.join(", ") : how === "bullets" ? texts.map((t) => `• ${t}`).join("\r\n") : how === "numbered" ? texts.map((t, i) => `${i + 1}. ${t}`).join("\r\n") : texts.join("\r\n");
    marked = [];
    overlay = null;
    deliver(api.pasteText(joined, false, copy));
  }

  const markHint = $derived(marked.length ? tr("{0} markiert · ↵ alle einfügen · Esc hebt auf", "{0} marked · ↵ paste all · Esc clears", marked.length) : null);

  const vm: ViewModel = $derived({ tab, items, snippets, sel, detail, settings, thumbs, missing, toast: toast ?? markHint, query, quick, marked });
  const act: ViewActions = {
    select: (i, e) => {
      sel = i;
      if (e?.ctrlKey && tab !== "snippets") toggleMark(i);
    },
    activate: (i) => pasteIndex(i),
    setTab: (tb) => (setTab(tb), focusSearch()),
    run: (id) => {
      if (id === "paste") pasteCurrent({ mode: settings?.preferPlainText ? "plain" : "default" });
      else if (id === "plain") pasteCurrent({ mode: "plain" });
      else if (id === "copy") pasteCurrent({ copy: true });
      else if (id === "pin") togglePin();
      else if (id === "actions") openActions();
      else if (id === "ai") openActions(`${t("aiPrefix")}:`);
      else if (id === "settings") api.openSettings();
      else if (id === "edit") editCurrent();
      else if (id === "new") newSnippet();
      else if (id === "del") removeCurrent();
    },
    quick: (q) => runQuick(q),
  };

  // Thumbnails for image entries (lazy, cached for the session).
  $effect(() => {
    const missing = items.filter((it) => it.kind === "image" && !(it.id in thumbs)).slice(0, 60);
    for (const it of missing) {
      thumbs[it.id] = "";
      api
        .thumb(it.id)
        .then((d) => {
          if (d) thumbs[it.id] = d;
        })
        .catch(() => {});
    }
  });

  // File entries whose files were moved or deleted (re-checked each time the list loads).
  let missingReq = 0;
  function checkMissing() {
    const ids = items.filter((it) => it.kind === "files").map((it) => it.id);
    const my = ++missingReq;
    if (!ids.length) return;
    api
      .missingFiles(ids)
      .then((gone) => {
        if (my === missingReq) missing = Object.fromEntries(gone.map((id) => [id, true]));
      })
      .catch(() => {});
  }

  async function refresh() {
    const my = ++reqNo;
    try {
      if (tab === "snippets") {
        const r = await api.snippets(query);
        if (my === reqNo) snippets = r;
      } else {
        const r = await api.search(query, tab);
        if (my === reqNo) {
          items = r;
          checkMissing();
        }
      }
      if (my === reqNo) sel = Math.max(0, Math.min(sel, count - 1));
    } catch (e) {
      showToast(errText(e));
    }
  }

  // Detail for the selected entry (debounced, cached until the history changes).
  $effect(() => {
    const id = current?.id;
    if (id == null) {
      detail = null;
      return;
    }
    const cached = detailCache.get(id);
    if (cached) {
      detail = cached;
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const d = await api.detail(id);
        detailCache.set(id, d);
        if (current?.id === id) detail = d;
      } catch {
        /* entry vanished */
      }
    }, 40);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    void sel;
    void tab;
    tick().then(() => {
      if (!overlay) document.querySelector('.view [role="option"][aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
    });
  });

  function showToast(msg: string, ms = 2600) {
    toast = msg;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), ms);
  }

  async function deliver(p: Promise<{ delivery: Delivery; missing?: string[] }>) {
    try {
      const r = await p;
      if (r.delivery === "copiedElevated") {
        showToast(t("elevatedHint"), 2400);
        setTimeout(() => api.hide(true), 2200);
      } else if (r.missing?.length) {
        showToast(t("filesPartlyMissing", { names: r.missing.join(", ") }), 3000);
      }
    } catch (e) {
      const msg = errText(e);
      if (msg.startsWith("files-missing:")) {
        showToast(t("filesMissing", { names: msg.slice(14) }), 3500);
        checkMissing();
      } else showToast(msg);
    }
  }

  /** Snippets with input fields first ask for the values (Q33). */
  async function pasteSnippetById(id: number, name: string, copy: boolean) {
    const fields = await api.snippetFields(id).catch(() => [] as SnippetField[]);
    if (!fields.length) {
      deliver(api.pasteSnippet(id, copy));
      return;
    }
    overlay = { kind: "fields", id, name, copy, fields, values: Object.fromEntries(fields.map((f) => [f.name, f.default])) };
  }

  function submitFields() {
    if (overlay?.kind !== "fields") return;
    const o = overlay;
    overlay = null;
    deliver(api.pasteSnippet(o.id, o.copy, $state.snapshot(o.values)));
  }

  let pasting = false;
  function pasteCurrent(opts: { alt?: boolean; copy?: boolean; mode?: PasteMode } = {}) {
    if (!settings || pasting) return;
    pasting = true;
    setTimeout(() => (pasting = false), 400);
    if (currentSnippet) {
      pasteSnippetById(currentSnippet.id, currentSnippet.name, !!opts.copy || settings.primaryAction === "copy");
      return;
    }
    if (!current) return;
    let mode: PasteMode = opts.mode ?? (settings.preferPlainText !== !!opts.alt ? "plain" : "default");
    if (opts.copy || (!opts.mode && !opts.alt && settings.primaryAction === "copy")) mode = "copy";
    deliver(api.paste(current.id, mode));
  }

  function pasteIndex(i: number) {
    if (i < count) {
      sel = i;
      pasteCurrent();
    }
  }

  async function togglePin() {
    if (!current) return;
    await api.togglePin(current.id);
    await refresh();
  }

  async function removeCurrent() {
    if (currentSnippet) {
      await api.deleteSnippet(currentSnippet.id);
      await refresh();
      return;
    }
    if (!current) return;
    await api.remove(current.id);
    detailCache.delete(current.id);
    await refresh();
    showToast(t("deleted"), 1200);
  }

  async function bulk(minutes: number | null) {
    const n = await api.bulkDelete(minutes);
    detailCache.clear();
    await refresh();
    showToast(t("deletedN", { n }));
  }

  async function startEdit() {
    if (!current) return;
    const d = detailCache.get(current.id) ?? (await api.detail(current.id));
    const text = d.text ?? d.files.join("\n");
    overlay = { kind: "edit", id: current.id, text, orig: text };
  }

  function startRename() {
    if (current) overlay = { kind: "rename", id: current.id, title: current.title ?? "" };
  }

  function newSnippet(body = "") {
    overlay = snippetOverlay({ id: 0, name: "", keyword: "", body, updatedAt: 0 });
  }

  async function ocrCopy() {
    if (!current || current.kind !== "image") return;
    try {
      await api.copyOcr(current.id);
      showToast(t("copiedOcr"), 1500);
      setTimeout(() => api.hide(true), 600);
    } catch (e) {
      showToast(errText(e) === "no text recognized" ? t("noOcr") : errText(e));
    }
  }

  async function inbox() {
    if (!current) return;
    try {
      const paths = await api.exportInbox(current.id);
      showToast(t("exported", { path: paths[0] ?? "" }), 3000);
    } catch (e) {
      showToast(errText(e));
    }
  }

  async function runAi(actionId: string, label: string) {
    if (!current) return;
    const token = ++aiToken;
    overlay = { kind: "busy", label };
    try {
      const result = await api.aiRun(actionId, current.id);
      if (token !== aiToken) return;
      overlay = null;
      deliver(api.pasteText(result, true));
    } catch (e) {
      if (token !== aiToken) return;
      overlay = null;
      showToast(errText(e), 5000);
    }
  }

  function actionList(): Action[] {
    const a: Action[] = [];
    const s = settings;
    if (currentSnippet) {
      a.push({ id: "paste", label: t("paste"), hint: "↵", run: () => pasteCurrent() });
      a.push({ id: "copy", label: t("copyOnly"), hint: "Alt ↵", run: () => pasteCurrent({ copy: true }) });
      a.push({ id: "edit", label: t("editSnippet"), hint: `${t("ctrlShort")} E`, run: () => {
        overlay = snippetOverlay({ ...currentSnippet! });
      } });
      a.push({ id: "new", label: t("newSnippet"), hint: `${t("ctrlShort")} N`, run: () => newSnippet() });
      a.push({ id: "del", label: t("deleteSnippet"), hint: `${t("ctrlShort")} ${tr("Entf", "Del")}`, run: removeCurrent });
    }
    if (marked.length) {
      const g = tr("{0} Einträge zusammen einfügen", "Paste {0} entries together", marked.length);
      a.push({ id: "m-newline", group: g, label: tr("Untereinander", "One per line"), hint: "↵", run: () => pasteMarked("newline") });
      a.push({ id: "m-blank", group: g, label: tr("Mit Leerzeile dazwischen", "With a blank line between"), run: () => pasteMarked("blank") });
      a.push({ id: "m-comma", group: g, label: tr("Durch Komma getrennt", "Separated by commas"), run: () => pasteMarked("comma") });
      a.push({ id: "m-bullets", group: g, label: tr("Als Aufzählung (•)", "As bullet list (•)"), run: () => pasteMarked("bullets") });
      a.push({ id: "m-numbered", group: g, label: tr("Als nummerierte Liste", "As numbered list"), run: () => pasteMarked("numbered") });
      a.push({ id: "m-clear", group: g, label: tr("Markierung aufheben", "Clear marks"), hint: "Esc", run: () => { marked = []; } });
    }
    if (!currentSnippet && current) {
      const c = current;
      for (const q of quick) a.push({ id: `q-${q.id}`, group: tr("Schnellaktionen", "Quick actions"), label: (q.kind === "open" ? "↗ " : "") + q.label, run: () => runQuick(q) });
      a.push({ id: "paste", label: t("paste"), hint: "↵", run: () => pasteCurrent({ mode: "default" }) });
      if (c.formats.includes("text")) a.push({ id: "plain", label: t("pastePlain"), hint: s?.preferPlainText ? "↵" : "⇧ ↵", run: () => pasteCurrent({ mode: "plain" }) });
      if (c.formats.includes("html")) a.push({ id: "html", label: t("pasteHtml"), run: () => pasteCurrent({ mode: "html" }) });
      if (c.formats.includes("rtf")) a.push({ id: "rtf", label: t("pasteRtf"), run: () => pasteCurrent({ mode: "rtf" }) });
      a.push({ id: "copy", label: t("copyOnly"), hint: "Alt ↵", run: () => pasteCurrent({ copy: true }) });
      a.push({ id: "pin", label: c.pinned ? t("unpin") : t("pin"), hint: `${t("ctrlShort")} P`, run: togglePin });
      a.push({ id: "rename", label: t("rename"), hint: "F2", run: startRename });
      if (c.kind !== "image") a.push({ id: "edit", label: t("edit"), hint: `${t("ctrlShort")} E`, run: startEdit });
      if (c.kind === "image") {
        a.push({ id: "ocr", label: t("ocrCopy"), hint: `${t("ctrlShort")} T`, run: ocrCopy });
        a.push({
          id: "ocr-run",
          label: t("ocrRun"),
          run: async () => {
            await api.runOcr(c.id).catch((e) => showToast(errText(e)));
            detailCache.delete(c.id);
            await refresh();
          },
        });
      }
      if (c.kind === "text" || c.kind === "link") {
        a.push({ id: "translate", label: t("translate"), run: () => api.translateEntry(c.id).catch((e) => showToast(errText(e))) });
        a.push({ id: "correct", label: t("correct"), run: () => api.translateEntry(c.id, "correct").catch((e) => showToast(errText(e))) });
      }
      if (s?.inboxDir) a.push({ id: "inbox", label: t("inbox"), hint: `${t("ctrlShort")} I`, run: inbox });
      if (c.kind !== "image") a.push({ id: "to-snippet", label: t("newSnippet"), run: async () => newSnippet((await api.detail(c.id)).text ?? "") });
      a.push({ id: "del", label: t("remove"), hint: `${t("ctrlShort")} ${tr("Entf", "Del")}`, run: removeCurrent });
      // With quick actions on top, the normal entry actions get their own heading.
      if (quick.length) for (const x of a) x.group ??= t("actions");
      if (s?.ai.enabled && c.kind !== "image") {
        for (const act of s.ai.actions) {
          a.push({ id: `ai-${act.id}`, group: t("aiPrefix"), label: `${t("aiPrefix")}: ${act.name}`, run: () => runAi(act.id, act.name) });
        }
      }
    }
    const g = t("bulkDelete");
    a.push({ id: "b5", group: g, label: t("last5"), run: () => bulk(5) });
    a.push({ id: "b15", group: g, label: t("last15"), run: () => bulk(15) });
    a.push({ id: "b30", group: g, label: t("last30"), run: () => bulk(30) });
    a.push({ id: "b60", group: g, label: t("last60"), run: () => bulk(60) });
    a.push({ id: "b1440", group: g, label: t("last1440"), run: () => bulk(1440) });
    a.push({ id: "ball", group: g, label: t("allUnpinned"), run: () => {
        overlay = { kind: "confirm", label: t("confirmAll"), run: () => bulk(null) };
      } });
    a.push({
      id: "pause",
      label: s?.paused ? t("resume") : t("pause"),
      run: async () => {
        if (!s) return;
        await api.setPaused(!s.paused);
        settings = await api.settings();
      },
    });
    a.push({ id: "settings", label: t("settings"), hint: `${t("ctrlShort")} ,`, run: () => api.openSettings() });
    return a;
  }

  const actions = $derived.by(() => {
    if (overlay?.kind !== "actions") return [] as Action[];
    const q = actionQuery.trim().toLowerCase();
    const all = actionList();
    return q ? all.filter((a) => a.label.toLowerCase().includes(q)) : all;
  });

  let actionEl = $state<HTMLInputElement>();
  async function openActions(filter = "") {
    actionQuery = filter;
    actionSel = 0;
    overlay = { kind: "actions" };
    // The filter gets the caret, not the history search behind the menu (also after a
    // pending focusSearch()).
    await tick();
    actionEl?.focus();
    requestAnimationFrame(() => overlay?.kind === "actions" && actionEl?.focus());
  }

  async function runAction(a: Action | undefined) {
    if (!a) return;
    overlay = null;
    await a.run();
    if (!overlay) focusSearch();
  }

  function onSearch() {
    sel = 0;
    refresh();
  }

  const snippetKey = (s: Snippet) => JSON.stringify([s.name, s.keyword, s.body]);
  function snippetOverlay(snippet: Snippet): Overlay {
    // The empty "new snippet" counts as unchanged only while nothing was typed.
    return { kind: "snippet", snippet, orig: snippetKey(snippet) };
  }

  /** An editor with changes that would be lost when it closes. */
  function unsaved(o: Overlay | null): boolean {
    if (o?.kind === "edit") return o.text !== o.orig;
    if (o?.kind === "snippet") return snippetKey(o.snippet) !== o.orig;
    return false;
  }

  let keepBtn = $state<HTMLButtonElement>();
  /** Closes the overlay; an editor with unsaved changes asks first (Esc, click beside it). */
  async function closeOverlay() {
    if (overlay && (overlay.kind === "edit" || overlay.kind === "snippet") && unsaved(overlay)) {
      overlay.ask = true;
      await tick();
      keepBtn?.focus();
      return;
    }
    overlay = null;
    focusSearch();
  }

  async function keepEditing() {
    if (overlay?.kind === "edit" || overlay?.kind === "snippet") overlay.ask = false;
    await tick();
    (document.querySelector(".card.editor textarea") as HTMLTextAreaElement | null)?.focus();
  }

  async function focusSearch() {
    await tick();
    // An open menu or editor keeps its own input focused.
    if (overlay && overlay.kind !== "busy" && overlay.kind !== "confirm") return;
    searchEl?.focus();
  }

  function editCurrent() {
    if (currentSnippet) overlay = snippetOverlay({ ...currentSnippet });
    else startEdit();
  }

  function setTab(next: ViewTab) {
    tab = next;
    sel = 0;
    refresh();
  }

  async function saveSnippet(s: Snippet) {
    try {
      await api.saveSnippet(s);
      overlay = null;
      if (tab !== "snippets") tab = "snippets";
      await refresh();
      focusSearch();
    } catch (e) {
      showToast(errText(e));
    }
  }

  async function saveEdit(o: { id: number; text: string }, paste: boolean) {
    try {
      await api.updateText(o.id, o.text);
      detailCache.delete(o.id);
      overlay = null;
      if (paste) deliver(api.pasteText(o.text, false));
      else {
        await refresh();
        focusSearch();
      }
    } catch (e) {
      showToast(errText(e));
    }
  }

  function onKey(e: KeyboardEvent) {
    const ctrl = e.ctrlKey;
    const k = e.key;
    if (e.isComposing) return;
    // Held-down Enter must not confirm a dialog or paste twice.
    if (k === "Enter" && e.repeat) {
      e.preventDefault();
      return;
    }

    if (overlay) {
      if (k === "Escape") {
        e.preventDefault();
        if (overlay.kind === "busy") aiToken++;
        if ((overlay.kind === "edit" || overlay.kind === "snippet") && overlay.ask) keepEditing();
        else closeOverlay();
        return;
      }
      if (overlay.kind === "actions") {
        if (k === "ArrowDown") (e.preventDefault(), (actionSel = Math.min(actions.length - 1, actionSel + 1)));
        else if (k === "ArrowUp") (e.preventDefault(), (actionSel = Math.max(0, actionSel - 1)));
        else if (k === "Enter") (e.preventDefault(), runAction(actions[actionSel]));
        else if (matchesShortcut(e, actionsKey)) (e.preventDefault(), (overlay = null), focusSearch());
      } else if (overlay.kind === "confirm" && k === "Enter") {
        e.preventDefault();
        const run = overlay.run;
        overlay = null;
        run();
        focusSearch();
      } else if (overlay.kind === "rename" && k === "Enter") {
        e.preventDefault();
        const o = overlay;
        api
          .rename(o.id, o.title)
          .then(() => {
            detailCache.delete(o.id);
            overlay = null;
            refresh();
            focusSearch();
          })
          .catch((err) => showToast(errText(err)));
      } else if (overlay.kind === "edit" && ctrl && k === "Enter") {
        e.preventDefault();
        saveEdit(overlay, true);
      } else if (overlay.kind === "edit" && ctrl && k.toLowerCase() === "s") {
        e.preventDefault();
        saveEdit(overlay, false);
      } else if (overlay.kind === "fields" && k === "Enter" && !e.shiftKey) {
        e.preventDefault();
        submitFields();
      } else if (overlay.kind === "snippet" && ctrl && (k.toLowerCase() === "s" || k === "Enter")) {
        e.preventDefault();
        saveSnippet(overlay.snippet);
      }
      return;
    }

    if (matchesShortcut(e, actionsKey)) {
      e.preventDefault();
      openActions();
      return;
    }
    if (k === " " && e.shiftKey && !ctrl && tab !== "snippets") {
      e.preventDefault();
      toggleMark(sel);
      return;
    }
    if (k === "Enter" && marked.length && !ctrl && !e.shiftKey) {
      e.preventDefault();
      pasteMarked("newline", e.altKey);
      return;
    }
    if (k === "Escape" && marked.length) {
      e.preventDefault();
      marked = [];
      return;
    }
    if (k === "Escape") {
      e.preventDefault();
      if (query) {
        query = "";
        sel = 0;
        refresh();
      } else api.hide(true);
    } else if (k === "ArrowDown") (e.preventDefault(), (sel = Math.max(0, Math.min(count - 1, sel + cols))));
    else if (k === "ArrowUp") (e.preventDefault(), (sel = Math.max(0, sel - cols)));
    else if (cols > 1 && !ctrl && (k === "ArrowLeft" || k === "ArrowRight") && tabArrowAllowed(k)) {
      e.preventDefault();
      sel = Math.max(0, Math.min(count - 1, sel + (k === "ArrowLeft" ? -1 : 1)));
    } else if (cols > 1 && ctrl && (k === "ArrowLeft" || k === "ArrowRight")) {
      e.preventDefault();
      const i = TABS.indexOf(tab);
      setTab(TABS[(i + (k === "ArrowLeft" ? TABS.length - 1 : 1)) % TABS.length]);
    }
    else if (k === "PageDown") (e.preventDefault(), (sel = Math.max(0, Math.min(count - 1, sel + 8))));
    else if (k === "PageUp") (e.preventDefault(), (sel = Math.max(0, sel - 8)));
    else if (ctrl && k === "Home") (e.preventDefault(), (sel = 0));
    else if (ctrl && k === "End") (e.preventDefault(), (sel = Math.max(0, count - 1)));
    else if (k === "Enter") {
      e.preventDefault();
      pasteCurrent({ alt: e.shiftKey, copy: e.altKey || ctrl });
    } else if ((k === "ArrowLeft" || k === "ArrowRight") && !ctrl && !e.shiftKey && !e.altKey && tabArrowAllowed(k)) {
      e.preventDefault();
      const i = TABS.indexOf(tab);
      setTab(TABS[(i + (k === "ArrowLeft" ? TABS.length - 1 : 1)) % TABS.length]);
    } else if (k === "Tab") {
      e.preventDefault();
      const i = TABS.indexOf(tab);
      setTab(TABS[(i + (e.shiftKey ? TABS.length - 1 : 1)) % TABS.length]);
    } else if (tabByAlt(e) !== null) {
      e.preventDefault();
      const t = TABS[tabByAlt(e)!];
      if (t) setTab(t);
    } else if (ctrl && /^[1-9]$/.test(k)) (e.preventDefault(), pasteIndex(Number(k) - 1));
    else if (ctrl && k.toLowerCase() === "p") (e.preventDefault(), togglePin());
    else if (ctrl && k.toLowerCase() === "e") (e.preventDefault(), editCurrent()); else if (k === "F2" || (ctrl && k.toLowerCase() === "r")) (e.preventDefault(), startRename());
    else if (ctrl && k.toLowerCase() === "n") (e.preventDefault(), newSnippet());
    else if (ctrl && k.toLowerCase() === "t") (e.preventDefault(), ocrCopy());
    else if (ctrl && k.toLowerCase() === "i") (e.preventDefault(), inbox());
    else if (ctrl && k === ",") (e.preventDefault(), api.openSettings());
    else if (ctrl && !query && (k === "Delete" || k === "Backspace")) (e.preventDefault(), removeCurrent());
  }

  /** ←/→ switch tabs unless the caret should move inside the search text. */
  function tabArrowAllowed(k: string): boolean {
    if (!query) return true;
    const el = searchEl;
    if (!el || document.activeElement !== el) return true;
    const atStart = el.selectionStart === 0 && el.selectionEnd === 0;
    const atEnd = el.selectionStart === query.length && el.selectionEnd === query.length;
    return k === "ArrowLeft" ? atStart : atEnd;
  }

  onMount(() => {
    const unlisten: Promise<() => void>[] = [];
    let refreshTimer: ReturnType<typeof setTimeout> | undefined;
    (async () => {
      settings = await api.settings();
      i18n.lang = settings.language;
      applyTheme(settings.theme);
      applyLook(settings.layout, settings.design);
      await refresh();
    })();
    unlisten.push(
      listen("history-changed", () => {
        detailCache.clear();
        clearTimeout(refreshTimer);
        refreshTimer = setTimeout(refresh, 60);
      }),
    );
    unlisten.push(
      listen("palette-shown", async () => {
        // An editor with unsaved changes survives closing the window (e.g. a click into another program).
        const draft = unsaved(overlay);
        if (draft && (overlay?.kind === "edit" || overlay?.kind === "snippet")) overlay.ask = false;
        else overlay = null;
        aiToken++;
        toast = null;
        if (!withinGrace()) {
          query = "";
          tab = "all";
          sel = 0;
          marked = [];
        }
        await refresh();
        if (draft) {
          showToast(tr("Deine ungespeicherten Änderungen sind noch da.", "Your unsaved changes are still here."));
          keepEditing();
        } else focusSearch();
        // A typed keyword whose snippet has input fields: show the form right away.
        const formId = await api.takeSnippetForm().catch(() => null);
        if (formId != null) {
          const all: Snippet[] = await api.snippets("").catch(() => []);
          const sn = all.find((x) => x.id === formId);
          if (sn) pasteSnippetById(sn.id, sn.name, false);
        }
      }),
    );
    unlisten.push(
      listen("settings-changed", async () => {
        settings = await api.settings();
        i18n.lang = settings.language;
        applyTheme(settings.theme);
        applyLook(settings.layout, settings.design);
      }),
    );
    focusSearch();
    return () => unlisten.forEach((p) => p.then((f) => f()));
  });

</script>

<svelte:window onkeydown={onKey} />

<div class="shell view" data-layout={layout}>
  {#if layout === "board"}
    <Board {vm} {act} bind:query bind:searchEl onsearch={onSearch} />
  {:else if layout === "drawer"}
    <Drawer {vm} {act} bind:query bind:searchEl onsearch={onSearch} />
  {:else if layout === "compact"}
    <Compact {vm} {act} bind:query bind:searchEl onsearch={onSearch} />
  {:else}
    <Fluent {vm} {act} bind:query bind:searchEl onsearch={onSearch} />
  {/if}

  {#if overlay}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="veil" onclick={(e) => e.target === e.currentTarget && overlay?.kind !== "busy" && closeOverlay()}>
      {#if overlay.kind === "actions"}
        <div class="card actions">
          <!-- svelte-ignore a11y_autofocus -->
          <input autofocus bind:this={actionEl} bind:value={actionQuery} oninput={() => (actionSel = 0)} placeholder={t("actionFilter")} spellcheck="false" />
          <div class="alist">
            {#each actions as a, i (a.id)}
              {#if a.group && (i === 0 || actions[i - 1].group !== a.group)}<div class="group">{a.group}</div>{/if}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div class="aitem" class:sel={i === actionSel} role="option" tabindex="-1" aria-selected={i === actionSel} onmousemove={() => (actionSel = i)} onclick={() => runAction(a)}>
                <span>{a.label}</span>{#if a.hint}<kbd>{a.hint}</kbd>{/if}
              </div>
            {/each}
          </div>
        </div>
      {:else if overlay.kind === "edit"}
        <div class="card editor">
          <h3>{t("edit")}</h3>
          <!-- svelte-ignore a11y_autofocus -->
          <textarea autofocus bind:value={overlay.text} spellcheck="false"></textarea>
          {#if overlay.ask}
            <div class="discard" role="alert">
              <span>{tr("Ungespeicherte Änderungen verwerfen?", "Discard unsaved changes?")}</span>
              <button bind:this={keepBtn} class="primary" onclick={keepEditing}>{tr("Weiter bearbeiten", "Keep editing")} <kbd>Esc</kbd></button>
              <button class="danger" onclick={() => ((overlay = null), focusSearch())}>{tr("Verwerfen", "Discard")}</button>
            </div>
          {/if}
          <div class="buttons">
            <button onclick={() => ((overlay = null), focusSearch())}>{t("cancel")} <kbd>Esc</kbd></button>
            <button onclick={() => overlay?.kind === "edit" && deliver(api.pasteText(overlay.text, false))}>{t("pasteOnce")}</button>
            <button onclick={() => overlay?.kind === "edit" && saveEdit(overlay, false)}>{t("save")} <kbd>{t("ctrlShort")} S</kbd></button>
            <button class="primary" onclick={() => overlay?.kind === "edit" && saveEdit(overlay, true)}>{t("saveAndPaste")} <kbd>{t("ctrlShort")} ↵</kbd></button>
          </div>
        </div>
      {:else if overlay.kind === "rename"}
        <div class="card small">
          <h3>{t("rename")}</h3>
          <!-- svelte-ignore a11y_autofocus -->
          <input autofocus bind:value={overlay.title} placeholder={t("title")} spellcheck="false" />
        </div>
      {:else if overlay.kind === "snippet"}
        <div class="card editor">
          <h3>{overlay.snippet.id ? t("editSnippet") : t("newSnippet")}</h3>
          <div class="row">
            <!-- svelte-ignore a11y_autofocus -->
            <input autofocus bind:value={overlay.snippet.name} placeholder={t("name")} />
            <input bind:value={overlay.snippet.keyword} placeholder={t("keyword")} class="kw" />
          </div>
          <textarea bind:value={overlay.snippet.body} placeholder={t("body")} spellcheck="false"></textarea>
          <p class="help">{t("placeholdersHelp")}</p>
          {#if overlay.ask}
            <div class="discard" role="alert">
              <span>{tr("Ungespeicherte Änderungen verwerfen?", "Discard unsaved changes?")}</span>
              <button bind:this={keepBtn} class="primary" onclick={keepEditing}>{tr("Weiter bearbeiten", "Keep editing")} <kbd>Esc</kbd></button>
              <button class="danger" onclick={() => ((overlay = null), focusSearch())}>{tr("Verwerfen", "Discard")}</button>
            </div>
          {/if}
          <div class="buttons">
            <button onclick={() => ((overlay = null), focusSearch())}>{t("cancel")} <kbd>Esc</kbd></button>
            <button class="primary" onclick={() => overlay?.kind === "snippet" && saveSnippet(overlay.snippet)}>{t("save")} <kbd>{t("ctrlShort")} S</kbd></button>
          </div>
        </div>
      {:else if overlay.kind === "fields"}
        <div class="card editor">
          <h3>{overlay.name}</h3>
          {#each overlay.fields as f, i (f.name)}
            <label class="ffield">
              <span>{f.name}</span>
              {#if f.options.length}
                <select bind:value={overlay.values[f.name]}>{#each f.options as o (o)}<option value={o}>{o}</option>{/each}</select>
              {:else}
                <!-- svelte-ignore a11y_autofocus -->
                <input autofocus={i === 0} bind:value={overlay.values[f.name]} spellcheck="false" />
              {/if}
            </label>
          {/each}
          <div class="buttons">
            <button onclick={() => ((overlay = null), focusSearch())}>{t("cancel")} <kbd>Esc</kbd></button>
            <button class="primary" onclick={submitFields}>{t("paste")} <kbd>↵</kbd></button>
          </div>
        </div>
      {:else if overlay.kind === "busy"}
        <div class="card small busy">
          <div class="spinner"></div>
          <div>
            <strong>{t("aiWorking")}</strong>
            <div class="help">{overlay.label} · {t("escCancel")}</div>
          </div>
        </div>
      {:else if overlay.kind === "confirm"}
        <div class="card small"><p>{overlay.label}</p></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .shell {
    position: relative;
    height: 100%;
  }
  .veil {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 56px;
  }
  .shell[data-layout="fluent"] .veil {
    background: rgba(0, 0, 0, 0.18);
  }
  .card {
    background: var(--bg-solid);
    color: var(--text);
    font-family: var(--font);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    padding: 12px;
    width: min(560px, 92%);
    max-height: calc(100% - 80px);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card.small {
    width: min(420px, 90%);
  }
  .discard {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 10px;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  .discard span {
    flex: 1;
    min-width: 180px;
    font-weight: 600;
  }
  .discard kbd {
    margin-left: 4px;
  }
  .discard button.danger {
    color: var(--danger);
  }
  .card.editor {
    width: min(720px, 94%);
    height: calc(100% - 80px);
  }
  .card input {
    width: 100%;
  }
  .card textarea {
    flex: 1;
    resize: none;
    font: 13px/1.45 var(--mono);
    min-height: 120px;
  }
  h3 {
    margin: 0;
    font: 600 15px var(--display);
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .row .kw {
    width: 160px;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 8px;
  }
  .buttons kbd {
    margin-left: 4px;
  }
  .help {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .alist {
    overflow-y: auto;
  }
  .group {
    font-size: 11px;
    color: var(--faint);
    padding: 8px 8px 2px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .aitem {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 8px;
    border-radius: 6px;
  }
  .aitem.sel {
    background: var(--sel);
  }
  .busy {
    flex-direction: row;
    align-items: center;
    gap: 14px;
  }
  .spinner {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 3px solid var(--accent-soft);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .ffield {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--muted);
  }
  .ffield input,
  .ffield select {
    font-size: 13.5px;
    color: var(--text);
  }
</style>
