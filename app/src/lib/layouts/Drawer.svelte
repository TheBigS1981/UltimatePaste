<script lang="ts">
  // Variant C – drawer: slides in from the right screen edge, timeline grouped by day,
  // the selected entry expands in place with its actions.
  import Icon from "../Icon.svelte";
  import { t, tr } from "../i18n.svelte";
  import { appColor, appName, entryLabel, formatsLabel, groupByDay, tabLabel, TABS, timeOf, type ViewActions, type ViewModel, shortcutLabel } from "../view";
  import Preview from "./Preview.svelte";

  let { vm, act, query = $bindable(), searchEl = $bindable(), onsearch }: { vm: ViewModel; act: ViewActions; query: string; searchEl?: HTMLInputElement; onsearch: () => void } = $props();

  const groups = $derived(!vm.query && vm.tab !== "snippets" ? groupByDay(vm.items) : [{ label: "", rows: vm.items.map((it, i) => ({ it, i })) }]);
  const total = $derived(vm.tab === "snippets" ? vm.snippets.length : vm.items.length);
</script>

<div class="frame">
  <aside class="sd" data-tauri-drag-region>
    <div class="head" data-tauri-drag-region="deep">
      <div class="title"><span>UltimatePaste</span><span class="tr"><small>{total} {t("entries")}</small><button class="gear" tabindex="-1" title={t("settings")} aria-label={t("settings")} onclick={() => act.run("settings")}><Icon name="gear" /></button></span></div>
      <div class="search">
        <Icon name="search" />
        <input bind:this={searchEl} bind:value={query} oninput={onsearch} placeholder={vm.tab === "snippets" ? t("snippetPlaceholder") : t("searchPlaceholder")} spellcheck="false" autocomplete="off" />
      </div>
      <div class="filter">
        {#each TABS as tb, ti (tb)}
          <button class:on={vm.tab === tb} tabindex="-1" class:tabbtn={true} onclick={() => act.setTab(tb)}>{tabLabel(tb)}<small class="tabhint">Alt {ti + 1}</small></button>
        {/each}
        {#if vm.settings?.paused}<span class="paused">{t("paused")}</span>{/if}
      </div>
    </div>

    <div class="list">
      {#if vm.tab === "snippets"}
        {#each vm.snippets as s, i (s.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="row" class:on={i === vm.sel} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={() => act.select(i)} ondblclick={() => act.activate(i)}>
            <span class="time">{s.keyword ? `;${s.keyword}` : ""}</span>
            <span class="rail"><i style:background="var(--accent)"></i></span>
            <div class="txt">
              <div class="line">{s.name}</div>
              {#if i === vm.sel}
                <div class="expanded"><Preview entry={null} detail={null} snippet={s} meta={false} maxLines={6} /></div>
                <div class="acts"><button class="pri" tabindex="-1" onclick={() => act.run("paste")}>↵ {t("paste")}</button><button tabindex="-1" onclick={() => act.run("actions")}>{t("more")}</button></div>
              {/if}
            </div>
          </div>
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptySnippets")}</div>
        {/each}
      {:else}
        {#each groups as g (g.label)}
          {#if g.label}<div class="day">{g.label}</div>{/if}
          {#each g.rows as { it, i } (it.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div class="row" class:on={i === vm.sel} class:gone={vm.missing[it.id]} class:marked={vm.marked.includes(it.id)} data-mark={vm.marked.includes(it.id) ? vm.marked.indexOf(it.id) + 1 : null} title={vm.missing[it.id] ? t("fileGone") : undefined} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={(e) => act.select(i, e)} ondblclick={() => act.activate(i)}>
              <span class="time">{timeOf(it.lastUsedAt)}</span>
              <span class="rail"><i style:background={appColor(it.sourceApp)}></i></span>
              <div class="txt">
                {#if i === vm.sel}
                  {#if it.title}<div class="line strong">{it.title}</div>{/if}
                  <div class="expanded">
                    {#if it.kind === "image" && vm.thumbs[it.id]}
                      <img src={vm.thumbs[it.id]} alt="" />
                    {:else}
                      <Preview entry={it} detail={vm.detail} meta={false} maxLines={8} />
                    {/if}
                  </div>
                  <small>{#if it.pinned}<span class="pin">★</span>{/if}{appName(it.sourceApp) || "—"} · {formatsLabel(it.formats)}</small>
                  <div class="acts">
                    <button class="pri" tabindex="-1" onclick={() => act.run("paste")}>↵ {t("paste")}</button>
                    {#if it.formats.includes("text")}<button tabindex="-1" onclick={() => act.run("plain")}>⇧↵ {t("footerAlt")}</button>{/if}
                    <button tabindex="-1" onclick={() => act.run("pin")}>{t("ctrlShort")} P {it.pinned ? t("unpin") : t("pin")}</button>
                    {#if vm.settings?.ai.enabled && it.kind !== "image"}<button tabindex="-1" onclick={() => act.run("ai")}>{t("aiPrefix")}</button>{/if}
                    <button tabindex="-1" onclick={() => act.run("actions")}>{t("more")}</button>
                  </div>
                {:else}
                  <div class="line" class:strong={!!it.title}>{entryLabel(it)}</div>
                  <small>{#if it.pinned}<span class="pin">★</span>{/if}{appName(it.sourceApp) || "—"}</small>
                {/if}
              </div>
            </div>
          {/each}
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptyHistory")}</div>
        {/each}
      {/if}
    </div>

    <footer>
      {#if vm.toast}<span class="toast">{vm.toast}</span>{:else}{#if vm.tab === "snippets"}<span>{t("ctrlShort")} E {t("edit")}</span><span>{t("ctrlShort")} N {t("newSnippet")}</span><span>{t("ctrlShort")} {tr("Entf", "Del")} {t("remove")}</span>{/if}<span>{shortcutLabel(vm.settings?.actionsKey ?? "")} {t("footerActions")}</span><span>Esc {t("close")}</span>{/if}
    </footer>
  </aside>
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
  .tr {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .frame {
    height: 100%;
    padding-left: 32px;
    display: flex;
  }
  .sd {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border-left: 1px solid var(--border);
    border-radius: 14px 0 0 14px;
    box-shadow: -18px 0 40px -18px rgba(0, 0, 0, 0.45);
    font: 14px/1.45 var(--font);
    animation: slide 0.16s ease-out;
  }
  @keyframes slide {
    from {
      transform: translateX(24px);
      opacity: 0.4;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sd {
      animation: none;
    }
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 16px 10px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
  }
  .title small {
    font: 12px var(--mono);
    color: var(--muted);
    font-weight: 400;
  }
  .search {
    display: flex;
    gap: 8px;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 10px;
    color: var(--muted);
    background: var(--panel);
  }
  .search input {
    flex: 1;
    border: 0;
    background: transparent;
    outline: none;
    padding: 0;
  }
  .filter {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .filter button {
    border: 0;
    background: transparent;
    padding: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .filter button.on {
    color: var(--accent);
    font-weight: 600;
  }
  .paused {
    margin-left: auto;
    font-size: 12px;
    color: var(--danger);
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 0 12px;
  }
  .day {
    font: 500 11px var(--mono);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
    padding: 12px 16px 4px;
  }
  .row {
    display: grid;
    grid-template-columns: 46px 12px minmax(0, 1fr);
    gap: 8px;
    padding: 6px 16px 6px 10px;
  }
  .row:hover {
    background: var(--kbd);
  }
  .row.on {
    background: var(--sel);
  }
  .time {
    font: 12px var(--mono);
    color: var(--muted);
    text-align: right;
    padding-top: 2px;
    white-space: nowrap;
    overflow: hidden;
  }
  .rail {
    position: relative;
  }
  .rail::before {
    content: "";
    position: absolute;
    left: 5px;
    top: -6px;
    bottom: -6px;
    width: 2px;
    background: var(--border);
  }
  .rail i {
    position: absolute;
    left: 1px;
    top: 6px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid var(--bg);
  }
  .txt {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .strong {
    font-weight: 600;
  }
  small {
    font-size: 12px;
    color: var(--muted);
  }
  .pin {
    color: var(--accent);
    margin-right: 4px;
  }
  .expanded {
    max-height: 220px;
    overflow: hidden;
    margin: 2px 0 4px;
  }
  .expanded img {
    max-width: 100%;
    max-height: 200px;
    border-radius: 6px;
    border: 1px solid var(--border);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }
  .acts button {
    font-size: 12px;
    padding: 2px 9px;
    border-radius: 6px;
    background: var(--bg);
  }
  .acts .pri {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .empty {
    padding: 32px 16px;
    text-align: center;
    color: var(--faint);
  }
  footer {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 16px;
    border-top: 1px solid var(--border);
    font: 12px var(--mono);
    color: var(--muted);
  }
  .toast {
    color: var(--text);
    font-family: var(--font);
  }
  .gone {
    opacity: 0.55;
  }
  .gone :global(.line),
  .gone :global(.body) {
    text-decoration: line-through;
  }
</style>
