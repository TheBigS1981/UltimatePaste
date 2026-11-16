<script lang="ts">
  // Variant A – Windows 11 style: Mica, underline tabs, day groups, command bar.
  import Icon from "../Icon.svelte";
  import { t, tr } from "../i18n.svelte";
  import { appName, entryLabel, groupByDay, tabLabel, TABS, timeOf, type ViewActions, type ViewModel, shortcutLabel } from "../view";
  import Avatar from "./Avatar.svelte";
  import Preview from "./Preview.svelte";

  let { vm, act, query = $bindable(), searchEl = $bindable(), onsearch }: { vm: ViewModel; act: ViewActions; query: string; searchEl?: HTMLInputElement; onsearch: () => void } = $props();

  const current = $derived(vm.tab === "snippets" ? null : (vm.items[vm.sel] ?? null));
  const snippet = $derived(vm.tab === "snippets" ? (vm.snippets[vm.sel] ?? null) : null);
  const groups = $derived(!vm.query && vm.tab === "all" ? groupByDay(vm.items) : [{ label: "", rows: vm.items.map((it, i) => ({ it, i })) }]);
</script>

<div class="fl">
  <div class="search" data-tauri-drag-region="deep">
    <Icon name="search" />
    <input bind:this={searchEl} bind:value={query} oninput={onsearch} placeholder={vm.tab === "snippets" ? t("snippetPlaceholder") : t("searchPlaceholder")} spellcheck="false" autocomplete="off" />
    {#if vm.settings?.paused}<span class="paused">{t("paused")}</span>{/if}
      <button class="gear" tabindex="-1" title={t("settings")} aria-label={t("settings")} onclick={() => act.run("settings")}><Icon name="gear" /></button>
  </div>
  <nav data-tauri-drag-region="deep">
    {#each TABS as tb, ti (tb)}
      <button class:on={vm.tab === tb} tabindex="-1" class:tabbtn={true} onclick={() => act.setTab(tb)}>{tabLabel(tb)}<small class="tabhint">Alt {ti + 1}</small></button>
    {/each}
  </nav>
  <main>
    <div class="list">
      {#if vm.tab === "snippets"}
        {#each vm.snippets as s, i (s.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="row" class:on={i === vm.sel} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={() => act.select(i)} ondblclick={() => act.activate(i)}>
            <span class="snip"><Icon name="snippet" /></span>
            <div class="txt"><div class="line">{s.name}</div><small>{s.keyword ? `;${s.keyword}` : s.body.slice(0, 60)}</small></div>
            {#if i < 9}<span class="key">{t("ctrlShort")} {i + 1}</span>{/if}
          </div>
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptySnippets")}</div>
        {/each}
      {:else}
        {#each groups as g (g.label)}
          {#if g.label}<div class="group">{g.label}</div>{/if}
          {#each g.rows as { it, i } (it.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div class="row" class:on={i === vm.sel} class:gone={vm.missing[it.id]} class:marked={vm.marked.includes(it.id)} data-mark={vm.marked.includes(it.id) ? vm.marked.indexOf(it.id) + 1 : null} title={vm.missing[it.id] ? t("fileGone") : undefined} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={(e) => act.select(i, e)} ondblclick={() => act.activate(i)}>
              <Avatar entry={it} thumb={vm.thumbs[it.id]} />
              <div class="txt">
                <div class="line" class:titled={!!it.title}>{entryLabel(it)}</div>
                <small>{#if it.pinned}<span class="pin">★</span>{/if}{appName(it.sourceApp) || "—"} · {timeOf(it.lastUsedAt)}</small>
              </div>
              {#if i < 9}<span class="key">{t("ctrlShort")} {i + 1}</span>{/if}
            </div>
          {/each}
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptyHistory")}</div>
        {/each}
      {/if}
    </div>
    <section class="preview">
      <div class="card"><Preview entry={current} detail={vm.detail} {snippet} quick={vm.quick} onquick={act.quick} /></div>
    </section>
  </main>
  <footer>
    <button class="pri" tabindex="-1" onclick={() => act.run("paste")}>{t("paste")} <kbd>↵</kbd></button>
    {#if vm.tab !== "snippets"}
      <button tabindex="-1" onclick={() => act.run("plain")}>{t("footerAlt")} <kbd>⇧↵</kbd></button>
      <button tabindex="-1" onclick={() => act.run("pin")}>{current?.pinned ? t("unpin") : t("pin")} <kbd>{t("ctrlShort")} P</kbd></button>
      {#if vm.settings?.ai.enabled}<button tabindex="-1" onclick={() => act.run("ai")}>{t("aiPrefix")} …</button>{/if}
    {:else}
      <button tabindex="-1" onclick={() => act.run("edit")}>{t("edit")} <kbd>{t("ctrlShort")} E</kbd></button>
      <button tabindex="-1" onclick={() => act.run("new")}>{t("newSnippet")} <kbd>{t("ctrlShort")} N</kbd></button>
      <button tabindex="-1" onclick={() => act.run("del")}>{t("remove")} <kbd>{t("ctrlShort")} {tr("Entf", "Del")}</kbd></button>
    {/if}
    <span class="sp"></span>
    {#if vm.toast}<span class="toast">{vm.toast}</span>{:else}<button class="ghost" tabindex="-1" onclick={() => act.run("actions")}><kbd>{shortcutLabel(vm.settings?.actionsKey ?? "")}</kbd> {t("allActions")}</button>{/if}
  </footer>
</div>

<style>
  .gear {
    flex: none;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
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
  .fl {
    height: 100%;
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    background: var(--bg);
    color: var(--text);
    font: 14px/1.4 var(--font);
  }
  .search {
    margin: 12px 12px 8px;
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-bottom: 2px solid var(--accent);
    border-radius: 6px;
    padding: 6px 12px;
    color: var(--muted);
  }
  .search input {
    flex: 1;
    border: 0;
    background: transparent;
    font-size: 15px;
    padding: 2px 0;
    outline: none;
  }
  .paused {
    font-size: 12px;
    color: var(--danger);
  }
  nav {
    display: flex;
    gap: 20px;
    padding: 0 18px;
    border-bottom: 1px solid var(--border);
  }
  nav button {
    border: 0;
    background: transparent;
    padding: 6px 0 8px;
    border-radius: 0;
    color: var(--muted);
    position: relative;
  }
  nav button.on {
    color: var(--text);
    font-weight: 600;
  }
  nav button.on::after {
    content: "";
    position: absolute;
    left: 25%;
    right: 25%;
    bottom: -1px;
    height: 3px;
    border-radius: 3px;
    background: var(--accent);
  }
  main {
    display: grid;
    grid-template-columns: minmax(280px, 40%) 1fr;
    min-height: 0;
  }
  .list {
    overflow-y: auto;
    padding: 4px 6px;
    border-right: 1px solid var(--border);
  }
  .group {
    font-size: 12px;
    color: var(--muted);
    padding: 10px 10px 4px;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 6px 10px;
    border-radius: 4px;
    position: relative;
  }
  .row:hover {
    background: var(--sel);
  }
  .row.on {
    background: var(--sel);
  }
  .row.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 3px;
    background: var(--accent);
  }
  .snip {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .line.titled {
    font-weight: 600;
  }
  small {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
  }
  .pin {
    color: var(--accent);
    margin-right: 4px;
  }
  .key {
    font-size: 11px;
    color: var(--faint);
    white-space: nowrap;
  }
  .empty {
    padding: 28px 12px;
    text-align: center;
    color: var(--faint);
  }
  .preview {
    padding: 12px 16px;
    min-width: 0;
    min-height: 0;
    display: flex;
  }
  .card {
    flex: 1;
    min-width: 0;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 14px;
  }
  footer {
    display: flex;
    gap: 6px;
    align-items: center;
    padding: 8px 12px;
    border-top: 1px solid var(--border);
    background: var(--panel);
  }
  footer button {
    padding: 4px 12px;
    border-radius: 4px;
    font-size: 13px;
  }
  footer .pri {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  footer .pri kbd {
    background: transparent;
    color: inherit;
    opacity: 0.8;
  }
  footer .ghost {
    border-color: transparent;
    background: transparent;
    color: var(--muted);
  }
  .sp {
    flex: 1;
  }
  .toast {
    font-size: 12.5px;
    background: var(--accent-soft);
    border-radius: 4px;
    padding: 3px 10px;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gone {
    opacity: 0.55;
  }
  .gone :global(.line),
  .gone :global(.body) {
    text-decoration: line-through;
  }
</style>
