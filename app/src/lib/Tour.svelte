<script lang="ts">
  // Welcome tour on first start (Q27): six short steps that set things up right away.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, errText, SERVICE_KEYS, type Settings } from "./api";
  import { i18n, tr, LANGS } from "./i18n.svelte";
  import { applyLook, applyTheme } from "./theme";
  import HotkeyPicker from "./HotkeyPicker.svelte";
  import KeyHelp from "./KeyHelp.svelte";
  import LangSelect from "./LangSelect.svelte";

  let s = $state<Settings | null>(null);
  let step = $state(0);
  let hkError = $state("");
  let note = $state("");
  let keys = $state<Record<string, string>>({});
  let hasKey = $state<Record<string, boolean>>({});
  let winVOff = $state(false);
  let busy = $state(false);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  const STEPS = $derived([
    tr("Willkommen", "Welcome"),
    tr("Kopieren & Einfügen", "Copy & paste"),
    tr("Übersetzen & Korrigieren", "Translate & correct"),
    tr("Emoji & GIF", "Emoji & GIF"),
    tr("Datenschutz", "Privacy"),
    tr("Fertig", "Done"),
  ]);

  onMount(async () => {
    s = await api.settings();
    i18n.lang = s.language;
    applyTheme(s.theme);
    applyLook(s.layout, s.design);
    for (const k of Object.values(SERVICE_KEYS)) hasKey[k] = await api.aiHasKey(k);
    const c = await api.hotkeyConflicts().catch(() => []);
    winVOff = c.some((x) => x.kind === "windows-v" && x.resolved);
  });

  function changed() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(save, 300);
  }

  async function save() {
    if (!s) return;
    i18n.lang = s.language;
    try {
      const r = await api.saveSettings($state.snapshot(s) as Settings);
      s.hotkey = r.hotkey;
      s.pickerHotkey = r.pickerHotkey;
      s.translateHotkey = r.translateHotkey;
      s.correctHotkey = r.correctHotkey;
      hkError = r.hotkeyErrors.join(" ");
    } catch (e) {
      hkError = errText(e);
    }
  }

  async function saveKey(k: string) {
    if (!keys[k]) return;
    await api.aiSetKey(k, keys[k]);
    hasKey[k] = true;
    keys[k] = "";
    note = tr("API-Schlüssel gespeichert ✓", "API key saved ✓");
  }

  async function switchOffWinV() {
    busy = true;
    try {
      const c = await api.setWindowsV(true);
      winVOff = c.some((x) => x.kind === "windows-v" && x.resolved);
    } catch (e) {
      hkError = errText(e);
    } finally {
      busy = false;
    }
  }

  async function finish() {
    clearTimeout(saveTimer);
    await save();
    await api.finishTour();
    getCurrentWindow().close();
  }

  const label = (hk: string) =>
    hk
      ? hk
          .split("+")
          .map((p) => ({ super: "Win", ctrl: tr("Strg", "Ctrl"), control: tr("Strg", "Ctrl"), shift: tr("Umschalt", "Shift"), alt: "Alt", period: ".", comma: "," })[p.trim().toLowerCase()] ?? p.trim())
          .join(" + ")
      : tr("aus", "off");

  function onKey(e: KeyboardEvent) {
    const typing = ["INPUT", "TEXTAREA", "SELECT"].includes((e.target as HTMLElement)?.tagName);
    if (e.key === "Escape") (e.preventDefault(), finish());
    else if (!typing && e.key === "ArrowRight" && step < STEPS.length - 1) step++;
    else if (!typing && e.key === "ArrowLeft" && step > 0) step--;
  }
</script>

<svelte:window onkeydown={onKey} />

{#if s}
  <div class="tour">
    <ol class="dots" aria-label={tr("Schritte", "Steps")}>
      {#each STEPS as name, i (i)}
        <li class:on={i === step} class:done={i < step}><button onclick={() => (step = i)} title={name}><span>{i + 1}</span></button></li>
      {/each}
    </ol>

    <main>
      {#if step === 0}
        <h1>{tr("Willkommen bei UltimatePaste", "Welcome to UltimatePaste")}</h1>
        <p class="lead">{tr("Drei Werkzeuge für jeden Tag – alle per Tastenkürzel, in jedem Programm:", "Three everyday tools – all by shortcut, in any program:")}</p>
        <div class="cards">
          <div class="card"><b>📋 {tr("Kopieren & Einfügen", "Copy & paste")}</b><span>{tr("Alles, was du kopierst, durchsuchbar wieder einfügen – auch Bilder, Dateien und Snippets.", "Everything you copy, searchable and ready to paste again – images, files and snippets too.")}</span></div>
          <div class="card"><b>🌐 {tr("Übersetzen & Korrigieren", "Translate & correct")}</b><span>{tr("Markierten Text übersetzen oder verbessern – mit DeepL oder KI.", "Translate or improve selected text – with DeepL or AI.")}</span></div>
          <div class="card"><b>😊 {tr("Emoji & GIF", "Emoji & GIF")}</b><span>{tr("Emojis, GIFs und Sonderzeichen suchen und einfügen.", "Search and insert emoji, GIFs and symbols.")}</span></div>
        </div>
        <div class="field">
          <span>{tr("Sprache", "Language")}</span>
          <LangSelect label={tr("Sprache", "Language")} value={s.language} options={LANGS.map((l) => ({ code: l.code, label: l.name }))} onchange={(v) => ((s!.language = v as Settings["language"]), changed())} />
        </div>
        <p class="help">{tr("In ein paar Schritten ist alles eingerichtet. Jeder Schritt lässt sich überspringen – alles findest du später auch in den Einstellungen.", "A few steps and you're set. Every step can be skipped – everything is in the settings later, too.")}</p>
      {:else if step === 1}
        <h1>📋 {tr("Kopieren & Einfügen", "Copy & paste")}</h1>
        <p class="lead">{tr("Mit diesem Kürzel öffnest du deinen Verlauf. Viele nehmen Win + V – das ersetzt dann die Zwischenablage von Windows.", "This shortcut opens your history. Many use Win + V – it then replaces the Windows clipboard.")}</p>
        <div class="field">
          <span>{tr("Einfüge-Fenster öffnen", "Open paste window")}</span>
          <HotkeyPicker value={s.hotkey} onchange={(v) => ((hkError = ""), (s!.hotkey = v), changed())} />
        </div>
        <div class="row">
          {#if s.hotkey !== "Super+V"}<button onclick={() => ((s!.hotkey = "Super+V"), changed())}>{tr("Win + V verwenden", "Use Win + V")}</button>{/if}
          {#if s.hotkey === "Super+V" && !winVOff}<button onclick={switchOffWinV} disabled={busy}>{busy ? "…" : tr("Windows' eigenes Win + V abschalten", "Switch off Windows' own Win + V")}</button>{/if}
        </div>
        {#if s.hotkey === "Super+V"}
          <p class="help">{winVOff ? "✓ " + tr("Windows' Win + V ist abgeschaltet (wirkt ab der nächsten Anmeldung).", "Windows' Win + V is switched off (takes effect from the next sign-in).") : tr("Empfohlen: Dann kann Windows nie dazwischenfunken. Wirkt ab der nächsten Windows-Anmeldung; rückgängig unter Einstellungen → Einfüge-Fenster.", "Recommended: then Windows can never get in the way. Takes effect from the next Windows sign-in; undo in Settings → Paste window.")}</p>
        {/if}
        <ul class="tips">
          <li><kbd>↵</kbd> {tr("einfügen", "paste")} · <kbd>⇧ ↵</kbd> {tr("als reiner Text", "as plain text")} · <kbd>{tr("Strg", "Ctrl")} 1–9</kbd> {tr("direkt den n-ten Eintrag", "the n-th entry directly")}</li>
          <li><kbd>{tr("Strg", "Ctrl")} ↵</kbd> {tr("Aktionen: Als … einfügen, KI, Schnellaktionen", "actions: paste as …, AI, quick actions")}</li>
          <li><kbd>Alt 1–5</kbd> {tr("Reiter: Verlauf, Angeheftet, Text, Bilder, Snippets", "tabs: history, pinned, text, images, snippets")}</li>
        </ul>
      {:else if step === 2}
        <h1>🌐 {tr("Übersetzen & Korrigieren", "Translate & correct")}</h1>
        <p class="lead">{tr("Text markieren, Kürzel drücken – fertig. Ohne Markierung wird die Zwischenablage genommen.", "Select text, press the shortcut – done. Without a selection, the clipboard is used.")}</p>
        <div class="grid2">
          <div class="field"><span>{tr("Übersetzen", "Translate")}</span><HotkeyPicker value={s.translateHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.translateHotkey = v), changed())} /></div>
          <div class="field"><span>{tr("Korrigieren & verbessern", "Correct & improve")}</span><HotkeyPicker value={s.correctHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.correctHotkey = v), changed())} /></div>
        </div>
        <div class="field">
          <span>DeepL API-Key {hasKey[SERVICE_KEYS.deepl] ? tr("(gespeichert)", "(stored)") : tr("(optional, kostenlos)", "(optional, free)")}</span>
          <div class="row">
            <input type="password" bind:value={keys[SERVICE_KEYS.deepl]} placeholder={hasKey[SERVICE_KEYS.deepl] ? "••••••••" : "…:fx"} />
            <button onclick={() => saveKey(SERVICE_KEYS.deepl)}>{tr("Speichern", "Save")}</button>
          </div>
          <KeyHelp service="deepl" />
        </div>
        <p class="help">{tr("Statt DeepL geht auch eine KI (Claude oder ein OpenAI-kompatibler Dienst) – einzurichten unter Einstellungen → KI.", "Instead of DeepL you can use an AI (Claude or an OpenAI-compatible service) – set up under Settings → AI.")}</p>
      {:else if step === 3}
        <h1>😊 {tr("Emoji & GIF", "Emoji & GIF")}</h1>
        <p class="lead">{tr("Öffnet das Auswahlfenster direkt am Text-Cursor.", "Opens the picker right at the text caret.")}</p>
        <div class="field">
          <span>{tr("Emoji, GIF & Sonderzeichen", "Emoji, GIF & symbols")}</span>
          <HotkeyPicker value={s.pickerHotkey} allowEmpty onchange={(v) => ((hkError = ""), (s!.pickerHotkey = v), changed())} />
        </div>
        {#if s.pickerHotkey !== "Super+Period"}<div class="row"><button onclick={() => ((s!.pickerHotkey = "Super+Period"), changed())}>{tr("Win + . verwenden (ersetzt das Emoji-Feld von Windows)", "Use Win + . (replaces the Windows emoji panel)")}</button></div>
        {/if}
        <div class="grid2">
          {#each [["giphy", "GIPHY"], ["klipy", "KLIPY"]] as [id, name] (id)}
            {@const k = SERVICE_KEYS[id as "giphy" | "klipy"]}
            <div class="field">
              <span>{name} API-Key {hasKey[k] ? tr("(gespeichert)", "(stored)") : tr("(optional, kostenlos)", "(optional, free)")}</span>
              <div class="row">
                <input type="password" bind:value={keys[k]} placeholder={hasKey[k] ? "••••••••" : ""} />
                <button onclick={() => saveKey(k)}>{tr("Speichern", "Save")}</button>
              </div>
              <KeyHelp service={id as "giphy" | "klipy"} />
            </div>
          {/each}
        </div>
        <p class="help">{tr("Ohne Schlüssel funktionieren Emojis und Sonderzeichen ganz normal, nur der GIF-Reiter bleibt leer.", "Without a key, emoji and symbols work as usual; only the GIF tab stays empty.")}</p>
      {:else if step === 4}
        <h1>🔒 {tr("Datenschutz", "Privacy")}</h1>
        <ul class="tips">
          <li>{tr("Dein Verlauf bleibt auf diesem PC – verschlüsselt und an dein Windows-Konto gebunden.", "Your history stays on this PC – encrypted and bound to your Windows account.")}</li>
          <li>{tr("Passwort-Manager und gesperrte Programme werden nicht aufgenommen.", "Password managers and blocked programs are not recorded.")}</li>
          <li>{tr("Text geht nur an DeepL oder die KI, wenn du selbst übersetzt, korrigierst oder eine KI-Aktion startest.", "Text only goes to DeepL or the AI when you translate, correct or start an AI action yourself.")}</li>
        </ul>
        <label class="check"><input type="checkbox" bind:checked={s.sensitive.otp} onchange={() => ((s!.sensitive.cards = s!.sensitive.keys = s!.sensitive.otp), changed())} /> {tr("Einmal-Codes, Kartennummern und API-Schlüssel nach {0} Minuten aus dem Verlauf löschen", "Remove one-time codes, card numbers and API keys from the history after {0} minutes", s.sensitive.minutes)}</label>
        <label class="check"><input type="checkbox" bind:checked={s.cleanLinks} onchange={changed} /> {tr("Tracking aus kopierten Links entfernen", "Remove tracking from copied links")}</label>
        <label class="check"><input type="checkbox" bind:checked={s.snippetExpand} onchange={changed} /> {tr("Snippets beim Tippen ersetzen (z. B. „;mfg“)", "Replace snippets while typing (e.g. “;mfg”)")}</label>
        <p class="help">{tr("Für die Snippet-Ersetzung merkt sich UltimatePaste die letzten rund 40 getippten Zeichen im Arbeitsspeicher – nichts davon wird gespeichert oder verschickt.", "For snippet replacement, UltimatePaste keeps the last ~40 typed characters in memory – nothing of it is stored or sent.")}</p>
      {:else}
        <h1>🎉 {tr("Alles bereit!", "All set!")}</h1>
        <p class="lead">{tr("Deine Tastenkürzel:", "Your shortcuts:")}</p>
        <table class="keys">
          <tbody>
            <tr><td>{tr("Einfüge-Fenster", "Paste window")}</td><td><kbd>{label(s.hotkey)}</kbd></td></tr>
            <tr><td>{tr("Übersetzen", "Translate")}</td><td><kbd>{label(s.translateHotkey)}</kbd></td></tr>
            <tr><td>{tr("Korrigieren & verbessern", "Correct & improve")}</td><td><kbd>{label(s.correctHotkey)}</kbd></td></tr>
            <tr><td>{tr("Emoji, GIF & Sonderzeichen", "Emoji, GIF & symbols")}</td><td><kbd>{label(s.pickerHotkey)}</kbd></td></tr>
            <tr><td>{tr("Nacheinander einfügen", "Paste sequentially")}</td><td><kbd>{label(s.sequentialHotkey)}</kbd></td></tr>
          </tbody>
        </table>
        <p class="help">{tr("UltimatePaste läuft im Infobereich der Taskleiste. Rechtsklick auf das Symbol öffnet Einstellungen, Übersetzen und mehr. Die Tour startest du unter Einstellungen → Allgemein erneut.", "UltimatePaste runs in the notification area of the taskbar. Right-click the icon for settings, translate and more. Restart the tour under Settings → General.")}</p>
      {/if}
      {#if hkError}<p class="err" role="alert">{hkError}</p>{/if}
      {#if note}<p class="ok" role="status">{note}</p>{/if}
    </main>

    <footer>
      <button class="ghost" onclick={finish}>{step < STEPS.length - 1 ? tr("Tour überspringen", "Skip tour") : ""}</button>
      <span class="sp"></span>
      {#if step > 0}<button onclick={() => (step--, (note = ""))}>{tr("Zurück", "Back")}</button>{/if}
      {#if step < STEPS.length - 1}
        <button class="primary" onclick={() => (step++, (note = ""))}>{tr("Weiter", "Next")} →</button>
      {:else}
        <button class="primary" onclick={finish}>{tr("Los geht's", "Let's go")}</button>
      {/if}
    </footer>
  </div>
{/if}

<style>
  .tour {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-solid);
    color: var(--text);
    font: 14px/1.5 var(--font);
  }
  .dots {
    display: flex;
    justify-content: center;
    gap: 6px;
    list-style: none;
    margin: 0;
    padding: 16px 0 4px;
  }
  .dots button {
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: 50%;
    font-size: 12px;
    color: var(--muted);
  }
  .dots li.done button {
    border-color: var(--accent);
    color: var(--accent);
  }
  .dots li.on button {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  main {
    flex: 1;
    overflow-y: auto;
    padding: 8px 36px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h1 {
    font-size: 22px;
    margin: 6px 0 0;
  }
  .lead {
    margin: 0;
    color: var(--muted);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    font-size: 13px;
  }
  .card span {
    color: var(--muted);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .row input {
    flex: 1;
    min-width: 160px;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    font-size: 13.5px;
  }
  .check input {
    margin-top: 4px;
  }
  .help {
    margin: 0;
    font-size: 12.5px;
    color: var(--faint);
  }
  .tips {
    margin: 0;
    padding-left: 18px;
    font-size: 13px;
    line-height: 1.7;
  }
  kbd {
    font: 600 11.5px var(--mono);
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--kbd);
  }
  .keys {
    border-collapse: collapse;
    font-size: 13.5px;
  }
  .keys td {
    padding: 5px 16px 5px 0;
    border-bottom: 1px solid var(--border);
  }
  .err {
    color: var(--danger);
    font-size: 13px;
    margin: 0;
  }
  .ok {
    color: var(--accent);
    font-size: 13px;
    margin: 0;
  }
  footer {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 12px 20px;
    border-top: 1px solid var(--border);
  }
  .sp {
    flex: 1;
  }
  .ghost {
    border: 0;
    background: transparent;
    color: var(--muted);
  }
</style>
