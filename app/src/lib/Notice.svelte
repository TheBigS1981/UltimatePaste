<script lang="ts">
  // Shown once after start when a shortcut is shared with Windows or PowerToys.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, errText, type Conflict } from "./api";
  import { i18n, tr } from "./i18n.svelte";
  import { applyLook, applyTheme } from "./theme";

  let list = $state<Conflict[]>([]);
  let busy = $state(false);
  let error = $state("");
  const winV = $derived(list.find((c) => c.kind === "windows-v"));
  const pt = $derived(list.filter((c) => c.kind === "powertoys"));

  const keyLabel = (hk: string) =>
    hk
      .split("+")
      .map((p) => ({ super: "Win", ctrl: tr("Strg", "Ctrl"), control: tr("Strg", "Ctrl"), shift: tr("Umschalt", "Shift"), alt: "Alt", period: ".", comma: "," })[p.trim().toLowerCase()] ?? p.trim())
      .join(" + ");

  onMount(async () => {
    const s = await api.settings();
    i18n.lang = s.language;
    applyTheme(s.theme);
    applyLook(s.layout, s.design);
    list = await api.noticeConflicts();
    if (!list.length) close();
  });

  function close() {
    getCurrentWindow().close();
  }

  async function switchOff() {
    busy = true;
    error = "";
    try {
      await api.setWindowsV(true);
      list = list.filter((c) => c.kind !== "windows-v");
      if (!list.length) close();
    } catch (e) {
      error = errText(e);
    } finally {
      busy = false;
    }
  }

  async function never() {
    await api.dismissConflicts(list.map((c) => c.key));
    close();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="nt">
  <h1>{tr("Tastenkürzel doppelt belegt", "Shortcut used twice")}</h1>
  {#if winV}
    <p>
      {tr("{0} öffnet auch die Zwischenablage von Windows. UltimatePaste fängt das Kürzel ab, aber Windows kann gelegentlich dazwischenfunken.", "{0} also opens the Windows clipboard. UltimatePaste intercepts it, but Windows can occasionally get in the way.", keyLabel(winV.hotkey))}
      <b>{tr("Am zuverlässigsten ist es, das Windows-Kürzel abzuschalten.", "Switching off the Windows shortcut is the most reliable fix.")}</b>
    </p>
    <p class="small">{tr("Das wirkt ab der nächsten Windows-Anmeldung. Rückgängig jederzeit unter Einstellungen → Einfüge-Fenster.", "This takes effect from the next Windows sign-in. Undo any time in Settings → Paste window.")}</p>
  {/if}
  {#each pt as c (c.key)}
    <p>{tr("{0} nutzt auch PowerToys ({1}). Bitte dort ändern oder das Modul abschalten.", "{0} is also used by PowerToys ({1}). Please change it there or switch the module off.", keyLabel(c.hotkey), c.module ?? "")}</p>
  {/each}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  <div class="btns">
    <button onclick={never} disabled={busy}>{tr("Nicht mehr fragen", "Don't ask again")}</button>
    <span class="sp"></span>
    <button onclick={close} disabled={busy}>{tr("Später", "Later")}</button>
    {#if winV}<button class="primary" onclick={switchOff} disabled={busy}>{busy ? "…" : tr("Jetzt abschalten", "Switch off now")}</button>{/if}
  </div>
</div>

<style>
  .nt {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px 18px 14px;
    background: var(--bg-solid);
    color: var(--text);
    font: 13.5px/1.45 var(--font);
  }
  h1 {
    margin: 0 0 2px;
    font-size: 16px;
  }
  p {
    margin: 0;
  }
  .small {
    color: var(--muted);
    font-size: 12.5px;
  }
  .err {
    color: var(--danger);
  }
  .btns {
    margin-top: auto;
    display: flex;
    gap: 8px;
  }
  .sp {
    flex: 1;
  }
</style>
