<script lang="ts">
  import { onMount } from "svelte";
  import { api, errText, SERVICE_KEYS, type AiAction, type AiProvider, type Conflict, type Language, type MonthUsage, type Settings } from "./api";
  import { i18n, tr, locale, languageName, LANGS } from "./i18n.svelte";
  import KeyHelp from "./KeyHelp.svelte";
  import Transfer from "./Transfer.svelte";
  import VersionBar from "./VersionBar.svelte";
  import { appShortcutHint } from "./hotkeyHints";

  const REPO = "https://github.com/TheBigS1981/UltimatePaste";
  import LangSelect from "./LangSelect.svelte";
  import Stats from "./Stats.svelte";
  import HotkeyPicker from "./HotkeyPicker.svelte";
  import { applyLook, applyTheme, resolveDesign } from "./theme";

  let s = $state<Settings | null>(null);
  let info = $state<{ version: string; dataDir: string; entries: number } | null>(null);
  let status = $state("");
  let keys = $state<Record<string, string>>({});
  let hasKey = $state<Record<string, boolean>>({});
  let blocked = $state("");
  let tags = $state("");
  // ?section= is only used by the dev screenshot harness.
  let section = $state(new URLSearchParams(location.search).get("section") ?? "general");
  let hkError = $state("");

  function pickMain(v: string) {
    if (!s) return;
    hkError = "";
    s.hotkey = v;
    changed();
  }
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let statusTimer: ReturnType<typeof setTimeout> | undefined;


  const LAYOUTS = $derived([
    { v: "fluent", name: "Fluent", de: "Wie Windows 11: Liste, Vorschau, Befehlsleiste", en: "Like Windows 11: list, preview, command bar" },
    { v: "board", name: tr("Pinnwand", "Board"), de: "Inhalte als Karten im Raster", en: "Content as cards in a grid" },
    { v: "drawer", name: tr("Schublade", "Drawer"), de: "Zeitleiste am rechten Bildschirmrand", en: "Timeline at the right screen edge" },
    { v: "compact", name: tr("Kompakt", "Compact"), de: "Kleines Popup am Text-Cursor", en: "Small popup at the text caret" },
  ] as const);

  const DESIGNS = $derived([
    { v: "windows", name: "Windows", de: "Segoe, Windows-Blau", en: "Segoe, Windows blue" },
    { v: "coral", name: tr("Koralle", "Coral"), de: "Warm, rundlich", en: "Warm, rounded" },
    { v: "sage", name: tr("Salbei", "Sage"), de: "Ruhig, IBM Plex", en: "Calm, IBM Plex" },
    { v: "violet", name: tr("Violett", "Violet"), de: "Klar, Inter", en: "Crisp, Inter" },
    { v: "graphite", name: tr("Graphit", "Graphite"), de: "Schwarz-weiß, kontrastreich", en: "Monochrome, high contrast" },
    { v: "ocean", name: tr("Ozean", "Ocean"), de: "Petrol und Tiefblau", en: "Teal and deep blue" },
    { v: "rose", name: tr("Rosé", "Rosé"), de: "Beere und Rosa", en: "Berry and pink" },
    { v: "amber", name: tr("Bernstein", "Amber"), de: "Papier und Honig", en: "Paper and honey" },
  ]);

  function pickDesign(v: string) {
    if (!s) return;
    s.design = v;
    applyLook(s.layout, v);
    changed();
  }

  const RETENTION = [
    { v: 1, de: "1 Tag", en: "1 day" },
    { v: 3, de: "3 Tage", en: "3 days" },
    { v: 7, de: "1 Woche", en: "1 week" },
    { v: 14, de: "2 Wochen", en: "2 weeks" },
    { v: 21, de: "3 Wochen", en: "3 weeks" },
    { v: 30, de: "1 Monat", en: "1 month" },
    { v: 90, de: "3 Monate", en: "3 months" },
    { v: 180, de: "6 Monate", en: "6 months" },
    { v: 365, de: "1 Jahr", en: "1 year" },
    { v: 0, de: "Unbegrenzt", en: "Unlimited" },
  ];

  async function reloadSettings() {
    s = await api.settings();
    i18n.lang = s.language;
    applyTheme(s.theme);
    applyLook(s.layout, s.design);
    blocked = s.blockedApps.join("\n");
    tags = s.inboxTags.join(", ");
    info = await api.appInfo();
    for (const p of s.ai.providers) hasKey[p.id] = await api.aiHasKey(p.id);
    for (const k of Object.values(SERVICE_KEYS)) hasKey[k] = await api.aiHasKey(k);
  }

  onMount(async () => {
    await reloadSettings();
    langs = await api.languages();
  });

  let langs = $state<Language[]>([]);
  let recentApps = $state<string[]>([]);
  $effect(() => {
    if (section === "rules") api.recentApps().then((a) => (recentApps = a));
  });
  let usage = $state<{ month: MonthUsage; deeplUsed: number | null; deeplLimit: number | null; deeplError: string | null } | null>(null);

  async function saveServiceKey(name: string) {
    try {
      await api.aiSetKey(name, keys[name] || null);
      hasKey[name] = !!keys[name];
      keys[name] = "";
      status = tr("API-Schlüssel gespeichert ✓", "API key saved ✓");
      clearTimeout(statusTimer);
      statusTimer = setTimeout(() => (status = ""), 2200);
    } catch (e) {
      status = errText(e);
    }
  }

  async function loadUsage() {
    usage = null;
    usage = await api.translateUsage();
  }

  $effect(() => {
    if (section === "translate") loadUsage();
    if (["paste", "picker", "translate", "sequential"].includes(section)) loadConflicts();
  });

  let conflicts = $state<Conflict[]>([]);
  let confirmWinV = $state<boolean | null>(null);
  let winVBusy = $state(false);
  async function loadConflicts() {
    conflicts = await api.hotkeyConflicts().catch(() => []);
  }
  async function applyWinV(off: boolean) {
    winVBusy = true;
    try {
      conflicts = await api.setWindowsV(off);
      confirmWinV = null;
    } catch (e) {
      hkError = errText(e);
    } finally {
      winVBusy = false;
    }
  }

  /** Built-in Claude prices (USD per million tokens), same table as the backend. */
  const BUILTIN_PRICES: [string, number, number][] = [
    ["claude-opus-5-5", 4, 20],
    ["claude-opus-5", 5, 25],
    ["claude-sonnet-5", 2, 10],
    ["claude-haiku-5", 0.1, 0.5],
    ["claude-fable-5-2", 8, 40],
    ["claude-fable-5", 10, 50],
  ];
  const builtinPrice = (model: string) => BUILTIN_PRICES.find(([p]) => model.startsWith(p));
  const styleOptions = () => [
    ["", tr("Keiner – nur flüssiger", "None – just smoother")],
    ["simple", tr("Stil: einfach", "Style: simple")],
    ["business", tr("Stil: geschäftlich", "Style: business")],
    ["academic", tr("Stil: akademisch", "Style: academic")],
    ["casual", tr("Stil: locker", "Style: casual")],
    ["tone:friendly", tr("Ton: freundlich", "Tone: friendly")],
    ["tone:confident", tr("Ton: selbstbewusst", "Tone: confident")],
    ["tone:diplomatic", tr("Ton: diplomatisch", "Tone: diplomatic")],
    ["tone:enthusiastic", tr("Ton: begeistert", "Tone: enthusiastic")],
  ];
  const translateProvider = $derived(s ? (s.ai.providers.find((p) => p.id === (s!.translate.aiProvider || s!.ai.defaultProvider)) ?? null) : null);
  const translateModel = $derived(s ? s.translate.aiModel.trim() || translateProvider?.model || "" : "");
  const nf = (n: number) => n.toLocaleString(locale());
  const langLabel = (l: Language) => languageName(l.code, l.de, l.en);
  const langOpts = $derived(langs.map((l) => ({ code: l.code, label: langLabel(l) })));

  function setCustomPrice(i: 0 | 1, v: number) {
    if (!s || !translateModel) return;
    const cur = s.translate.customPrices[translateModel] ?? [0, 0];
    cur[i] = v;
    s.translate.customPrices[translateModel] = [cur[0], cur[1]];
    changed();
  }

  async function save() {
    if (!s) return;
    s.blockedApps = blocked
      .split(/\r?\n/)
      .map((x) => x.trim().toLowerCase())
      .filter(Boolean);
    s.inboxTags = tags
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
    i18n.lang = s.language;
    applyTheme(s.theme);
    applyLook(s.layout, s.design);
    try {
      const r = await api.saveSettings($state.snapshot(s) as Settings);
      // A shortcut Windows refused is reverted by the backend; show what is really active.
      s.hotkey = r.hotkey;
      s.sequentialHotkey = r.sequentialHotkey;
      s.pickerHotkey = r.pickerHotkey;
      s.translateHotkey = r.translateHotkey;
      s.translateAltHotkey = r.translateAltHotkey;
      s.correctHotkey = r.correctHotkey;
      hkError = r.hotkeyErrors.join(" ");
      if (["paste", "picker", "translate", "sequential"].includes(section)) loadConflicts();
      status = r.hotkeyErrors.length ? "" : tr("Gespeichert ✓", "Saved ✓");
      clearTimeout(statusTimer);
      statusTimer = setTimeout(() => (status = ""), 2200);
    } catch (e) {
      status = errText(e);
    }
  }

  /** Auto-save shortly after any change. */
  function changed() {
    status = "";
    clearTimeout(saveTimer);
    saveTimer = setTimeout(save, 400);
  }

  async function saveKey(p: AiProvider) {
    try {
      await api.aiSetKey(p.id, keys[p.id] || null);
      hasKey[p.id] = !!keys[p.id];
      keys[p.id] = "";
      status = tr("API-Key gespeichert ✓", "API key saved ✓");
    } catch (e) {
      status = errText(e);
    }
  }

  function addProvider() {
    if (!s) return;
    const id = `p${Date.now().toString(36)}`;
    s.ai.providers.push({ id, name: tr("Neuer Anbieter", "New provider"), kind: "openai", baseUrl: "http://localhost:1234/v1", model: "" });
    changed();
  }

  function addAction() {
    if (!s) return;
    s.ai.actions.push({ id: `a${Date.now().toString(36)}`, name: tr("Neue Aktion", "New action"), prompt: "", provider: "", builtin: false });
    changed();
  }

  function removeAt<T>(arr: T[], i: number) {
    arr.splice(i, 1);
    changed();
  }

  function moveAction(i: number, d: number) {
    if (!s) return;
    const a = s.ai.actions;
    const j = i + d;
    if (j < 0 || j >= a.length) return;
    [a[i], a[j]] = [a[j], a[i]];
    changed();
  }

  async function clearAll() {
    if (!confirm(tr("Alle nicht angehefteten Einträge löschen?", "Delete all unpinned entries?"))) return;
    const n = await api.bulkDelete(null);
    status = tr("{0} Einträge gelöscht", "{0} entries deleted", n);
    info = await api.appInfo();
  }

  const SECTIONS = $derived([
    ["general", tr("Allgemein", "General")],
    ["#copy", tr("Kopieren & Einfügen", "Copy & paste")],
    ["paste", tr("Einfüge-Fenster", "Paste window")],
    ["history", tr("Verlauf & Datenschutz", "History & privacy")],
    ["sequential", tr("Nacheinander einfügen", "Paste sequentially")],
    ["snippets", tr("Snippets", "Snippets")],
    ["rules", tr("Regeln je Programm", "Rules per program")],
    ["ocr", tr("Texterkennung & Inbox", "OCR & inbox")],
    ["#text", tr("Übersetzen & Korrigieren", "Translate & correct")],
    ["translate", tr("Dienste & Sprachen", "Services & languages")],
    ["#emoji", tr("Emoji & GIF", "Emoji & GIF")],
    ["picker", tr("Auswahlfenster", "Picker")],
    ["#more", tr("Weiteres", "More")],
    ["ai", tr("KI", "AI")],
    ["stats", tr("Statistik", "Statistics")],
    ["about", tr("Info", "About")],
  ]);
</script>

{#snippet conflictNote(id: number)}
  {#each conflicts.filter((c) => c.id === id) as c (c.kind + (c.module ?? ""))}
    <div class="conflict" class:ok={c.resolved}>
      {#if c.kind === "windows-v"}
        {#if c.resolved}
          <span>✓ {tr("Windows' eigenes Win + V (Zwischenablage) ist abgeschaltet.", "Windows' own Win + V (clipboard) is switched off.")}</span>
          <button type="button" disabled={winVBusy} onclick={() => (confirmWinV = false)}>{tr("Windows-Kürzel wiederherstellen", "Restore Windows shortcut")}</button>
        {:else}
          <span>{tr("Windows nutzt Win + V selbst (Zwischenablage). UltimatePaste fängt das Kürzel ab – zuverlässiger ist es, das Windows-Kürzel abzuschalten.", "Windows uses Win + V itself (clipboard). UltimatePaste intercepts it – switching off the Windows shortcut is more reliable.")}</span>
          <button type="button" disabled={winVBusy} onclick={() => (confirmWinV = true)}>{tr("In Windows abschalten", "Switch off in Windows")}</button>
        {/if}
        {#if confirmWinV !== null}
          <div class="confirm">
            <span>{tr("Das wirkt ab der nächsten Windows-Anmeldung (abmelden und wieder anmelden oder neu starten).", "This takes effect from the next Windows sign-in (sign out and in again, or restart).")}{#if confirmWinV}{" "}{tr("Windows' Win + Umschalt + V wird ebenfalls frei.", "Windows' Win + Shift + V is freed as well.")}{/if}</span>
            <button type="button" class="primary" disabled={winVBusy} onclick={() => applyWinV(confirmWinV!)}>{winVBusy ? "…" : confirmWinV ? tr("Jetzt abschalten", "Switch off now") : tr("Jetzt wiederherstellen", "Restore now")}</button>
            <button type="button" disabled={winVBusy} onclick={() => (confirmWinV = null)}>{tr("Abbrechen", "Cancel")}</button>
          </div>
        {/if}
      {:else if c.kind === "windows-emoji"}
        <span>{tr("Windows nutzt dieses Kürzel selbst für sein Emoji-Feld. UltimatePaste fängt es ab; das Auswahlfenster übernimmt dabei bewusst nicht den Fokus, damit Windows nicht dazwischenkommt.", "Windows uses this shortcut for its own emoji panel. UltimatePaste intercepts it; the picker deliberately doesn't take the focus so Windows can't get in the way.")}</span>
      {:else}
        <span>{tr("Auch PowerToys nutzt dieses Kürzel ({0}). Bitte in den PowerToys-Einstellungen ändern oder das Modul abschalten.", "PowerToys uses this shortcut too ({0}). Please change it in the PowerToys settings or switch the module off.", c.module)}</span>
      {/if}
    </div>
  {/each}
{/snippet}

{#if status}<div class="saved" role="status">{status}</div>{/if}

{#if s}
  <div class="layout">
    <aside>
      <h1>UltimatePaste</h1>
      {#each SECTIONS as [id, label] (id)}
        {#if id.startsWith("#")}<div class="grp">{label}</div>{:else}<button class:active={section === id} onclick={() => (section = id)}>{label}</button>{/if}
      {/each}
      <VersionBar />
    </aside>

    <div class="body">
      {#if hkError}<p class="hkerr" role="alert">{hkError}</p>{/if}
      {#if section === "general"}
        <h2>{tr("Allgemein", "General")}</h2>
        <p class="intro">{tr("Sprache, Darstellung und Start mit Windows.", "Language, appearance and starting with Windows.")}</p>
        <div class="field">
          <span>{tr("Sprache", "Language")}</span>
          <LangSelect label={tr("Sprache", "Language")} value={s.language} options={LANGS.map((l) => ({ code: l.code, label: l.name }))} onchange={(v) => ((s!.language = v as Settings["language"]), changed())} />
        </div>
        <div class="field">
          <span>{tr("Darstellung", "Appearance")}</span>
          <div class="seg" role="group">
            {#each [["system", tr("System", "System")], ["light", tr("Hell", "Light")], ["dark", tr("Dunkel", "Dark")]] as [v, l] (v)}
              <button type="button" class:on={s.theme === v} aria-pressed={s.theme === v} onclick={() => ((s!.theme = v as Settings["theme"]), applyTheme(s!.theme), changed())}>{l}</button>
            {/each}
          </div>
        </div>
        <label class="check"><input type="checkbox" bind:checked={s.autostart} onchange={changed} /> {tr("Mit Windows starten", "Start with Windows")}</label>
        <label class="check"><input type="checkbox" bind:checked={s.updateCheck} onchange={changed} /> {tr("Automatisch nach neuen Versionen suchen (beim Start und täglich, nur Abfrage bei GitHub)", "Check for new versions automatically (at start and daily, only a request to GitHub)")}</label>
        <div><button onclick={() => api.openTour()}>{tr("Einführungstour erneut starten", "Restart the welcome tour")}</button></div>
        <h3>{tr("Einstellungen sichern & übertragen", "Back up & transfer settings")}</h3>
        <Transfer onimported={reloadSettings} />
      {:else if section === "paste"}
        <h2>{tr("Einfüge-Fenster", "Paste window")}</h2>
        <p class="intro">{tr("Dein Zwischenablage-Verlauf: Kürzel, Aussehen und was beim Einfügen passiert.", "Your clipboard history: shortcut, look and what happens when you paste.")}</p>
        <div class="field">
          <span>{tr("Einfüge-Fenster öffnen", "Open paste window")}</span>
          <HotkeyPicker value={s.hotkey} onchange={pickMain} />
          {#if appShortcutHint(s.hotkey)}<small class="apphint">⚠ {appShortcutHint(s.hotkey)}</small>{/if}
          {@render conflictNote(0)}
          <small>{tr("Auch Kombinationen, die Windows selbst benutzt (z. B. Win + V), sind möglich – UltimatePaste übernimmt sie dann.", "Combinations Windows uses itself (e.g. Win + V) work too – UltimatePaste takes them over.")}</small>
        </div>
        <div class="field">
          <span>{tr("Aktionsmenü im Einfüge-Fenster", "Actions menu in the paste window")}</span>
          <HotkeyPicker value={s.actionsKey || "Ctrl+Enter"} local onchange={(v) => ((s!.actionsKey = v), changed())} />
          <small>{tr("Gilt nur im Fenster. Nur kopieren (ohne Einfügen) liegt auf Alt + Eingabe.", "Only inside the window. Copy only (without pasting) is Alt + Enter.")}</small>
        </div>
        <div class="field">
          <span>{tr("Fensterstil", "Window style")}</span>
          <div class="layouts">
            {#each LAYOUTS as l (l.v)}
              <button type="button" class="lay" class:on={s.layout === l.v} aria-pressed={s.layout === l.v} onclick={() => ((s!.layout = l.v), applyLook(l.v, s!.design), changed())}>
                <span class="mini {l.v}" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
                <b>{l.name}</b>
                <small>{tr(l.de, l.en)}</small>
              </button>
            {/each}
          </div>
        </div>
        <div class="field">
          <span>{tr("Design", "Design")}</span>
          <div class="designs">
            <button type="button" class="des" class:on={s.design === "auto"} aria-pressed={s.design === "auto"} onclick={() => pickDesign("auto")}>
              <span class="dprev" data-design-preview={resolveDesign(s.layout, "auto")} aria-hidden="true"><span class="bar"></span><span class="dot"></span><span class="aa">Aa</span></span>
              <b>{tr("Passend", "Matching")}</b>
              <small>{tr("Gehört zum Fensterstil", "Belongs to the window style")}</small>
            </button>
            {#each DESIGNS as d (d.v)}
              <button type="button" class="des" class:on={s.design === d.v} aria-pressed={s.design === d.v} onclick={() => pickDesign(d.v)}>
                <span class="dprev" data-design-preview={d.v} aria-hidden="true"><span class="bar"></span><span class="dot"></span><span class="aa">Aa</span></span>
                <b>{d.name}</b>
                <small>{tr(d.de, d.en)}</small>
              </button>
            {/each}
          </div>
        </div>
        {#if s.layout === "fluent" || s.layout === "board"}
        <label class="field">
          <span>{tr("Fensterposition", "Window position")}</span>
          <select bind:value={s.position} onchange={changed}>
            <option value="center">{tr("Mitte des aktiven Bildschirms", "Center of active screen")}</option>
            <option value="cursor">{tr("An der Maus", "At mouse cursor")}</option>
          </select>
        </label>
        {/if}
        <label class="field">
          <span>{tr("Enter bewirkt", "Enter does")}</span>
          <select bind:value={s.primaryAction} onchange={changed}>
            <option value="paste">{tr("Einfügen", "Paste")}</option>
            <option value="copy">{tr("Nur kopieren", "Copy only")}</option>
          </select>
        </label>
        <label class="check"><input type="checkbox" bind:checked={s.preferPlainText} onchange={changed} /> {tr("Standardmäßig als reinen Text einfügen (Shift+Enter = mit Formatierung)", "Paste as plain text by default (Shift+Enter = with formatting)")}</label>
        <label class="check"><input type="checkbox" bind:checked={s.moveUsedToTop} onchange={changed} /> {tr("Eingefügte Einträge nach oben verschieben", "Move pasted entries to the top")}</label>
      {:else if section === "picker"}
        <h2>{tr("Emoji & GIF", "Emoji & GIF")}</h2>
        <p class="intro">{tr("Das Auswahlfenster für Emojis, GIFs und Sonderzeichen.", "The picker for emoji, GIFs and symbols.")}</p>
        <div class="field">
          <span>{tr("Emoji, GIF & Sonderzeichen", "Emoji, GIF & symbols")}</span>
          <HotkeyPicker value={s.pickerHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.pickerHotkey = v), changed())} />
          {#if appShortcutHint(s.pickerHotkey)}<small class="apphint">⚠ {appShortcutHint(s.pickerHotkey)}</small>{/if}
          {@render conflictNote(2)}
        </div>
        <div class="row">
          <label class="field">
            <span>{tr("„Zuletzt verwendet“ zeigt", "“Recently used” shows")}</span>
            <select bind:value={s.picker.recentCount} onchange={changed}>
              {#each [8, 16, 24, 32, 48, 64] as n (n)}<option value={n}>{n} {tr("Einträge", "entries")}</option>{/each}
            </select>
          </label>
          <div class="field">
            <span>{tr("Standard-Hautfarbe", "Default skin tone")}</span>
            <div class="tones">
              {#each ["👋", "👋🏻", "👋🏼", "👋🏽", "👋🏾", "👋🏿"] as e, i (i)}
                <button type="button" class:on={s.picker.skinTone === i} aria-pressed={s.picker.skinTone === i} onclick={() => ((s!.picker.skinTone = i), changed())}>{e}</button>
              {/each}
            </div>
          </div>
        </div>

        <h3>GIF & Sticker</h3>
        <label class="check"><input type="checkbox" bind:checked={s.picker.gifEnabled} onchange={changed} /> {tr("GIF-Reiter anzeigen", "Show GIF tab")}</label>
        {#if s.picker.gifEnabled}
          <p class="help">{tr("Die Suche läuft über GIPHY oder KLIPY mit deinem eigenen, kostenlosen Schlüssel (100 Abfragen pro Stunde). Suchbegriffe gehen nur an den Dienst, wenn du im GIF-Reiter suchst.", "Search runs via GIPHY or KLIPY with your own free key (100 requests per hour). Search terms only go to the service when you search in the GIF tab.")}</p>
          {#each [["giphy", "GIPHY", "developers.giphy.com"], ["klipy", "KLIPY", "partner.klipy.com"]] as [id, name, site] (id)}
            {@const k = SERVICE_KEYS[id as "giphy" | "klipy"]}
            <div class="row end">
              <label class="field grow">
                <span>{name} API-Key {hasKey[k] ? tr("(gespeichert)", "(stored)") : `– ${site}`}</span>
                <input type="password" bind:value={keys[k]} placeholder={hasKey[k] ? "••••••••" : ""} />
              </label>
              <button onclick={() => saveServiceKey(k)}>{tr("Key speichern", "Save key")}</button>
              <label class="check"><input type="radio" name="gifprov" checked={s.picker.gifProvider === id} onchange={() => ((s!.picker.gifProvider = id as "giphy" | "klipy"), changed())} /> {tr("Standard", "Default")}</label>
            </div>
            <KeyHelp service={id as "giphy" | "klipy"} />
          {/each}
          <div class="row">
            <label class="field">
              <span>{tr("Jugendschutz", "Content rating")}</span>
              <select bind:value={s.picker.gifRating} onchange={changed}>
                <option value="g">G – {tr("für alle", "everyone")}</option>
                <option value="pg">PG – {tr("unbedenklich, auch im Büro", "safe, also at work")}</option>
                <option value="pg-13">PG-13</option>
                <option value="r">R – {tr("keine Einschränkung", "no restriction")}</option>
              </select>
            </label>
          </div>
          <label class="check"><input type="checkbox" bind:checked={s.picker.gifOriginalSize} onchange={changed} /> {tr("In Originalgröße einfügen (sonst bis ca. 2 MB, passt in jeden Chat)", "Paste in original size (otherwise up to ~2 MB, fits every chat)")}</label>
          <label class="check"><input type="checkbox" bind:checked={s.picker.gifFeedback} onchange={changed} /> {tr("Dem Dienst melden, welches GIF verschickt wurde (anonyme Geräte-ID, keine persönlichen Daten)", "Tell the service which GIF was sent (anonymous device id, no personal data)")}</label>
          <p class="help">{tr("Tipp: Lass Werbung für deinen KLIPY-Schlüssel im KLIPY-Portal ausgeschaltet. UltimatePaste zeigt ohnehin keine an.", "Tip: keep ads switched off for your KLIPY key in the KLIPY portal. UltimatePaste never shows them anyway.")}</p>
        {/if}
      {:else if section === "translate"}
        <h2>{tr("Übersetzen & Korrigieren", "Translate & correct")}</h2>
        <p class="intro">{tr("Markierten Text per Tastenkürzel übersetzen oder korrigieren – mit DeepL oder einem KI-Anbieter. Text wird nur gesendet, wenn du es auslöst.", "Translate or correct selected text with a shortcut – via DeepL or an AI provider. Text is only sent when you trigger it.")}</p>
        <div class="field">
          <span>{tr("Übersetzen", "Translate")} ({s.translate.mode === "direct" ? tr("direkt ersetzen", "replace directly") : tr("mit Fenster", "with window")})</span>
          <HotkeyPicker value={s.translateHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.translateHotkey = v), changed())} />
          {#if appShortcutHint(s.translateHotkey)}<small class="apphint">⚠ {appShortcutHint(s.translateHotkey)}</small>{/if}
          {@render conflictNote(3)}
        </div>
        <div class="field">
          <span>{tr("Übersetzen", "Translate")} ({s.translate.mode === "direct" ? tr("mit Fenster", "with window") : tr("direkt ersetzen", "replace directly")}) – {tr("optional", "optional")}</span>
          <HotkeyPicker value={s.translateAltHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.translateAltHotkey = v), changed())} />
          {#if appShortcutHint(s.translateAltHotkey)}<small class="apphint">⚠ {appShortcutHint(s.translateAltHotkey)}</small>{/if}
          {@render conflictNote(4)}
        </div>
        <div class="field">
          <span>{tr("Korrigieren & verbessern", "Correct & improve")}</span>
          <HotkeyPicker value={s.correctHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.correctHotkey = v), changed())} />
          {#if appShortcutHint(s.correctHotkey)}<small class="apphint">⚠ {appShortcutHint(s.correctHotkey)}</small>{/if}
          {@render conflictNote(5)}
        </div>
        <div class="field">
          <span>{tr("Ablauf", "Mode")}</span>
          <div class="seg" role="group">
            <button type="button" class:on={s.translate.mode === "window"} onclick={() => ((s!.translate.mode = "window"), changed())}>{tr("Fenster öffnen", "Open window")}</button>
            <button type="button" class:on={s.translate.mode === "direct"} onclick={() => ((s!.translate.mode = "direct"), changed())}>{tr("Direkt ersetzen", "Replace directly")}</button>
          </div>
          <small>{s.translate.mode === "direct" ? tr("Die Markierung wird sofort ersetzt, unten rechts erscheint ein kurzer Hinweis. Rückgängig mit Strg + Z. Ohne Markierung öffnet sich das Fenster.", "The selection is replaced right away, a short notice appears at the bottom right. Undo with Ctrl + Z. Without a selection the window opens.") : tr("Original und Übersetzung nebeneinander; Sprachen, Dienst und Anrede lassen sich dort spontan ändern.", "Original and translation side by side; languages, service and address form can be changed there.")}</small>
        </div>
        <div class="field">
          <span>{tr("Richtung", "Direction")}</span>
          <div class="seg" role="group">
            <button type="button" class:on={s.translate.rule === "pair"} onclick={() => ((s!.translate.rule = "pair"), changed())}>{tr("Sprachpaar", "Language pair")}</button>
            <button type="button" class:on={s.translate.rule === "fixed"} onclick={() => ((s!.translate.rule = "fixed"), changed())}>{tr("Immer nach …", "Always to …")}</button>
          </div>
        </div>
        {#if s.translate.rule === "pair"}
          <div class="row">
            <div class="field"><span>{tr("Sprache A", "Language A")}</span><LangSelect label={tr("Sprache A", "Language A")} value={s.translate.primary} options={langOpts} onchange={(v) => ((s!.translate.primary = v), changed())} /></div>
            <span class="arrow">⇄</span>
            <div class="field"><span>{tr("Sprache B", "Language B")}</span><LangSelect label={tr("Sprache B", "Language B")} value={s.translate.secondary} options={langOpts} onchange={(v) => ((s!.translate.secondary = v), changed())} /></div>
          </div>
          <p class="help">{tr("Wird Sprache A erkannt, geht es nach B – alles andere wird nach A übersetzt.", "If language A is detected, it goes to B – everything else is translated to A.")}</p>
        {:else}
          <div class="field"><span>{tr("Zielsprache", "Target language")}</span><LangSelect label={tr("Zielsprache", "Target language")} value={s.translate.fixedTarget} options={langOpts} onchange={(v) => ((s!.translate.fixedTarget = v), changed())} /></div>
        {/if}
        <div class="field">
          <span>{tr("Anrede (Standard)", "Address form (default)")}</span>
          <div class="seg" role="group">
            {#each [["auto", tr("Automatisch", "Automatic")], ["less", tr("Du", "Informal")], ["more", tr("Sie", "Formal")]] as [v, l] (v)}
              <button type="button" class:on={s.translate.formality === v} onclick={() => ((s!.translate.formality = v as "auto" | "less" | "more"), changed())}>{l}</button>
            {/each}
          </div>
        </div>

        <h3>{tr("Dienst", "Service")}</h3>
        <div class="seg" role="group">
          <button type="button" class:on={s.translate.engine === "deepl"} onclick={() => ((s!.translate.engine = "deepl"), changed())}>DeepL</button>
          <button type="button" class:on={s.translate.engine === "ai"} onclick={() => ((s!.translate.engine = "ai"), changed())}>{tr("KI-Anbieter", "AI provider")}</button>
        </div>
        {#if s.translate.engine === "deepl"}
          <div class="row end">
            <label class="field grow">
              <span>DeepL API-Key {hasKey[SERVICE_KEYS.deepl] ? tr("(gespeichert)", "(stored)") : "– deepl.com/pro-api"}</span>
              <input type="password" bind:value={keys[SERVICE_KEYS.deepl]} placeholder={hasKey[SERVICE_KEYS.deepl] ? "••••••••" : "…:fx"} />
            </label>
            <button onclick={() => saveServiceKey(SERVICE_KEYS.deepl)}>{tr("Key speichern", "Save key")}</button>
          </div>
          <KeyHelp service="deepl" />
          <p class="help">{tr("Free- und Pro-Schlüssel werden automatisch erkannt (Free-Schlüssel enden auf „:fx“, 500.000 Zeichen pro Monat).", "Free and Pro keys are detected automatically (Free keys end in “:fx”, 500,000 characters per month).")}</p>
        {:else}
          <div class="row">
            <label class="field">
              <span>{tr("Anbieter", "Provider")}</span>
              <select bind:value={s.translate.aiProvider} onchange={() => ((s!.translate.aiModel = ""), changed())}>
                <option value="">{tr("Standard", "Default")}</option>
                {#each s.ai.providers as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
              </select>
            </label>
            <label class="field grow"><span>{tr("Modell", "Model")}</span><input type="text" bind:value={s.translate.aiModel} placeholder={translateProvider?.model ?? ""} onchange={changed} spellcheck="false" /></label>
          </div>
          {#if translateModel}
            {@const bp = builtinPrice(translateModel)}
            {#if bp}
              <p class="help">{tr("Preis", "Price")}: {bp[1]} $ / {bp[2]} $ {tr("pro Million Tokens (Eingabe / Ausgabe)", "per million tokens (input / output)")}</p>
            {:else}
              <div class="row">
                <label class="field"><span>{tr("Preis Eingabe ($ / Mio. Tokens)", "Input price ($ / M tokens)")}</span><input type="number" min="0" step="0.01" value={s.translate.customPrices[translateModel]?.[0] ?? 0} onchange={(e) => setCustomPrice(0, Number((e.target as HTMLInputElement).value))} /></label>
                <label class="field"><span>{tr("Preis Ausgabe ($ / Mio. Tokens)", "Output price ($ / M tokens)")}</span><input type="number" min="0" step="0.01" value={s.translate.customPrices[translateModel]?.[1] ?? 0} onchange={(e) => setCustomPrice(1, Number((e.target as HTMLInputElement).value))} /></label>
              </div>
              <p class="help">{tr("Für dieses Modell sind keine Preise eingebaut. Lokale Modelle kosten nichts – dann einfach 0 lassen.", "No built-in prices for this model. Local models are free – just leave 0.")}</p>
            {/if}
          {/if}
          <p class="help">{tr("Anbieter und API-Schlüssel verwaltest du unter „KI“.", "Manage providers and API keys under “AI”.")}</p>
        {/if}

        <h3>{tr("Korrigieren & verbessern", "Correct & improve")}</h3>
        <div class="field">
          <span>{tr("Dienst", "Service")}</span>
          <div class="seg" role="group">
            <button type="button" class:on={s.translate.correctEngine === "deepl"} onclick={() => ((s!.translate.correctEngine = "deepl"), changed())}>DeepL</button>
            <button type="button" class:on={s.translate.correctEngine === "ai"} onclick={() => ((s!.translate.correctEngine = "ai"), changed())}>{tr("KI-Anbieter", "AI provider")}</button>
          </div>
          <small>{tr("DeepL Write braucht einen Pro-Schlüssel. Mit Free-Schlüssel korrigiert DeepL per Doppelübersetzung (z. B. Deutsch → Englisch → Deutsch) – das formuliert oft stärker um. Die KI nutzt denselben Anbieter und dasselbe Modell wie die Übersetzung.", "DeepL Write needs a Pro key. With a Free key, DeepL corrects via a double translation (e.g. German → English → German), which often rephrases more. The AI uses the same provider and model as translation.")}</small>
        </div>
        <label class="field">
          <span>{tr("Stil beim Verbessern (Standard)", "Style when improving (default)")}</span>
          <select bind:value={s.translate.improveStyle} onchange={changed}>
            {#each styleOptions() as [v, l] (v)}<option value={v}>{l}</option>{/each}
          </select>
        </label>
        <label class="check"><input type="checkbox" bind:checked={s.translate.live} onchange={changed} /> {tr("Beim Tippen im Fenster automatisch neu übersetzen / korrigieren (nach 1 Sekunde Pause)", "Re-run automatically while typing in the window (after a 1 second pause)")}</label>

        <h3>{tr("Verbrauch diesen Monat", "Usage this month")}</h3>
        {#if usage}
          <div class="usage">
            <div><b>{tr("KI", "AI")}</b><span>{usage.month.aiCostUsd.toLocaleString(locale(), { minimumFractionDigits: 2, maximumFractionDigits: 4 })} $ · {nf(usage.month.aiTokensIn)} → {nf(usage.month.aiTokensOut)} Tokens</span></div>
            <div><b>DeepL</b><span>{#if usage.deeplUsed != null && usage.deeplLimit}{nf(usage.deeplUsed)} / {nf(usage.deeplLimit)} {tr("Zeichen", "characters")}{:else if usage.deeplError}{usage.deeplError}{:else}{nf(usage.month.deeplChars)} {tr("Zeichen (über UltimatePaste)", "characters (via UltimatePaste)")}{/if}</span></div>
          </div>
        {:else}
          <p class="help">…</p>
        {/if}
      {:else if section === "history"}
        <h2>{tr("Verlauf & Datenschutz", "History & privacy")}</h2>
        <p class="intro">{tr("Was aufgenommen wird und wie lange es bleibt.", "What gets recorded and how long it is kept.")}</p>
        <label class="check"><input type="checkbox" bind:checked={s.paused} onchange={changed} /> {tr("Aufnahme pausiert", "Recording paused")}</label>
        <label class="field">
          <span>{tr("Aufbewahrung", "Keep history for")}</span>
          <select value={s.retentionDays ?? 0} onchange={(e) => ((s!.retentionDays = Number((e.target as HTMLSelectElement).value) || null), changed())}>
            {#each RETENTION as r (r.v)}<option value={r.v}>{tr(r.de, r.en)}</option>{/each}
          </select>
          <small>{tr("Angeheftete Einträge bleiben immer erhalten.", "Pinned entries are always kept.")}</small>
        </label>
        <div class="row">
          <label class="field"><span>{tr("Max. Textgröße (MB)", "Max. text size (MB)")}</span><input type="number" min="1" max="100" bind:value={s.maxTextMb} onchange={changed} /></label>
          <label class="field"><span>{tr("Max. Bildgröße (MB)", "Max. image size (MB)")}</span><input type="number" min="1" max="500" bind:value={s.maxImageMb} onchange={changed} /></label>
        </div>
        <label class="field">
          <span>{tr("Nie aufnehmen von (Programmname, eine Zeile pro Programm)", "Never record from (executable, one per line)")}</span>
          <textarea rows="8" bind:value={blocked} onchange={changed} spellcheck="false"></textarea>
          <small>{tr("Zusätzlich werden Inhalte ignoriert, die Passwort-Manager als vertraulich markieren.", "Content marked as confidential by password managers is always ignored.")}</small>
        </label>
        <button class="danger" onclick={clearAll}>{tr("Verlauf leeren (außer Angeheftetem)", "Clear history (except pinned)")}</button>

        <h3>{tr("Sensible Einträge automatisch löschen", "Remove sensitive entries automatically")}</h3>
        <p class="help">{tr("Erkennt solche Inhalte beim Kopieren und entfernt sie nach der eingestellten Zeit wieder aus dem Verlauf. Angeheftetes bleibt.", "Recognizes such content when copied and removes it from the history after the set time. Pinned entries stay.")}</p>
        <label class="check"><input type="checkbox" bind:checked={s.sensitive.otp} onchange={changed} /> {tr("Einmal-Codes (2FA, 6–8 Ziffern)", "One-time codes (2FA, 6–8 digits)")}</label>
        <label class="check"><input type="checkbox" bind:checked={s.sensitive.cards} onchange={changed} /> {tr("Kreditkartennummern", "Credit card numbers")}</label>
        <label class="check"><input type="checkbox" bind:checked={s.sensitive.keys} onchange={changed} /> {tr("API-Schlüssel und Tokens", "API keys and tokens")}</label>
        <label class="check"><input type="checkbox" bind:checked={s.sensitive.iban} onchange={changed} /> IBAN</label>
        <label class="field">
          <span>{tr("Löschen nach", "Remove after")}</span>
          <select bind:value={s.sensitive.minutes} onchange={changed}>
            {#each [1, 5, 15, 60] as m (m)}<option value={m}>{tr("{0} Minuten", "{0} minutes", m)}</option>{/each}
          </select>
        </label>

        <h3>{tr("Links", "Links")}</h3>
        <label class="check"><input type="checkbox" bind:checked={s.cleanLinks} onchange={changed} /> {tr("Tracking aus kopierten Links automatisch entfernen (utm_…, fbclid, gclid …)", "Remove tracking from copied links automatically (utm_…, fbclid, gclid …)")}</label>
        <p class="help">{tr("Der gesäuberte Link landet in der Zwischenablage, das Original bleibt im Verlauf. Einzeln geht es auch über „Ohne Tracking einfügen“ im Aktionsmenü.", "The cleaned link goes to the clipboard, the original stays in the history. One by one it also works via “Paste without tracking” in the actions menu.")}</p>

              <h3>{tr("Übersetzen & Korrigieren", "Translate & correct")}</h3>
        <p class="help">{tr("Die letzten 20 Vorgänge des Übersetzungsfensters liegen verschlüsselt auf diesem PC.", "The last 20 runs of the translate window are stored encrypted on this PC.")}</p>
        <button onclick={async () => { await api.thistClear(); status = tr("Verlauf gelöscht ✓", "History cleared ✓"); }}>{tr("Übersetzungsverlauf löschen", "Clear translation history")}</button>
      {:else if section === "sequential"}
        <h2>{tr("Nacheinander einfügen", "Paste sequentially")}</h2>
        <p class="intro">{tr("Jeder Druck auf das Tastenkürzel fügt den nächstälteren Eintrag ein – ohne Fenster.", "Each press of the shortcut pastes the next older entry – without opening the window.")}</p>
        <div class="field">
          <span>{tr("Nacheinander einfügen", "Paste sequentially")}</span>
          <HotkeyPicker value={s.sequentialHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.sequentialHotkey = v), changed())} />
          {#if appShortcutHint(s.sequentialHotkey)}<small class="apphint">⚠ {appShortcutHint(s.sequentialHotkey)}</small>{/if}
          {@render conflictNote(1)}
        </div>
        <p class="help">{tr("Das Tastenkürzel stellst du unter „Tastenkürzel“ ein.", "Set the shortcut under “Shortcuts”.")}</p>
        <label class="field">
          <span>{tr("Neustart nach (Sekunden ohne Druck)", "Restart after (seconds idle)")}</span>
          <select bind:value={s.sequentialTimeoutSecs} onchange={changed}>
            <option value={30}>30</option><option value={60}>60</option><option value={180}>180</option>
          </select>
        </label>
        <label class="check"><input type="checkbox" bind:checked={s.sequentialNewline} onchange={changed} /> {tr("Nach jedem Eintrag Zeilenumbruch einfügen", "Add a line break after each entry")}</label>
      {:else if section === "snippets"}
        <h2>{tr("Snippets", "Snippets")}</h2>
        <p class="intro">{tr("Textbausteine legst du im Einfüge-Fenster im Reiter „Snippets“ an (Strg + N). Hier stellst du ein, wie sie sich verhalten.", "You create text snippets in the paste window, tab “Snippets” (Ctrl + N). Here you set how they behave.")}</p>
        <label class="check"><input type="checkbox" bind:checked={s.snippetExpand} onchange={changed} /> {tr("Snippets beim Tippen ersetzen", "Replace snippets while typing")}</label>
        <p class="help">{tr("Tippst du irgendwo das Kürzel eines Snippets mit vorangestelltem Semikolon – z. B. „;mfg“ –, wird es sofort durch den Text ersetzt. Hat das Snippet Eingabefelder, öffnet sich vorher das Formular.", "If you type a snippet's keyword with a leading semicolon anywhere – e.g. “;mfg” –, it is replaced by the text right away. If the snippet has input fields, the form opens first.")}</p>
        <p class="help">{tr("Dafür merkt sich UltimatePaste die letzten rund 40 getippten Zeichen im Arbeitsspeicher – nichts davon wird gespeichert oder verschickt. In Passwortfeldern, gesperrten Programmen und Administrator-Fenstern ist die Funktion aus.", "For this, UltimatePaste keeps the last ~40 typed characters in memory – nothing of it is stored or sent. It is off in password fields, blocked programs and administrator windows.")}</p>
        <h3>{tr("Platzhalter", "Placeholders")}</h3>
        <ul class="facts">
          <li><code>{"{date}"}</code> <code>{"{time}"}</code> <code>{"{datetime}"}</code> <code>{"{date:%d.%m.%Y}"}</code> – {tr("Datum und Uhrzeit", "date and time")}</li>
          <li><code>{"{clipboard}"}</code> – {tr("der neueste Text aus dem Verlauf", "the newest text from the history")}</li>
          <li><code>{"{cursor}"}</code> – {tr("hier steht danach der Cursor", "the cursor ends up here")}</li>
          <li><code>{"{eingabe:Name}"}</code> <code>{"{eingabe:Ort|Stuttgart}"}</code> – {tr("Eingabefeld, optional mit Vorgabe", "input field, optionally with a default")}</li>
          <li><code>{"{auswahl:Anrede|Herr|Frau}"}</code> – {tr("Auswahlliste", "choice list")}</li>
        </ul>
        <p class="help">{tr("Englische Schreibweisen funktionieren ebenso: {input:…} und {select:…}.", "English forms work as well: {input:…} and {select:…}.")}</p>
      {:else if section === "rules"}
        <h2>{tr("Regeln je Programm", "Rules per program")}</h2>
        <p class="intro">{tr("Wie in bestimmte Programme eingefügt wird – z. B. in Outlook immer als reiner Text. Gilt für normales Einfügen und „Nacheinander einfügen“.", "How content is pasted into specific programs – e.g. always plain text in Outlook. Applies to normal pasting and “Paste sequentially”.")}</p>
        {#each s.appRules as r, i (i)}
          <div class="row end">
            <label class="field grow">
              <span>{tr("Programm", "Program")}</span>
              <input type="text" list="recent-apps" bind:value={r.app} onchange={changed} placeholder="outlook.exe" spellcheck="false" />
            </label>
            <label class="field">
              <span>{tr("Einfügen als", "Paste as")}</span>
              <select bind:value={r.mode} onchange={changed}>
                <option value="plain">{tr("Reiner Text", "Plain text")}</option>
                <option value="formatted">{tr("Mit Formatierung", "With formatting")}</option>
                <option value="oneline">{tr("Eine Zeile (Umbrüche entfernen)", "One line (remove line breaks)")}</option>
                <option value="trim">{tr("Ohne Leerzeichen am Anfang/Ende", "Without leading/trailing spaces")}</option>
              </select>
            </label>
            <button class="danger" onclick={() => (s!.appRules.splice(i, 1), changed())}>✕</button>
          </div>
        {:else}
          <p class="help">{tr("Noch keine Regeln.", "No rules yet.")}</p>
        {/each}
        <datalist id="recent-apps">{#each recentApps as a (a)}<option value={a}></option>{/each}</datalist>
        <div><button onclick={() => (s!.appRules.push({ app: recentApps[0] ?? "", mode: "plain" }), changed())}>+ {tr("Regel hinzufügen", "Add rule")}</button></div>
        <p class="help">{tr("Vorschläge stammen aus den Programmen, in die du zuletzt eingefügt hast. „Als … einfügen“ aus dem Aktionsmenü geht immer vor.", "Suggestions come from the programs you pasted into recently. “Paste as …” from the actions menu always wins.")}</p>
      {:else if section === "ocr"}
        <h2>{tr("Texterkennung & Inbox", "OCR & inbox")}</h2>
        <p class="intro">{tr("Text in Bildern erkennen und Einträge in einem Ordner ablegen.", "Recognize text in images and save entries to a folder.")}</p>
        <label class="check"><input type="checkbox" bind:checked={s.ocrEnabled} onchange={changed} /> {tr("Text in Bildern automatisch erkennen (lokal, Windows-Texterkennung)", "Recognize text in images automatically (local, Windows OCR)")}</label>
        <p class="help">{tr("Benötigt ein installiertes Windows-Sprachpaket mit Texterkennung (Einstellungen → Zeit und Sprache).", "Requires a Windows language pack with OCR (Settings → Time & language).")}</p>
        <label class="field">
          <span>{tr("Inbox-Ordner", "Inbox folder")}</span>
          <input type="text" placeholder="C:\Users\…\Obsidian\Inbox" bind:value={s.inboxDir} onchange={changed} spellcheck="false" />
          <small>{tr("Strg+I legt den Eintrag dort ab: Text als Markdown mit Front-Matter, Bilder als PNG, Dateien als Kopie.", "Ctrl+I saves the entry there: text as Markdown with front matter, images as PNG, files as copies.")}</small>
        </label>
        <label class="field"><span>{tr("Tags (kommagetrennt)", "Tags (comma separated)")}</span><input type="text" bind:value={tags} onchange={changed} /></label>
      {:else if section === "ai"}
        <h2>{tr("KI", "AI")}</h2>
        <p class="intro">{tr("KI-Anbieter, API-Schlüssel und eigene KI-Aktionen für das Einfüge-Fenster.", "AI providers, API keys and your own AI actions for the paste window.")}</p>
        <label class="check"><input type="checkbox" bind:checked={s.ai.enabled} onchange={changed} /> {tr("KI-Aktionen anzeigen (Inhalte werden nur bei Auswahl einer Aktion gesendet)", "Show AI actions (content is only sent when you pick an action)")}</label>

        <h3>{tr("Anbieter", "Providers")}</h3>
        {#each s.ai.providers as p, i (p.id)}
          <div class="box">
            <div class="row">
              <label class="field"><span>Name</span><input type="text" bind:value={p.name} onchange={changed} /></label>
              <label class="field">
                <span>{tr("Typ", "Type")}</span>
                <select bind:value={p.kind} onchange={changed}>
                  <option value="anthropic">Anthropic (Claude)</option>
                  <option value="openai">{tr("OpenAI-kompatibel", "OpenAI-compatible")}</option>
                </select>
              </label>
            </div>
            <div class="row">
              <label class="field grow"><span>URL</span><input type="text" bind:value={p.baseUrl} onchange={changed} spellcheck="false" /></label>
              <label class="field"><span>{tr("Modell", "Model")}</span><input type="text" bind:value={p.model} onchange={changed} spellcheck="false" /></label>
            </div>
            <div class="row end">
              <label class="field grow">
                <span>API-Key {hasKey[p.id] ? tr("(gespeichert)", "(stored)") : ""}</span>
                <input type="password" bind:value={keys[p.id]} placeholder={hasKey[p.id] ? "••••••••" : p.kind === "openai" ? tr("optional", "optional") : "sk-ant-…"} />
              </label>
              <button onclick={() => saveKey(p)}>{tr("Key speichern", "Save key")}</button>
              <label class="check"><input type="radio" name="defprov" checked={s.ai.defaultProvider === p.id} onchange={() => ((s!.ai.defaultProvider = p.id), changed())} /> {tr("Standard", "Default")}</label>
              <button class="danger" onclick={() => removeAt(s!.ai.providers, i)}>✕</button>
            </div>
            <KeyHelp service={p.kind === "anthropic" ? "anthropic" : p.baseUrl.includes("localhost:11434") ? "ollama" : "openai"} />
          </div>
        {/each}
        <button onclick={addProvider}>+ {tr("Anbieter hinzufügen", "Add provider")}</button>
        <p class="help">{tr("API-Keys liegen in der Windows-Anmeldeinformationsverwaltung. Für Claude wird ein API-Key von platform.claude.com benötigt – ein Claude-Abo reicht dafür nicht.", "API keys are stored in Windows Credential Manager. Claude needs an API key from platform.claude.com – a Claude subscription does not work for this.")}</p>

        <h3>{tr("Aktionen", "Actions")}</h3>
        {#each s.ai.actions as a, i (a.id)}
          <div class="box">
            <div class="row">
              <label class="field grow"><span>Name</span><input type="text" bind:value={a.name} onchange={changed} /></label>
              <label class="field">
                <span>{tr("Anbieter", "Provider")}</span>
                <select bind:value={a.provider} onchange={changed}>
                  <option value="">{tr("Standard", "Default")}</option>
                  {#each s.ai.providers as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
                </select>
              </label>
              <button onclick={() => moveAction(i, -1)}>↑</button>
              <button onclick={() => moveAction(i, 1)}>↓</button>
              <button class="danger" onclick={() => removeAt<AiAction>(s!.ai.actions, i)}>✕</button>
            </div>
            <label class="field"><span>Prompt</span><textarea rows="2" bind:value={a.prompt} onchange={changed}></textarea></label>
          </div>
        {/each}
        <button onclick={addAction}>+ {tr("Aktion hinzufügen", "Add action")}</button>
      {:else if section === "stats"}
        <Stats />
      {:else if section === "about"}
        <h2>{tr("Info", "About")}</h2>
        <p class="intro">{tr("Version, Projektseite, Datenschutz und Lizenzen.", "Version, project page, privacy and licenses.")}</p>
        {#if info}
          <div class="aboutcard">
            <b>UltimatePaste {info.version}</b>
            <span class="help">{tr("Einträge im Verlauf", "Entries in history")}: {info.entries} · {tr("Lizenz", "License")}: MIT</span>
            <div class="row">
              <button onclick={() => api.openLink(REPO)}>{tr("Projekt auf GitHub", "Project on GitHub")} ↗</button>
              <button onclick={() => api.openLink(REPO + "/issues")}>{tr("Fehler melden / Idee vorschlagen", "Report a bug / suggest an idea")} ↗</button>
            </div>
            <div class="row">
              <button onclick={() => api.openData("data")}>{tr("Datenordner öffnen", "Open data folder")}</button>
              <button onclick={() => api.openData("log")}>{tr("Log-Datei öffnen", "Open log file")}</button>
            </div>
            <span class="help"><code>{info.dataDir}</code></span>
          </div>
        {/if}
        <h3>{tr("Datenschutz", "Privacy")}</h3>
        <ul class="facts">
          <li>{tr("Verlauf, Snippets, Statistik und Übersetzungsverlauf bleiben auf diesem PC – verschlüsselt, gebunden an dein Windows-Konto.", "History, snippets, statistics and translation history stay on this PC – encrypted and bound to your Windows account.")}</li>
          <li>{tr("Inhalte von Passwort-Managern und aus gesperrten Programmen werden nicht aufgenommen.", "Content from password managers and blocked programs is not recorded.")}</li>
          <li>{tr("An DeepL oder den KI-Anbieter geht Text nur, wenn du übersetzt, korrigierst oder eine KI-Aktion startest.", "Text only goes to DeepL or the AI provider when you translate, correct or start an AI action.")}</li>
          <li>{tr("An GIPHY oder KLIPY gehen nur Suchbegriffe im GIF-Reiter – mit einer anonymen Geräte-ID, wenn die Rückmeldung eingeschaltet ist.", "Only search terms in the GIF tab go to GIPHY or KLIPY – with an anonymous device ID if feedback is on.")}</li>
          <li>{tr("API-Schlüssel liegen in der Windows-Anmeldeinformationsverwaltung.", "API keys are stored in Windows Credential Manager.")}</li>
        </ul>
        <h3>{tr("Verwendete Daten und Bibliotheken", "Third-party data and libraries")}</h3>
        <ul class="facts">
          <li>Emojibase (MIT) – {tr("Emoji-Namen und Suchbegriffe", "emoji names and keywords")} <button class="linkbtn" onclick={() => api.openLink("https://emojibase.dev")}>emojibase.dev ↗</button></li>
          <li>flag-icons (MIT) – {tr("Flaggen in den Sprachlisten", "flags in the language lists")} <button class="linkbtn" onclick={() => api.openLink("https://github.com/lipis/flag-icons")}>GitHub ↗</button></li>
          <li>{tr("GIF-Suche über GIPHY und KLIPY mit deinem eigenen Schlüssel", "GIF search via GIPHY and KLIPY with your own key")}</li>
          <li>Tauri, Svelte, Rust – {tr("und viele weitere Open-Source-Bibliotheken", "and many more open-source libraries")}</li>
        </ul>
        <p class="help">{tr("Verlauf, Snippets und Bilder sind verschlüsselt gespeichert (AES-256-GCM, Schlüssel per DPAPI an dein Windows-Konto gebunden).", "History, snippets and images are stored encrypted (AES-256-GCM, key bound to your Windows account via DPAPI).")}</p>
        <p class="help">MIT License</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns: 220px 1fr;
    align-items: start;
    min-height: 100%;
  }
  aside {
    position: sticky;
    top: 0;
    height: 100vh;
    border-right: 1px solid var(--border);
    padding: 16px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  h1 {
    font-size: 16px;
    margin: 0 8px 14px;
  }
  aside button {
    text-align: left;
    border: none;
    background: transparent;
    color: var(--muted);
  }
  aside button.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .saved {
    position: fixed;
    right: 20px;
    bottom: 20px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 13px;
    font-weight: 600;
    box-shadow: var(--shadow);
    animation: pop 0.15s ease-out;
  }
  @keyframes pop {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .saved {
      animation: none;
    }
  }
  .body {
    padding: 18px 28px 40px;
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 20px;
  }
  h3 {
    margin: 12px 0 0;
    font-size: 14px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field > span {
    font-size: 12px;
    color: var(--muted);
  }
  .field small,
  .help {
    font-size: 12px;
    color: var(--faint);
    margin: 0;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .row .field {
    min-width: 140px;
  }
  .grow {
    flex: 1;
  }
  .box {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--panel);
  }
  textarea {
    resize: vertical;
    font: 13px var(--mono);
  }
  button.danger {
    align-self: flex-start;
  }
  code {
    font: 12px var(--mono);
  }
  .seg {
    display: inline-flex;
    align-self: flex-start;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 3px;
    gap: 2px;
    background: var(--panel);
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--muted);
    padding: 4px 14px;
  }
  .seg button.on {
    background: var(--accent);
    color: var(--on-accent);
  }
  .intro {
    margin: -6px 0 6px;
    font-size: 13px;
    color: var(--muted);
  }
  .tones {
    display: flex;
    gap: 4px;
  }
  .tones button {
    font: 20px/1 "Segoe UI Emoji", var(--font);
    padding: 4px 6px;
    border-radius: 8px;
  }
  .tones button.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }
  .arrow {
    align-self: center;
    padding-top: 16px;
    color: var(--muted);
  }
  .usage {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
    background: var(--panel);
  }
  .usage div {
    display: flex;
    gap: 12px;
  }
  .usage b {
    width: 60px;
  }
  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
  .conflict {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    align-items: center;
    margin-top: 4px;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border));
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    font-size: 12.5px;
  }
  .conflict.ok {
    border-color: var(--border);
    background: transparent;
    color: var(--muted);
  }
  .conflict > span {
    flex: 1 1 260px;
  }
  .conflict .confirm {
    flex: 1 1 100%;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 8px;
    align-items: center;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }
  .conflict .confirm > span {
    flex: 1 1 260px;
  }
  .hkerr {
    margin: 4px 0 0;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid var(--danger);
    color: var(--danger);
    font-size: 13px;
  }
  .layouts {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .designs {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .des {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    text-align: left;
    padding: 8px;
    border-radius: 10px;
  }
  .des.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }
  .des small {
    font-size: 11px;
    color: var(--faint);
  }
  .dprev {
    position: relative;
    width: 100%;
    height: 54px;
    margin-bottom: 4px;
    border-radius: var(--radius);
    background: var(--bg-solid);
    border: 1px solid var(--border);
    color: var(--text);
    overflow: hidden;
  }
  .dprev .bar {
    position: absolute;
    left: 10px;
    right: 44px;
    top: 10px;
    height: 12px;
    border-radius: 4px;
    background: var(--panel);
    border: 1px solid var(--border);
  }
  .dprev .dot {
    position: absolute;
    left: 10px;
    top: 30px;
    width: 46px;
    height: 14px;
    border-radius: 999px;
    background: var(--accent);
  }
  .dprev .aa {
    position: absolute;
    right: 10px;
    bottom: 6px;
    font: 600 20px var(--display);
  }
  .lay {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    text-align: left;
    padding: 10px;
    border-radius: 10px;
  }
  .lay.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }
  .lay small {
    font-size: 11.5px;
    color: var(--faint);
  }
  .mini {
    width: 100%;
    height: 64px;
    border-radius: 6px;
    background: var(--kbd);
    position: relative;
    overflow: hidden;
    margin-bottom: 4px;
  }
  .mini i {
    position: absolute;
    border-radius: 3px;
    background: var(--faint);
    opacity: 0.55;
  }
  .mini.fluent i:nth-child(1) { left: 10%; top: 14%; width: 80%; height: 10%; }
  .mini.fluent i:nth-child(2) { left: 10%; top: 32%; width: 34%; height: 56%; }
  .mini.fluent i:nth-child(3) { left: 48%; top: 32%; width: 42%; height: 56%; opacity: 0.3; }
  .mini.fluent i:nth-child(4) { display: none; }
  .mini.board i { width: 18%; height: 34%; top: 14%; }
  .mini.board i:nth-child(1) { left: 8%; background: var(--accent); }
  .mini.board i:nth-child(2) { left: 30%; }
  .mini.board i:nth-child(3) { left: 52%; }
  .mini.board i:nth-child(4) { left: 74%; }
  .mini.drawer i:nth-child(1) { right: 0; top: 0; width: 34%; height: 100%; opacity: 0.4; }
  .mini.drawer i:nth-child(2) { right: 6%; top: 16%; width: 22%; height: 8%; }
  .mini.drawer i:nth-child(3) { right: 6%; top: 34%; width: 22%; height: 8%; background: var(--accent); }
  .mini.drawer i:nth-child(4) { right: 6%; top: 52%; width: 22%; height: 8%; }
  .mini.compact i:nth-child(1) { left: 10%; top: 30%; width: 30%; height: 3%; }
  .mini.compact i:nth-child(2) { left: 20%; top: 40%; width: 36%; height: 46%; background: var(--accent); opacity: 0.45; }
  .mini.compact i:nth-child(3) { left: 60%; top: 40%; width: 22%; height: 30%; opacity: 0.3; }
  .mini.compact i:nth-child(4) { display: none; }

  .grp {
    margin: 12px 10px 2px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--faint);
  }
  .apphint {
    color: var(--warn, #b26a00);
  }
  .aboutcard {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  .facts {
    margin: 0;
    padding-left: 18px;
    font-size: 13px;
    line-height: 1.55;
  }
  .linkbtn {
    font-size: 12px;
    padding: 0 6px;
  }
  .emsg {
    flex: 1 1 100%;
    font-size: 12px;
    color: var(--muted);
  }
</style>
