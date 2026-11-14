<script lang="ts">
  // Settings page "Statistics": counters only, all local.
  import { onMount } from "svelte";
  import { api, type Stats } from "./api";
  import { locale, tr } from "./i18n.svelte";

  let st = $state<Stats | null>(null);
  let confirmReset = $state(false);

  onMount(async () => {
    st = await api.statsGet();
  });

  const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const nf = (n: number) => n.toLocaleString(locale());
  const usd = (n: number) => `${n.toLocaleString(locale(), { minimumFractionDigits: 2, maximumFractionDigits: 2 })} $`;

  type Kind = "paste" | "sequential" | "translate" | "correct" | "emoji" | "symbol" | "gif";
  const KINDS = $derived<[Kind, string][]>([
    ["paste", tr("Eingefügt", "Pasted")],
    ["sequential", tr("Nacheinander eingefügt", "Pasted sequentially")],
    ["translate", tr("Übersetzt", "Translated")],
    ["correct", tr("Korrigiert", "Corrected")],
    ["emoji", tr("Emojis", "Emoji")],
    ["symbol", tr("Sonderzeichen", "Symbols")],
    ["gif", tr("GIFs", "GIFs")],
  ]);

  /** Sums per kind for days on or after `from` (ISO date). */
  function sum(from: string): Record<Kind, number> {
    const r: Record<Kind, number> = { paste: 0, sequential: 0, translate: 0, correct: 0, emoji: 0, symbol: 0, gif: 0 };
    if (!st) return r;
    for (const [d, v] of Object.entries(st.days)) if (d >= from) for (const k of Object.keys(r) as Kind[]) r[k] += v[k] ?? 0;
    return r;
  }
  const periods = $derived.by(() => {
    void st;
    const now = new Date();
    const monday = new Date(now);
    monday.setDate(now.getDate() - ((now.getDay() + 6) % 7));
    return [
      [tr("Heute", "Today"), sum(iso(now))],
      [tr("Diese Woche", "This week"), sum(iso(monday))],
      [tr("Dieser Monat", "This month"), sum(iso(new Date(now.getFullYear(), now.getMonth(), 1)))],
      [tr("Insgesamt", "Total"), sum("")],
    ] as [string, Record<Kind, number>][];
  });

  /** Last 30 days, all actions together. */
  const days = $derived.by(() => {
    const out: { date: Date; n: number }[] = [];
    for (let i = 29; i >= 0; i--) {
      const d = new Date();
      d.setDate(d.getDate() - i);
      const v = st?.days[iso(d)];
      out.push({ date: d, n: v ? v.paste + v.sequential + v.translate + v.correct + v.emoji + v.symbol + v.gif : 0 });
    }
    return out;
  });
  const maxDay = $derived(Math.max(1, ...days.map((d) => d.n)));

  const top = (m: Record<string, number> | undefined, n = 10) =>
    Object.entries(m ?? {})
      .sort((a, b) => b[1] - a[1])
      .slice(0, n);
  const months = $derived(
    Object.entries(st?.aiCost ?? {})
      .sort((a, b) => b[0].localeCompare(a[0]))
      .slice(0, 12),
  );
  const monthName = (ym: string) => new Date(Number(ym.slice(0, 4)), Number(ym.slice(5, 7)) - 1, 1).toLocaleDateString(locale(), { month: "long", year: "numeric" });

  async function reset() {
    await api.statsReset();
    st = await api.statsGet();
    confirmReset = false;
  }
</script>

<h2>{tr("Statistik", "Statistics")}</h2>
<p class="intro">{tr("Wie oft du UltimatePaste nutzt. Gezählt wird nur, nie der Inhalt – alles bleibt auf diesem PC.", "How often you use UltimatePaste. Only counts are kept, never the content – everything stays on this PC.")}</p>

{#if st}
  {#if st.since}<p class="help">{tr("Erfasst seit {0}", "Recorded since {0}", new Date(st.since).toLocaleDateString(locale(), { dateStyle: "long" }))}</p>{/if}

  <div class="table" role="table">
    <div class="tr head" role="row">
      <span role="columnheader"></span>
      {#each periods as [label] (label)}<span role="columnheader">{label}</span>{/each}
    </div>
    {#each KINDS as [k, label] (k)}
      <div class="tr" role="row">
        <span role="rowheader">{label}</span>
        {#each periods as [p, v] (p)}<span role="cell" class:zero={!v[k]}>{nf(v[k])}</span>{/each}
      </div>
    {/each}
  </div>

  <h3>{tr("Letzte 30 Tage", "Last 30 days")}</h3>
  <div class="chart" role="img" aria-label={tr("Nutzung der letzten 30 Tage", "Usage over the last 30 days")}>
    {#each days as d (d.date.getTime())}
      <div class="col" title={`${d.date.toLocaleDateString(locale(), { weekday: "short", day: "numeric", month: "short" })}: ${nf(d.n)}`}>
        <div class="bar" style:height={`${(d.n / maxDay) * 100}%`}></div>
      </div>
    {/each}
  </div>
  <div class="axis"><span>{days[0].date.toLocaleDateString(locale(), { day: "numeric", month: "short" })}</span><span>{tr("Heute", "Today")}</span></div>

  <div class="tops">
    <section>
      <h3>{tr("Top-Emojis", "Top emoji")}</h3>
      {#if top(st.emoji).length}
        <ol class="chips">{#each top(st.emoji) as [e, n] (e)}<li title={nf(n)}><span class="big">{e}</span><small>{nf(n)}</small></li>{/each}</ol>
      {:else}<p class="help">{tr("Noch keine.", "None yet.")}</p>{/if}
    </section>
    <section>
      <h3>{tr("Top-Sonderzeichen", "Top symbols")}</h3>
      {#if top(st.symbols).length}
        <ol class="chips">{#each top(st.symbols) as [e, n] (e)}<li title={nf(n)}><span class="big">{e}</span><small>{nf(n)}</small></li>{/each}</ol>
      {:else}<p class="help">{tr("Noch keine.", "None yet.")}</p>{/if}
    </section>
  </div>

  <div class="tops">
    <section>
      <h3>{tr("Eingefügt in", "Pasted into")}</h3>
      {#if top(st.apps).length}
        <ol class="list">{#each top(st.apps) as [a, n] (a)}<li><span>{a.replace(/\.exe$/, "")}</span><b>{nf(n)}</b></li>{/each}</ol>
      {:else}<p class="help">{tr("Noch keine.", "None yet.")}</p>{/if}
    </section>
    <section>
      <h3>{tr("Übersetzt (Zeichen)", "Translated (characters)")}</h3>
      {#if top(st.pairs).length}
        <ol class="list">{#each top(st.pairs) as [p, n] (p)}<li><span>{p}</span><b>{nf(n)}</b></li>{/each}</ol>
      {:else}<p class="help">{tr("Noch keine.", "None yet.")}</p>{/if}
    </section>
  </div>

  <h3>{tr("KI-Kosten pro Monat", "AI costs per month")}</h3>
  {#if months.length}
    <ol class="list narrow">{#each months as [m, c] (m)}<li><span>{monthName(m)}</span><b>{usd(c)}</b></li>{/each}</ol>
  {:else}<p class="help">{tr("Noch keine.", "None yet.")}</p>{/if}

  <div class="reset">
    {#if confirmReset}
      <span>{tr("Alle Zähler löschen?", "Delete all counters?")}</span>
      <button class="danger" onclick={reset}>{tr("Ja, löschen", "Yes, delete")}</button>
      <button onclick={() => (confirmReset = false)}>{tr("Abbrechen", "Cancel")}</button>
    {:else}
      <button onclick={() => (confirmReset = true)}>{tr("Statistik zurücksetzen", "Reset statistics")}</button>
    {/if}
  </div>
{:else}
  <p class="help">…</p>
{/if}

<style>
  h2 {
    margin: 0 0 4px;
    font-size: 20px;
  }
  h3 {
    margin: 12px 0 0;
    font-size: 14px;
  }
  .intro {
    margin: -6px 0 6px;
    font-size: 13px;
    color: var(--muted);
  }
  .help {
    font-size: 12px;
    color: var(--faint);
    margin: 0;
  }
  .table {
    display: grid;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    font-size: 13px;
  }
  .tr {
    display: grid;
    grid-template-columns: minmax(130px, 1.4fr) repeat(4, minmax(70px, 1fr));
    gap: 10px;
    padding: 6px 12px;
    border-top: 1px solid var(--border);
  }
  .tr.head {
    border-top: 0;
    background: var(--panel);
    font-weight: 600;
    color: var(--muted);
    font-size: 12px;
  }
  .tr span:not(:first-child) {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .zero {
    color: var(--faint);
  }
  .chart {
    display: flex;
    gap: 3px;
    align-items: flex-end;
    height: 110px;
    padding: 8px 10px 0;
    border: 1px solid var(--border);
    border-radius: 10px 10px 0 0;
    border-bottom: 0;
  }
  .col {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .bar {
    width: 100%;
    min-height: 2px;
    border-radius: 3px 3px 0 0;
    background: var(--accent);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-top: 1px solid var(--border);
    border-radius: 0 0 10px 10px;
    font-size: 11.5px;
    color: var(--faint);
  }
  .tops {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .chips {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chips li {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 40px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .big {
    font-size: 22px;
    line-height: 1.2;
  }
  .chips small {
    font-size: 11px;
    color: var(--muted);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 13px;
  }
  .list.narrow {
    max-width: 360px;
  }
  .list li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 4px 2px;
    border-bottom: 1px solid var(--border);
  }
  .list b {
    font-variant-numeric: tabular-nums;
  }
  .reset {
    margin-top: 18px;
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
