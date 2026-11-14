<script lang="ts">
  // Settings page part: export / import settings, snippets, AI actions and program rules.
  import { save as saveDialog, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, errText, type TransferInfo } from "./api";
  import { locale, tr } from "./i18n.svelte";

  let { onimported }: { onimported: () => void } = $props();

  let withKeys = $state(false);
  let pw1 = $state("");
  let pw2 = $state("");
  let msg = $state("");
  let err = $state("");
  let busy = $state(false);

  let importPath = $state<string | null>(null);
  let info = $state<TransferInfo | null>(null);
  let replace = $state(false);
  let importPw = $state("");

  const today = () => new Date().toISOString().slice(0, 10);

  async function doExport() {
    err = msg = "";
    if (withKeys && (pw1.length < 6 || pw1 !== pw2)) {
      err = pw1.length < 6 ? tr("Das Passwort braucht mindestens 6 Zeichen.", "The password needs at least 6 characters.") : tr("Die Passwörter stimmen nicht überein.", "The passwords don't match.");
      return;
    }
    const path = await saveDialog({ defaultPath: `UltimatePaste-${today()}.json`, filters: [{ name: "UltimatePaste", extensions: ["json"] }] });
    if (!path) return;
    busy = true;
    try {
      const r = await api.settingsExport(path, withKeys ? pw1 : null);
      msg = tr("Gespeichert: Einstellungen, {0} Snippets, {1} API-Schlüssel.", "Saved: settings, {0} snippets, {1} API keys.", r.snippets, r.keys);
      pw1 = pw2 = "";
    } catch (e) {
      err = errText(e);
    } finally {
      busy = false;
    }
  }

  async function pickImport() {
    err = msg = "";
    const path = await openDialog({ multiple: false, directory: false, filters: [{ name: "UltimatePaste", extensions: ["json"] }] });
    if (!path || Array.isArray(path)) return;
    try {
      info = await api.settingsInspect(path);
      importPath = path;
      importPw = "";
    } catch (e) {
      err = errText(e) === "not-an-export" ? tr("Das ist keine Export-Datei von UltimatePaste.", "This is not an UltimatePaste export file.") : errText(e);
    }
  }

  async function doImport() {
    if (!importPath) return;
    err = msg = "";
    busy = true;
    try {
      const r = await api.settingsImport(importPath, info?.hasKeys && importPw ? importPw : null, replace);
      msg = tr("Importiert: Einstellungen, {0} Snippets, {1} API-Schlüssel.", "Imported: settings, {0} snippets, {1} API keys.", r.report.snippets, r.report.keys);
      if (r.save.hotkeyErrors.length) err = r.save.hotkeyErrors.join(" ");
      importPath = null;
      info = null;
      importPw = "";
      onimported();
    } catch (e) {
      err = errText(e) === "wrong-password" ? tr("Das Passwort stimmt nicht.", "Wrong password.") : errText(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="box">
  <b>{tr("Exportieren", "Export")}</b>
  <p class="help">{tr("Einstellungen, Snippets, KI-Aktionen und Programm-Regeln in eine Datei – z. B. für einen neuen PC. Der Verlauf kommt nicht mit.", "Settings, snippets, AI actions and program rules in one file – e.g. for a new PC. The history is not included.")}</p>
  <label class="check"><input type="checkbox" bind:checked={withKeys} /> {tr("API-Schlüssel mitnehmen (mit Passwort verschlüsselt)", "Include API keys (encrypted with a password)")}</label>
  {#if withKeys}
    <div class="row">
      <input type="password" bind:value={pw1} placeholder={tr("Passwort", "Password")} autocomplete="new-password" />
      <input type="password" bind:value={pw2} placeholder={tr("Passwort wiederholen", "Repeat password")} autocomplete="new-password" />
    </div>
    <p class="help">{tr("Ohne dieses Passwort lassen sich die Schlüssel nicht wiederherstellen.", "The keys can't be restored without this password.")}</p>
  {/if}
  <div><button onclick={doExport} disabled={busy}>{tr("Exportieren …", "Export …")}</button></div>
</div>

<div class="box">
  <b>{tr("Importieren", "Import")}</b>
  {#if !info}
    <div><button onclick={pickImport} disabled={busy}>{tr("Datei wählen …", "Choose file …")}</button></div>
  {:else}
    <p class="help">
      {tr("Export vom {0} (Version {1}) · {2} Snippets", "Export from {0} (version {1}) · {2} snippets", new Date(info.exported).toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" }), info.app, info.snippets)}{info.hasKeys ? " · " + tr("enthält API-Schlüssel", "contains API keys") : ""}
    </p>
    <div class="seg" role="group">
      <button type="button" class:on={!replace} onclick={() => (replace = false)}>{tr("Ergänzen", "Merge")}</button>
      <button type="button" class:on={replace} onclick={() => (replace = true)}>{tr("Ersetzen", "Replace")}</button>
    </div>
    <p class="help">{replace ? tr("Einstellungen werden übernommen, vorhandene Snippets, KI-Aktionen und Regeln durch die aus der Datei ersetzt.", "Settings are taken over; existing snippets, AI actions and rules are replaced by those in the file.") : tr("Einstellungen werden übernommen, Snippets, KI-Aktionen und Regeln aus der Datei kommen zu deinen hinzu.", "Settings are taken over; snippets, AI actions and rules from the file are added to yours.")}</p>
    {#if info.hasKeys}
      <input type="password" bind:value={importPw} placeholder={tr("Passwort für die API-Schlüssel (leer = ohne Schlüssel)", "Password for the API keys (empty = without keys)")} />
    {/if}
    <div class="row">
      <button class="primary" onclick={doImport} disabled={busy}>{tr("Importieren", "Import")}</button>
      <button onclick={() => ((info = null), (importPath = null))} disabled={busy}>{tr("Abbrechen", "Cancel")}</button>
    </div>
  {/if}
</div>
{#if msg}<p class="ok" role="status">✓ {msg}</p>{/if}
{#if err}<p class="bad" role="alert">{err}</p>{/if}

<style>
  .box {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: 10px;
    margin-bottom: 8px;
  }
  .help {
    font-size: 12px;
    color: var(--faint);
    margin: 0;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 13px;
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
  .seg {
    display: inline-flex;
    width: fit-content;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2px;
    gap: 2px;
  }
  .seg button {
    border: 0;
    background: transparent;
    padding: 3px 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--on-accent);
  }
  .ok {
    color: var(--accent);
    font-size: 13px;
    margin: 0;
  }
  .bad {
    color: var(--danger);
    font-size: 13px;
    margin: 0;
  }
</style>
