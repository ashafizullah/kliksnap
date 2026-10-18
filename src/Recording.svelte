<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke, param, ready, shortcutLabel } from "./lib/api";
  import { tr } from "./lib/i18n";

  const start = Date.now();
  // The record shortcut stops the recording too.
  const key = param("key") ? shortcutLabel(param("key")!) : "";
  let elapsed = $state(0);
  let stopping = $state(false);

  const clock = $derived(`${Math.floor(elapsed / 60)}:${String(elapsed % 60).padStart(2, "0")}`);

  function stop() {
    stopping = true;
    invoke("stop_recording");
  }

  // Dragged anywhere but the button; it stays where it's put, rather than
  // moving out of the webcam's way.
  function drag(e: MouseEvent) {
    if (e.button !== 0 || (e.target as Element).closest("button")) return;
    invoke("controls_moved");
    getCurrentWindow().startDragging();
  }

  onMount(() => {
    const timer = setInterval(() => (elapsed = Math.floor((Date.now() - start) / 1000)), 250);
    tick().then(ready);
    return () => clearInterval(timer);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="bar" onmousedown={drag}>
  <span class="dot" aria-hidden="true"></span>
  <span class="clock" aria-label={tr("Recording time")}>{clock}</span>
  {#if key}
    <kbd title={tr("Press to stop")}>{key}</kbd>
  {/if}
  <button onclick={stop} disabled={stopping}>
    <span class="square" aria-hidden="true"></span>
    {tr("Stop")}
  </button>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .bar {
    position: fixed;
    inset: 0;
    border-radius: 10px;
    background: rgba(28, 28, 30, 0.55);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 12px;
    color: #f2f2f7;
    cursor: grab;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #ff3b30;
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .clock {
    flex: 1;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  kbd {
    font: inherit;
    font-size: 11px;
    color: rgba(242, 242, 247, 0.75);
  }
  button {
    cursor: default;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border: 0;
    border-radius: 6px;
    background: rgba(255, 59, 48, 0.85);
    color: #fff;
    font-weight: 600;
  }
  button:hover:not(:disabled) {
    filter: brightness(1.1);
  }
  button:disabled {
    opacity: 0.6;
  }
  .square {
    width: 8px;
    height: 8px;
    border-radius: 1px;
    background: currentColor;
  }
  @media (prefers-reduced-motion: reduce) {
    .dot {
      animation: none;
    }
  }
</style>
