<script lang="ts">
  // Language dropdown with flags (a native <select> can't show images).
  import { tick } from "svelte";
  import { flagUrl } from "./flags";

  type Opt = { code: string; label: string; flag?: string };
  let {
    value,
    options,
    onchange,
    label = "",
    minWidth = 180,
  }: { value: string; options: Opt[]; onchange: (v: string) => void; label?: string; minWidth?: number } = $props();

  let open = $state(false);
  let active = $state(0);
  let root = $state<HTMLDivElement>();
  let list = $state<HTMLUListElement>();
  let typed = "";
  let typedAt = 0;
  const current = $derived(options.find((o) => o.code === value) ?? options[0]);
  const flagOf = (o: Opt) => (o.flag === undefined ? flagUrl(o.code) : o.flag ? flagUrl(o.flag) : undefined);

  async function toggle() {
    open = !open;
    if (open) {
      active = Math.max(0, options.findIndex((o) => o.code === value));
      await tick();
      list?.focus();
      list?.children[active]?.scrollIntoView({ block: "nearest" });
    }
  }

  function pick(i: number) {
    open = false;
    root?.querySelector("button")?.focus();
    if (options[i] && options[i].code !== value) onchange(options[i].code);
  }

  function onKey(e: KeyboardEvent) {
    // Keep the window's own shortcuts (Esc closes, Enter pastes) away while the list is open.
    e.stopPropagation();
    if (e.key === "ArrowDown") (e.preventDefault(), (active = Math.min(options.length - 1, active + 1)));
    else if (e.key === "ArrowUp") (e.preventDefault(), (active = Math.max(0, active - 1)));
    else if (e.key === "Home") (e.preventDefault(), (active = 0));
    else if (e.key === "End") (e.preventDefault(), (active = options.length - 1));
    else if (e.key === "Enter" || e.key === " ") (e.preventDefault(), pick(active));
    else if (e.key === "Escape" || e.key === "Tab") (e.preventDefault(), (open = false), root?.querySelector("button")?.focus());
    else if (e.key.length === 1) {
      // Type-ahead: jump to the first label starting with the typed letters.
      typed = Date.now() - typedAt < 700 ? typed + e.key.toLowerCase() : e.key.toLowerCase();
      typedAt = Date.now();
      const i = options.findIndex((o) => o.label.toLowerCase().startsWith(typed));
      if (i >= 0) active = i;
    }
    list?.children[active]?.scrollIntoView({ block: "nearest" });
  }

  function onButtonKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") (e.preventDefault(), e.stopPropagation(), toggle());
  }
</script>

<svelte:window onpointerdown={(e) => open && root && !root.contains(e.target as Node) && (open = false)} />

<div class="ls" bind:this={root} style:min-width={`${minWidth}px`}>
  <button type="button" class="cur" aria-haspopup="listbox" aria-expanded={open} aria-label={label} onclick={toggle} onkeydown={onButtonKey}>
    {#if current && flagOf(current)}<img src={flagOf(current)} alt="" />{:else}<span class="noflag" aria-hidden="true"></span>{/if}
    <span class="txt">{current?.label ?? ""}</span>
    <span class="caret" aria-hidden="true">▾</span>
  </button>
  {#if open}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <ul class="pop" role="listbox" tabindex="0" bind:this={list} onkeydown={onKey} aria-label={label}>
      {#each options as o, i (o.code)}
        <li role="option" aria-selected={o.code === value} class:active={i === active} class:sel={o.code === value} onpointerenter={() => (active = i)} onclick={() => pick(i)} onkeydown={() => {}}>
          {#if flagOf(o)}<img src={flagOf(o)} alt="" />{:else}<span class="noflag" aria-hidden="true"></span>{/if}
          <span>{o.label}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .ls {
    position: relative;
    display: inline-block;
  }
  .cur {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    font-size: 13px;
    color: var(--text);
  }
  .txt {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    color: var(--muted);
    font-size: 11px;
  }
  img,
  .noflag {
    flex: none;
    width: 20px;
    height: 15px;
    border-radius: 2px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.12);
  }
  .noflag {
    box-shadow: none;
  }
  .pop {
    position: absolute;
    z-index: 30;
    top: calc(100% + 4px);
    left: 0;
    min-width: 100%;
    max-height: 320px;
    overflow-y: auto;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--bg-solid);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 12px 30px -10px rgba(0, 0, 0, 0.45);
    outline: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 6px;
    font-size: 13px;
    white-space: nowrap;
    cursor: pointer;
  }
  li.active {
    background: var(--accent-soft, var(--panel));
  }
  li.sel {
    font-weight: 600;
  }
</style>
