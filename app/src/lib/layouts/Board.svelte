<script lang="ts">
  // Variant B – pinboard: every entry is a card that shows its content directly.
  import Icon from "../Icon.svelte";
  import { ago, t, tr } from "../i18n.svelte";
  import { appColor, appName, colorValue, entryLabel, looksLikeCode, tabLabel, TABS, type ViewActions, type ViewModel, shortcutLabel } from "../view";

  let { vm, act, query = $bindable(), searchEl = $bindable(), onsearch }: { vm: ViewModel; act: ViewActions; query: string; searchEl?: HTMLInputElement; onsearch: () => void } = $props();

  function host(url: string): { host: string; path: string } {
    try {
      const u = new URL(url.startsWith("www.") ? `https://${url}` : url);
      return { host: u.hostname.replace(/^www\./, ""), path: (u.pathname + u.search).replace(/^\/$/, "") };
    } catch {
      return { host: url, path: "" };
    }
  }
</script>

<div class="frame">
  <div class="pw">
    <div class="top" data-tauri-drag-region>
      <div class="search">
        <Icon name="search" />
        <input bind:this={searchEl} bind:value={query} oninput={onsearch} placeholder={vm.tab === "snippets" ? t("snippetPlaceholder") : t("searchPlaceholder")} spellcheck="false" autocomplete="off" />
      </div>
      {#each TABS as tb, ti (tb)}
        <button class="chip tabbtn" class:on={vm.tab === tb} tabindex="-1" onclick={() => act.setTab(tb)}>{tabLabel(tb)}<small class="tabhint">Alt {ti + 1}</small></button>
      {/each}
      <button class="gear" tabindex="-1" title={t("settings")} aria-label={t("settings")} onclick={() => act.run("settings")}><Icon name="gear" /></button>
    </div>

    <div class="grid">
      {#if vm.tab === "snippets"}
        {#each vm.snippets as s, i (s.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="card" class:on={i === vm.sel} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={() => act.select(i)} ondblclick={() => act.activate(i)}>
            <div class="src"><span class="dot snip"><Icon name="snippet" /></span>{s.keyword ? `;${s.keyword}` : "Snippet"}</div>
            <div class="name">{s.name}</div>
            <div class="body">{s.body}</div>
            {#if i < 9}<span class="num">{i + 1}</span>{/if}
          </div>
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptySnippets")}</div>
        {/each}
      {:else}
        {#each vm.items as it, i (it.id)}
          {@const sw = colorValue(it)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="card" class:on={i === vm.sel} class:gone={vm.missing[it.id]} class:marked={vm.marked.includes(it.id)} data-mark={vm.marked.includes(it.id) ? vm.marked.indexOf(it.id) + 1 : null} title={vm.missing[it.id] ? t("fileGone") : undefined} role="option" tabindex="-1" aria-selected={i === vm.sel} onclick={(e) => act.select(i, e)} ondblclick={() => act.activate(i)}>
            <div class="src">
              <span class="dot" style:background={appColor(it.sourceApp)}></span>
              {#if it.pinned}<span class="pin">★</span>{/if}
              <span class="who">{appName(it.sourceApp) || "—"} · {ago(it.lastUsedAt)}</span>
            </div>
            {#if it.title}<div class="name">{it.title}</div>{/if}
            {#if it.kind === "image"}
              {#if vm.thumbs[it.id]}<img src={vm.thumbs[it.id]} alt="" />{:else}<div class="ph"><Icon name="image" /></div>{/if}
              <div class="cap">{it.imageW} × {it.imageH}</div>
            {:else if sw}
              <div class="swatch" style:background={sw}></div>
              <div class="cap mono">{sw}</div>
            {:else if it.kind === "link"}
              {@const h = host(it.preview)}
              <div class="link"><b>{h.host}</b><span>{h.path}</span></div>
            {:else if it.kind === "files"}
              <div class="body files">{it.preview}</div>
              <div class="cap">{it.fileCount} {it.fileCount === 1 ? t("file") : t("files")}</div>
            {:else}
              <div class="body" class:mono={looksLikeCode(it.preview)}>{it.title ? it.preview : entryLabel(it)}</div>
            {/if}
            {#if i < 9}<span class="num">{i + 1}</span>{/if}
          </div>
        {:else}
          <div class="empty">{vm.query ? t("empty") : t("emptyHistory")}</div>
        {/each}
      {/if}
    </div>

    <footer>
      <span><b>↵</b> {t("footerPaste")}</span>
      {#if vm.tab === "snippets"}
        <span><b>{t("ctrlShort")} E</b> {t("edit")}</span>
        <span><b>{t("ctrlShort")} N</b> {t("newSnippet")}</span>
        <span><b>{t("ctrlShort")} {tr("Entf", "Del")}</b> {t("remove")}</span>
      {:else}
        <span><b>⇧↵</b> {t("footerAlt")}</span>
      {/if}
      <span><b>← → ↑ ↓</b> {t("move")}</span>
      <span><b>Tab</b> / <b>Alt 1–5</b> {t("footerTabs")}</span>
      <span><b>{shortcutLabel(vm.settings?.actionsKey ?? "")}</b> {t("footerActions")}</span>
      {#if vm.toast}<span class="toast">{vm.toast}</span>{/if}
    </footer>
  </div>
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
  .frame {
    height: 100%;
    padding: 14px;
  }
  .pw {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 18px;
    box-shadow: 0 18px 40px -16px rgba(0, 0, 0, 0.45);
    font: 14px/1.45 var(--font);
  }
  .top {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .search {
    flex: 1;
    display: flex;
    gap: 8px;
    align-items: center;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 8px 12px;
    color: var(--muted);
  }
  .search input {
    flex: 1;
    border: 0;
    background: transparent;
    outline: none;
    font-size: 15px;
    padding: 0;
  }
  .chip {
    border-radius: 999px;
    padding: 6px 13px;
    font-size: 13px;
    color: var(--muted);
    white-space: nowrap;
  }
  .chip.on {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    grid-auto-rows: 168px;
    gap: 12px;
    padding: 3px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    padding: 12px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 14px;
    overflow: hidden;
  }
  .card.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .src {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
    padding-right: 18px;
    min-width: 0;
  }
  .who {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    flex: none;
    width: 14px;
    height: 14px;
    border-radius: 4px;
  }
  .dot.snip {
    width: auto;
    height: auto;
    color: var(--accent);
    display: grid;
  }
  .pin {
    color: var(--accent);
  }
  .num {
    position: absolute;
    top: 10px;
    right: 11px;
    font: 600 11px var(--font);
    color: var(--faint);
  }
  .name {
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .body {
    flex: 1;
    min-height: 0;
    font-size: 13px;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
    word-break: break-word;
  }
  .body.mono,
  .mono {
    font: 12px/1.5 var(--mono);
  }
  .body.files {
    -webkit-line-clamp: 4;
    line-clamp: 4;
  }
  img,
  .ph {
    flex: 1;
    min-height: 0;
    width: 100%;
    object-fit: cover;
    border-radius: 8px;
    border: 1px solid var(--border);
  }
  .ph {
    display: grid;
    place-items: center;
    color: var(--faint);
    background: var(--kbd);
  }
  .swatch {
    flex: 1;
    border-radius: 8px;
    border: 1px solid var(--border);
  }
  .cap {
    font-size: 12px;
    color: var(--muted);
  }
  .link {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
    min-width: 0;
  }
  .link span {
    color: var(--muted);
    word-break: break-all;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }
  .empty {
    grid-column: 1 / -1;
    padding: 40px;
    text-align: center;
    color: var(--faint);
  }
  footer {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    align-items: center;
    font-size: 12px;
    color: var(--muted);
  }
  footer b {
    color: var(--text);
    font-weight: 600;
  }
  .toast {
    margin-left: auto;
    background: var(--accent-soft);
    color: var(--text);
    border-radius: 999px;
    padding: 2px 10px;
  }
  .gone {
    opacity: 0.55;
  }
  .gone :global(.line),
  .gone :global(.body) {
    text-decoration: line-through;
  }
</style>
