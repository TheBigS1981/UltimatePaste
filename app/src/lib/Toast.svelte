<script lang="ts">
  // Small notice at the bottom right (direct translation); never takes the focus.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "./api";
  import { applyLook, applyTheme } from "./theme";

  let text = $state("");

  async function load() {
    text = await api.toastText();
  }

  onMount(() => {
    api.settings().then((s) => {
      applyTheme(s.theme);
      applyLook(s.layout, s.design);
    });
    load();
    const un = listen("toast", load);
    return () => un.then((f) => f());
  });
</script>

<div class="scene">
  <div class="toast" role="status">
    <span class="dot" aria-hidden="true"></span>
    <span class="txt">{text}</span>
  </div>
</div>

<style>
  .scene {
    height: 100%;
    padding: 8px;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    font: 13px/1.35 var(--font);
  }
  .toast {
    display: flex;
    gap: 10px;
    align-items: center;
    max-width: 100%;
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--bg-solid);
    border: 1px solid var(--border);
    box-shadow: 0 10px 28px -10px rgba(0, 0, 0, 0.45);
    color: var(--text);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
  }
  .txt {
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
</style>
