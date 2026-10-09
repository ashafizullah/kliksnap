<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  type Progress = { height: number; lost: boolean; full: boolean };

  let progress = $state<Progress>({ height: 0, lost: false, full: false });
  let ending = $state(false);

  const message = $derived(
    progress.full
      ? tr("Maximum height reached")
      : progress.lost
        ? tr("Too fast: scroll back up a little")
        : progress.height
          ? `${progress.height.toLocaleString()} px`
          : tr("Scroll down slowly"),
  );

  function end(done: boolean) {
    ending = true;
    invoke("end_scroll", { done });
  }

  onMount(() => {
    const unlisten = listen<Progress>("scroll:progress", (e) => (progress = e.payload));
    tick().then(ready);
    return () => unlisten.then((off) => off());
  });
</script>

<div class="bar">
  <span class="dot" class:warn={progress.lost || progress.full} aria-hidden="true"></span>
  <span class="message" role="status">{message}</span>
  <button class="cancel" onclick={() => end(false)} disabled={ending}>{tr("Cancel")}</button>
  <button class="done" onclick={() => end(true)} disabled={ending}>{tr("Done")}</button>
</div>

<style>
  :global(body) {
    background: #1c1c1e;
  }
  .bar {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 12px;
    color: #f2f2f7;
  }
  .dot {
    width: 10px;
    height: 10px;
    flex: none;
    border-radius: 50%;
    background: #34c759;
  }
  .dot.warn {
    background: #ff9f0a;
  }
  .message {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  button {
    height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: 6px;
    font-weight: 600;
  }
  button:disabled {
    opacity: 0.6;
  }
  .cancel {
    background: #3a3a3c;
    color: #f2f2f7;
  }
  .done {
    background: #2563eb;
    color: #fff;
  }
  button:hover:not(:disabled) {
    filter: brightness(1.15);
  }
</style>
