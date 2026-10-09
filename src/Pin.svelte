<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { closeWindow, imageUrl, invoke, isMac, param, ready } from "./lib/api";

  const id = Number(param("id"));
  const win = getCurrentWindow();
  const MIN_SIDE = 24;

  // The shot's size at 100%, in logical pixels.
  let natural = { w: 0, h: 0 };
  let zoom = 1;
  let shown = false;
  let lastDown = 0;
  let opacity = $state(1);
  let status = $state("");
  let statusTimer: ReturnType<typeof setTimeout> | undefined;

  function flash(text: string) {
    status = text;
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => (status = ""), text.length > 20 ? 2500 : 900);
  }

  async function onLoad(e: Event) {
    if (shown) return;
    shown = true;
    const img = e.currentTarget as HTMLImageElement;
    const info = await invoke<{ scale: number } | null>("shot_info", { id });
    const scale = info?.scale || devicePixelRatio;
    natural = { w: img.naturalWidth / scale, h: img.naturalHeight / scale };
    // Rust shrinks shots that don't fit the screen.
    zoom = innerWidth / natural.w;
    ready();
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    // Dragging swallows the second click, so spot double-clicks by hand.
    const now = performance.now();
    if (now - lastDown < 350) {
      closeWindow();
      return;
    }
    lastDown = now;
    win.startDragging();
  }

  function setZoom(next: number) {
    if (!natural.w) return;
    next = Math.min(4, Math.max(MIN_SIDE / Math.min(natural.w, natural.h), next));
    if (next === zoom) return;
    zoom = next;
    win.setSize(new LogicalSize(Math.round(natural.w * zoom), Math.round(natural.h * zoom)));
    flash(`${Math.round(zoom * 100)}%`);
  }

  function setOpacity(next: number) {
    opacity = Math.round(Math.min(1, Math.max(0.2, next)) * 100) / 100;
    flash(`Opacity ${Math.round(opacity * 100)}%`);
  }

  // Scroll zooms; with Alt (Option) held it fades the pin instead.
  function onWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.altKey) setOpacity(opacity - Math.sign(e.deltaY) * 0.05);
    else setZoom(zoom * Math.exp(-e.deltaY * 0.002));
  }

  onMount(() => {
    const off = [
      listen<number>("pin:opacity", (e) => setOpacity(e.payload)),
      listen<boolean>("pin:click-through", (e) => {
        if (e.payload && opacity === 1) opacity = 0.6;
        flash(e.payload ? "Clicks pass through · undo from the menu bar icon" : "Clickable again");
      }),
    ];
    return () => off.forEach((p) => p.then((unlisten) => unlisten()));
  });

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    invoke("pin_menu");
  }

  function onKeyDown(e: KeyboardEvent) {
    const cmd = isMac ? e.metaKey : e.ctrlKey;
    const key = e.key.toLowerCase();
    if (e.key === "Escape" || (cmd && key === "w")) closeWindow();
    else if (cmd && key === "c") invoke("copy_shot", { id }).then(() => flash("Copied"), (err) => flash(String(err)));
    else if (key === "0") setZoom(1);
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pin" style:opacity={opacity} onpointerdown={onPointerDown} onwheel={onWheel} oncontextmenu={onContextMenu}>
  <img src={imageUrl(`shot-${id}`)} alt="Pinned screenshot" draggable="false" onload={onLoad} onerror={closeWindow} />
  {#if status}
    <div class="status">{status}</div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .pin {
    position: fixed;
    inset: 0;
    cursor: grab;
  }
  /* A hairline edge, so a pin can be told apart from the screen under it. */
  .pin::after {
    content: "";
    position: absolute;
    inset: 0;
    box-shadow: inset 0 0 0 1px rgba(127, 127, 127, 0.6);
    pointer-events: none;
  }
  img {
    width: 100%;
    height: 100%;
    display: block;
  }
  .status {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.75);
    color: #fff;
    font-size: 12px;
    white-space: nowrap;
    pointer-events: none;
  }
</style>
