<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  type Progress = { height: number; lost: boolean; full: boolean; auto: boolean; needs_permission: boolean };

  let progress = $state<Progress>({ height: 0, lost: false, full: false, auto: false, needs_permission: false });
  const size = $derived(`${progress.height.toLocaleString()} px`);
  let ending = $state(false);

  // KlikSnap scrolls by itself when it may, and stops at the end; by hand
  // otherwise (no permission on macOS, Linux, or when it lost its place).
  const message = $derived(
    progress.full
      ? tr("Maximum height reached")
      : progress.auto
        ? `${tr("Scrolling…")} ${size}`
        : progress.lost
          ? tr("Too fast: scroll back up a little")
          : progress.needs_permission
            ? tr("Scroll by hand, or allow Accessibility to auto-scroll")
            : progress.height
              ? size
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
  <span
    class="message"
    role="status"
    title={progress.needs_permission
      ? tr("Allow KlikSnap in System Settings → Privacy & Security → Accessibility, then start the capture again.")
      : undefined}>{message}</span
  >
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
