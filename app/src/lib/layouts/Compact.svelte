<script lang="ts">
  // Variant D – compact popup at the text caret with a floating peek preview.
  import Icon from "../Icon.svelte";
  import { t, tr } from "../i18n.svelte";
  import { appName, entryLabel, contentLabel, formatsLabel, timeOf, TABS, tabLabel, type ViewActions, type ViewModel, shortcutLabel } from "../view";
  import Preview from "./Preview.svelte";

  let { vm, act, query = $bindable(), searchEl = $bindable(), onsearch }: { vm: ViewModel; act: ViewActions; query: string; searchEl?: HTMLInputElement; onsearch: () => void } = $props();

  const current = $derived(vm.tab === "snippets" ? null : (vm.items[vm.sel] ?? null));
  const snippet = $derived(vm.tab === "snippets" ? (vm.snippets[vm.sel] ?? null) : null);
  const short = (tb: string) => (tb === "pinned" ? t("pinsShort") : tabLabel(tb as never));
</script>

<div class="scene">
  <div class="kp" data-tauri-drag-region>
    <div class="q" data-tauri-drag-region="deep">
      <b>›</b>
      <input bind:this={searchEl} bind:value={query} oninput={onsearch} placeholder={t("searchShort")} spellcheck="false" autocomplete="off" />
      {#if vm.settings?.paused}<span class="paused">{t("paused")}</span>{/if}
      <button class="gear" tabindex="-1" title={t("settings")} aria-label={t("settings")} onclick={() => act.run("settings")}><Icon name="gear" /></button>
    </div>
    <div class="tabs">
      {#each TABS as tb, ti (tb)}
        <button class:on={vm.tab === tb} tabindex="-1" class:tabbtn={true} onclick={() => act.setTab(tb)}>{short(tb)}<small class="tabhint">Alt {ti + 1}</small></button>
      {/each}
    </div>
    <div class="list">
      {#if vm.tab === "snippets"}
        {#each vm.snippets as s, i (s.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="row" class:on={i === vm.sel} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={() => act.select(i)} ondblclick={() => act.activate(i)}>
            <span class="n">{i < 9 ? i + 1 : ""}</span>
            <div class="line">{s.name}</div>
            <small>{s.keyword ? `;${s.keyword}` : ""}</small>
          </div>
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptySnippets")}</div>
        {/each}
      {:else}
        {#each vm.items as it, i (it.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="row" class:on={i === vm.sel} class:gone={vm.missing[it.id]} class:marked={vm.marked.includes(it.id)} data-mark={vm.marked.includes(it.id) ? vm.marked.indexOf(it.id) + 1 : null} title={vm.missing[it.id] ? t("fileGone") : undefined} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={(e) => act.select(i, e)} ondblclick={() => act.activate(i)}>
            <span class="n">{#if i < 9}{i + 1}{:else if it.kind === "image"}<Icon name="image" />{/if}</span>
            <div class="line">{#if it.pinned}<span class="pin">★ </span>{/if}{#if it.title}<b>{it.title}</b><span class="rest"> · {contentLabel(it)}</span>{:else}{entryLabel(it)}{/if}</div>
            <small>{appName(it.sourceApp)}</small>
          </div>
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptyHistory")}</div>
        {/each}
      {/if}
    </div>
    <div class="foot">
      {#if vm.toast}<span class="toast">{vm.toast}</span>{:else}<span><kbd>↵</kbd> {t("footerPaste")}</span>{#if vm.tab === "snippets"}<span><kbd>{t("ctrlShort")} E</kbd> {t("edit")}</span><span><kbd>{t("ctrlShort")} {tr("Entf", "Del")}</kbd> {t("remove")}</span>{:else}<span><kbd>⇧↵</kbd> {t("footerAlt")}</span>{/if}<span><kbd>{shortcutLabel(vm.settings?.actionsKey ?? "")}</kbd> {t("more")}</span>{/if}
    </div>
  </div>

  {#if snippet || (current && vm.detail && vm.detail.summary.id === current.id)}
    <div class="peek">
      <div class="pbody"><Preview entry={current} detail={vm.detail} {snippet} meta={false} maxLines={12} onimage={() => act.run("view-image")} /></div>
      {#if current}<small>{appName(current.sourceApp) || "—"} · {timeOf(current.lastUsedAt)} · {formatsLabel(current.formats)}</small>{/if}
    </div>
  {/if}
</div>

<style>
  .gear {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
  }
  .gear:hover {
    background: var(--kbd);
    color: var(--text);
  }
  .scene {
    height: 100%;
    padding: 16px;
    display: flex;
    gap: 10px;
    align-items: flex-start;
    color: var(--text);
    font: 13.5px/1.4 var(--font);
  }
  .kp {
    width: 400px;
    flex: none;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 16px 36px -12px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }
  .q {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 9px 12px;
    border-bottom: 1px solid var(--border);
    font: 13px var(--mono);
  }
  .q b {
    color: var(--accent);
  }
  .q input {
    flex: 1;
    border: 0;
    background: transparent;
    outline: none;
    padding: 0;
    font: 13px var(--mono);
  }
  .paused {
    font: 11px var(--font);
    color: var(--danger);
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 6px 8px 2px;
  }
  .tabs button {
    border: 0;
    background: transparent;
    padding: 2px 8px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .tabs button.on {
    background: var(--sel);
    color: var(--accent);
    font-weight: 600;
  }
  .list {
    overflow-y: auto;
    padding: 4px 6px;
    max-height: 290px;
  }
  .row {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 5px 8px;
    border-radius: 7px;
  }
  .row:hover {
    background: var(--kbd);
  }
  .row.on {
    background: var(--sel);
  }
  .n {
    font: 600 11px var(--mono);
    color: var(--faint);
    text-align: center;
    display: grid;
    place-items: center;
  }
  .row.on .n {
    color: var(--accent);
  }
  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pin {
    color: var(--accent);
  }
  small {
    font-size: 11px;
    color: var(--faint);
    white-space: nowrap;
  }
  .empty {
    padding: 20px 10px;
    text-align: center;
    color: var(--faint);
  }
  .foot {
    display: flex;
    gap: 12px;
    padding: 7px 12px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--muted);
  }
  .foot kbd {
    font: 600 10.5px var(--mono);
    color: var(--text);
  }
  .toast {
    color: var(--text);
  }
  .peek {
    width: 234px;
    flex: none;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 14px 30px -12px rgba(0, 0, 0, 0.4);
    font-size: 12.5px;
  }
  .pbody {
    max-height: 300px;
    overflow: hidden;
  }
  .gone {
    opacity: 0.55;
  }
  .gone :global(.line),
  .gone :global(.body) {
    text-decoration: line-through;
  }
  .line .rest {
    color: var(--muted);
  }
</style>
