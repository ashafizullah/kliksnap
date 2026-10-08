<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, invoke, ready } from "./lib/api";

  let text = $state("");
  const failed = $derived(text === "No text found" || text.startsWith("Text recognition failed"));

  onMount(async () => {
    text = await invoke<string>("toast_text");
    await tick();
    ready();
    setTimeout(closeWindow, failed ? 2500 : 3500);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="toast" onclick={closeWindow}>
  {#if failed}
    <strong>{text}</strong>
  {:else}
    <strong>Text copied</strong>
    <p>{text}</p>
  {/if}
</div>

<style>
  :global(body) {
    background: #1c1c1e;
  }
  .toast {
    position: fixed;
    inset: 0;
    padding: 10px 14px;
    color: #f2f2f7;
    cursor: default;
  }
  strong {
    display: block;
    font-size: 13px;
  }
  p {
    margin: 2px 0 0;
    color: #a1a1a6;
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-line;
  }
</style>
