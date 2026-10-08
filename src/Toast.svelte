<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, invoke, isMac, ready } from "./lib/api";

  // An empty `text` means nothing was copied and `title` says why.
  // `path` is a saved file, shown in Finder or Explorer on click.
  let toast = $state({ title: "", text: "", path: "" });

  onMount(async () => {
    toast = await invoke<{ title: string; text: string; path: string }>("toast_text");
    await tick();
    ready();
    setTimeout(closeWindow, toast.path ? 6000 : toast.text ? 3500 : 2500);
  });

  async function onClick() {
    if (toast.path) await invoke("reveal_toast_file").catch(() => {});
    closeWindow();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="toast" class:file={toast.path} title={toast.path ? "Show in folder" : undefined} onclick={onClick}>
  <strong>{toast.title}</strong>
  {#if toast.text}
    <p class:one-line={toast.path}>{toast.text}</p>
  {/if}
  {#if toast.path}
    <p class="action">Click to show in {isMac ? "Finder" : "Explorer"}</p>
  {/if}
</div>

<style>
  :global(body) {
    background: #1c1c1e;
  }
  .toast {
    position: fixed;
    inset: 0;
    /* Centered, so short and long notices both sit evenly in the window. */
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 10px 14px;
    color: #f2f2f7;
    cursor: default;
  }
  strong {
    display: block;
    font-size: 13px;
  }
  .file {
    cursor: pointer;
  }
  .action {
    color: #60a5fa;
  }
  p.one-line {
    -webkit-line-clamp: 1;
    line-clamp: 1;
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
