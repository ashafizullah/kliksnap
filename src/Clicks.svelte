<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { ready } from "./lib/api";

  // Drawn over the screen while recording, so the clicks are recorded too;
  // the window lets every click through.
  type Mouse = { x: number; y: number; down: boolean };

  let held = $state<Mouse | null>(null);
  let ripples = $state<{ id: number; x: number; y: number }[]>([]);
  let next = 0;

  onMount(() => {
    const unlisten = listen<Mouse>("clicks:mouse", ({ payload: m }) => {
      if (m.down && !held) ripples.push({ id: next++, x: m.x, y: m.y });
      held = m.down ? m : null;
    });
    unlisten.then(ready);
    return () => unlisten.then((off) => off());
  });

  const done = (id: number) => (ripples = ripples.filter((r) => r.id !== id));
</script>

{#if held}
  <div class="dot" style="left: {held.x}px; top: {held.y}px"></div>
{/if}
{#each ripples as r (r.id)}
  <div class="ripple" style="left: {r.x}px; top: {r.y}px" onanimationend={() => done(r.id)}></div>
{/each}

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .dot,
  .ripple {
    position: fixed;
    border-radius: 50%;
    pointer-events: none;
    transform: translate(-50%, -50%);
  }
  .dot {
    width: 34px;
    height: 34px;
    background: rgba(255, 196, 0, 0.35);
    border: 2px solid rgba(255, 170, 0, 0.85);
  }
  .ripple {
    width: 64px;
    height: 64px;
    border: 3px solid rgba(255, 170, 0, 0.9);
    animation: ripple 0.5s ease-out forwards;
  }
  @keyframes ripple {
    from {
      transform: translate(-50%, -50%) scale(0.3);
      opacity: 1;
    }
    to {
      transform: translate(-50%, -50%) scale(1);
      opacity: 0;
    }
  }
</style>
