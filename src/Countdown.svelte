<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, param, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  // Rust takes the shot when its own timer runs out; closing this cancels it.
  let left = $state(Number(param("s") ?? 3));

  onMount(() => {
    const timer = setInterval(() => (left = Math.max(1, left - 1)), 1000);
    tick().then(ready);
    return () => clearInterval(timer);
  });
</script>

<button class="countdown" title={tr("Cancel")} onclick={closeWindow}>
  <strong>{left}</strong>
  <span>{tr("Click to cancel")}</span>
</button>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .countdown {
    position: fixed;
    inset: 0;
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    border: 0;
    background: rgba(28, 28, 30, 0.85);
    color: #f2f2f7;
    cursor: default;
  }
  strong {
    font-size: 26px;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  span {
    color: #a1a1a6;
    font-size: 10px;
  }
</style>
