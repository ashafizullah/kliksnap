<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, invoke, isMac, isWindows, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  // An empty `text` means nothing was copied and `title` says why.
  // `path` is a saved file, shown in Finder or Explorer on click.
  let toast = $state({ title: "", text: "", path: "" });

  onMount(async () => {
    toast = await invoke<{ title: string; text: string; path: string }>("toast_text");
    await tick();
    ready();
    // Long enough to read: longer text stays longer.
    setTimeout(closeWindow, toast.path ? 6000 : toast.text ? Math.max(3500, toast.text.length * 70) : 2500);
  });

  async function onClick() {
    if (toast.path) await invoke("reveal_toast_file").catch(() => {});
    closeWindow();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="toast" class:file={toast.path} title={toast.path ? tr("Show in folder") : undefined} onclick={onClick}>
  <strong>{toast.title}</strong>
  {#if toast.text}
    <p class:one-line={toast.path}>{toast.text}</p>
  {/if}
  {#if toast.path}
    <p class="action">{tr("Click to show in {app}", { app: isMac ? "Finder" : isWindows ? "Explorer" : tr("the file manager") })}</p>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  /* The preview's card: a light border for dark screens, a dark ring for light ones. */
  .toast {
    position: fixed;
    inset: 1px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    border-radius: 12px;
    background: #1c1c1e;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.45);
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
