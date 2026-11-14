<script lang="ts">
  // Content preview of the selected entry/snippet, shared by the layouts.
  import type { EntryDetail, EntrySummary, Snippet } from "../api";
  import type { Quick } from "../quick";
  import { fmtBytes, fmtDate, t } from "../i18n.svelte";
  import { appName, colorValue, formatsLabel, looksLikeCode } from "../view";

  let {
    entry,
    detail,
    snippet = null,
    meta = true,
    maxLines = 0,
    quick = [],
    onquick,
  }: { entry: EntrySummary | null; detail: EntryDetail | null; snippet?: Snippet | null; meta?: boolean; maxLines?: number; quick?: Quick[]; onquick?: (q: Quick) => void } = $props();

  const ready = $derived(!!entry && !!detail && detail.summary.id === entry.id);
  const swatch = $derived(entry ? colorValue(entry) : null);
  const code = $derived(!!detail?.text && looksLikeCode(detail.text));
</script>

{#if snippet}
  <div class="pv">
    <div class="title">{snippet.name}{#if snippet.keyword}<span class="kw">;{snippet.keyword}</span>{/if}</div>
    <pre class="text" style:-webkit-line-clamp={maxLines || null} class:clamp={maxLines > 0}>{snippet.body}</pre>
    {#if meta}<p class="help">{t("placeholdersHelp")}</p>{/if}
  </div>
{:else if ready && entry && detail}
  <div class="pv">
    <div class="content">
      {#if detail.image}
        <img src={detail.image} alt="" />
        {#if detail.ocrText && meta}
          <div class="label">{t("recognizedText")}</div>
          <p class="ocr">{detail.ocrText}</p>
        {/if}
      {:else if detail.files.length}
        <ul class="files">
          {#each detail.files as f (f)}{@const gone = detail.missingFiles?.includes(f)}<li class:gone><span>{f.split(/[\\/]/).pop()}{#if gone} <em>· {t("fileGone")}</em>{/if}</span><small>{f}</small></li>{/each}
        </ul>
      {:else if swatch}
        <div class="swatch" style:background={swatch}></div>
        <pre class="text mono">{swatch}</pre>
      {:else if detail.text != null}
        <pre class="text" class:mono={code} class:link={entry.kind === "link"} class:clamp={maxLines > 0} style:-webkit-line-clamp={maxLines || null}>{detail.text}</pre>
        {#if detail.textTruncated}<p class="help">{t("truncated")}</p>{/if}
      {/if}
    </div>
    {#if meta && quick.length && onquick}
      <div class="quick" role="group">
        {#each quick.slice(0, 8) as q (q.id)}<button tabindex="-1" onclick={() => onquick(q)}>{q.kind === "open" ? "↗ " : ""}{q.label}</button>{/each}
      </div>
    {/if}
    {#if meta}
      <dl>
        {#if entry.title}<dt>{t("title")}</dt><dd>{entry.title}</dd>{/if}
        {#if entry.sourceApp}<dt>{t("source")}</dt><dd>{appName(entry.sourceApp)}</dd>{/if}
        <dt>{t("copied")}</dt><dd>{fmtDate(entry.createdAt)}</dd>
        {#if entry.imageW}<dt>{t("dimensions")}</dt><dd>{entry.imageW} × {entry.imageH}</dd>{/if}
        <dt>{t("formats")}</dt><dd>{formatsLabel(entry.formats)}</dd>
        <dt>{t("size")}</dt><dd>{fmtBytes(entry.byteSize)}</dd>
        {#if detail.text != null && !detail.files.length}
          <dt>{t("contentLabel")}</dt><dd>{t("statsLine", { words: (detail.text.trim() ? detail.text.trim().split(/\s+/).length : 0).toLocaleString(), chars: [...detail.text].length.toLocaleString() })}{detail.textTruncated ? " +" : ""}</dd>
        {/if}
      </dl>
    {/if}
  </div>
{/if}

<style>
  .pv {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    height: 100%;
    user-select: text;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .title {
    font: 600 15px var(--display);
  }
  .kw {
    margin-left: 8px;
    font: 12px var(--mono);
    color: var(--accent);
  }
  .text {
    margin: 0;
    font: 13.5px/1.55 var(--font);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .text.mono {
    font: 12.5px/1.55 var(--mono);
  }
  .text.link {
    color: var(--accent);
  }
  .text.clamp {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  img {
    display: block;
    max-width: 100%;
    max-height: 240px;
    border-radius: calc(var(--radius) - 2px);
    border: 1px solid var(--border);
    background: repeating-conic-gradient(var(--kbd) 0 25%, transparent 0 50%) 0 0 / 14px 14px;
  }
  .label {
    margin: 12px 0 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .ocr {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .files li {
    display: flex;
    flex-direction: column;
  }
  .files li.gone span {
    text-decoration: line-through;
    color: var(--muted);
  }
  .files li.gone em {
    font-style: normal;
    text-decoration: none;
    display: inline-block;
    color: var(--danger);
    font-size: 12px;
  }
  .files small {
    color: var(--faint);
    font: 11.5px var(--mono);
    word-break: break-all;
  }
  .swatch {
    height: 90px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    margin-bottom: 8px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 3px 16px;
    margin: 0;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    font-size: 12px;
  }
  dt {
    color: var(--faint);
  }
  dd {
    margin: 0;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .help {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .quick {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 4px 0 10px;
  }
  .quick button {
    font-size: 11.5px;
    padding: 2px 9px;
    border-radius: 999px;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
