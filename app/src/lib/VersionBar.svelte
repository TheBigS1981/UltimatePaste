<script lang="ts">
  // Version, build date and update check at the bottom of the settings sidebar.
  import { onMount } from "svelte";
  import { api, errText, type UpdateInfo } from "./api";
  import { locale, tr } from "./i18n.svelte";

  let ver = $state<{ version: string; buildDate: string; portable: boolean } | null>(null);
  let busy = $state(false);
  let result = $state<UpdateInfo | null>(null);
  let err = $state("");

  onMount(async () => {
    ver = await api.versionInfo();
    result = await api.updateInfo().catch(() => null);
  });

  async function check() {
    busy = true;
    err = "";
    try {
      result = await api.updateCheck();
    } catch (e) {
      const m = errText(e);
      err = m === "no-release" ? tr("Noch keine Veröffentlichung auf GitHub.", "No release on GitHub yet.") : tr("Prüfung fehlgeschlagen.", "Check failed.");
    } finally {
      busy = false;
    }
  }

  const date = (d: string) => (d ? new Date(d).toLocaleDateString(locale(), { dateStyle: "medium" }) : "");
</script>

{#if ver}
  <div class="vbar">
    <button class="ver" title={tr("Nach Updates suchen", "Check for updates")} onclick={check} disabled={busy}>
      <b>v{ver.version}</b>
      <span>{date(ver.buildDate)} · {ver.portable ? tr("portabel", "portable") : tr("installiert", "installed")}</span>
    </button>
    {#if busy}
      <span class="st">{tr("Suche …", "Checking …")}</span>
    {:else if result?.newer}
      <button class="new" onclick={() => api.openUpdate()}>⬆ {tr("Update auf {0}", "Update to {0}", result.latest)}</button>
    {:else if result}
      <span class="st ok">✓ {tr("Aktuell", "Up to date")}</span>
    {:else if err}
      <span class="st">{err}</span>
    {:else}
      <button class="chk" onclick={check}>{tr("Nach Updates suchen", "Check for updates")}</button>
    {/if}
  </div>
{/if}

<style>
  .vbar {
    margin-top: auto;
    padding: 10px 6px 0;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .ver {
    all: unset;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .ver:hover {
    background: var(--panel);
  }
  .ver b {
    color: var(--text);
    font-size: 12.5px;
  }
  .st {
    padding: 0 6px;
  }
  .ok {
    color: var(--accent);
  }
  .new,
  .chk {
    font-size: 12px;
    padding: 4px 8px;
    text-align: left;
  }
  .new {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
