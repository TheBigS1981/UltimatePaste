<script lang="ts">
  // "A new version is available": what's new, update now or later.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, errText, type UpdateInfo } from "./api";
  import { i18n, locale, tr } from "./i18n.svelte";
  import { applyLook, applyTheme } from "./theme";

  let info = $state<UpdateInfo | null>(null);
  let busy = $state(false);
  let err = $state("");

  onMount(async () => {
    const s = await api.settings();
    i18n.lang = s.language;
    applyTheme(s.theme);
    applyLook(s.layout, s.design);
    info = await api.updateInfo();
  });

  async function install() {
    busy = true;
    err = "";
    try {
      await api.updateInstall();
    } catch (e) {
      const m = errText(e);
      err =
        m === "checksum-mismatch"
          ? tr("Die heruntergeladene Datei ist beschädigt (Prüfsumme stimmt nicht). Bitte später erneut versuchen.", "The downloaded file is damaged (checksum mismatch). Please try again later.")
          : m === "no-asset"
            ? tr("Für diese Installation gibt es keine passende Datei im Release.", "The release has no file for this kind of installation.")
            : m;
      busy = false;
    }
  }

  const close = () => getCurrentWindow().close();
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !busy && close()} />

<div class="up">
  {#if info}
    <h1>⬆ {tr("UltimatePaste {0} ist da", "UltimatePaste {0} is available", info.latest)}</h1>
    <p class="sub">
      {tr("Du hast {0}.", "You have {0}.", info.current)}
      {#if info.published}{tr("Veröffentlicht am {0}.", "Released on {0}.", new Date(info.published).toLocaleDateString(locale(), { dateStyle: "long" }))}{/if}
    </p>
    <h2>{tr("Was ist neu?", "What's new?")}</h2>
    <pre class="notes">{info.notes || "–"}</pre>
    <p class="help">
      {info.installed
        ? tr("Das Update wird heruntergeladen, geprüft und installiert; danach startet UltimatePaste neu.", "The update is downloaded, verified and installed; UltimatePaste restarts afterwards.")
        : tr("Die portable Datei wird heruntergeladen, geprüft und ersetzt; danach startet UltimatePaste neu.", "The portable file is downloaded, verified and replaced; UltimatePaste restarts afterwards.")}
      {tr("Windows kann dabei eine Sicherheitswarnung zeigen, weil die App noch nicht signiert ist.", "Windows may show a security warning because the app isn't signed yet.")}
    </p>
    {#if err}<p class="err" role="alert">{err}</p>{/if}
    <div class="btns">
      <button onclick={() => api.openLink(info!.page)} disabled={busy}>{tr("Auf GitHub ansehen", "View on GitHub")} ↗</button>
      <span class="sp"></span>
      <button onclick={close} disabled={busy}>{tr("Später", "Later")}</button>
      <button class="primary" onclick={install} disabled={busy}>{busy ? tr("Wird geladen …", "Downloading …") : tr("Jetzt aktualisieren", "Update now")}</button>
    </div>
  {:else}
    <p class="sub">{tr("Kein Update gefunden.", "No update found.")}</p>
  {/if}
</div>

<style>
  .up {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 18px 20px 14px;
    background: var(--bg-solid);
    color: var(--text);
    font: 13.5px/1.45 var(--font);
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  h2 {
    margin: 6px 0 0;
    font-size: 13px;
    color: var(--muted);
  }
  .sub {
    margin: 0;
    color: var(--muted);
  }
  .notes {
    flex: 1;
    min-height: 60px;
    overflow: auto;
    margin: 0;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    white-space: pre-wrap;
    font: 12.5px/1.5 var(--font);
  }
  .help {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .err {
    margin: 0;
    color: var(--danger);
  }
  .btns {
    display: flex;
    gap: 8px;
  }
  .sp {
    flex: 1;
  }
</style>
