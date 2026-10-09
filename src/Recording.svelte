<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  const start = Date.now();
  let elapsed = $state(0);
  let stopping = $state(false);

  const clock = $derived(`${Math.floor(elapsed / 60)}:${String(elapsed % 60).padStart(2, "0")}`);

  function stop() {
    stopping = true;
    invoke("stop_recording");
  }

  onMount(() => {
    const timer = setInterval(() => (elapsed = Math.floor((Date.now() - start) / 1000)), 250);
    tick().then(ready);
    return () => clearInterval(timer);
  });
</script>

<div class="bar">
  <span class="dot" aria-hidden="true"></span>
  <span class="clock" aria-label={tr("Recording time")}>{clock}</span>
  <button onclick={stop} disabled={stopping}>
    <span class="square" aria-hidden="true"></span>
    {tr("Stop")}
  </button>
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
  button {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border: 0;
    border-radius: 6px;
    background: #ff3b30;
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
