<script lang="ts">
  // Source-app badge (colored initial), or the image thumbnail / kind icon.
  import type { EntrySummary } from "../api";
  import Icon from "../Icon.svelte";
  import { appColor, appInitial, colorValue } from "../view";

  let { entry, thumb = null, size = 32, round = false }: { entry: EntrySummary; thumb?: string | null; size?: number; round?: boolean } = $props();
  const swatch = $derived(colorValue(entry));
</script>

<span class="av" class:round style:width="{size}px" style:height="{size}px" style:--c={appColor(entry.sourceApp)}>
  {#if thumb}
    <img src={thumb} alt="" />
  {:else if swatch}
    <span class="sw" style:background={swatch}></span>
  {:else if entry.kind === "image" || entry.kind === "files" || entry.kind === "link"}
    <span class="ic"><Icon name={entry.kind} /></span>
  {:else}
    <span class="ini">{appInitial(entry.sourceApp)}</span>
  {/if}
</span>

<style>
  .av {
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--c);
    color: #fff;
    overflow: hidden;
  }
  .av.round {
    border-radius: 50%;
  }
  .ini {
    font: 600 13px var(--font);
  }
  .ic {
    display: grid;
    place-items: center;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .sw {
    width: 100%;
    height: 100%;
  }
</style>
