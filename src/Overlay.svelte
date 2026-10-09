<script lang="ts">
  import { onMount } from "svelte";
  import { imageUrl, invoke, param, ready } from "./lib/api";

  type Mode = "area" | "window" | "text";
  type Rect = { x: number; y: number; w: number; h: number };

  const index = Number(param("m") ?? 0);
  const src = imageUrl(`frozen-${index}-${param("g") ?? 0}`);
  const LOUPE = 120;
  const ZOOM = 8;

  let mode = $state<Mode>("area");
  // Selecting what to record rather than what to capture.
  let recording = $state(false);
  let imageWidth = $state(0);
  let windows = $state<number[][] | null>(null);
  let mouse = $state({ x: -1, y: -1 });
  let dragStart = $state<{ x: number; y: number } | null>(null);
  let viewport = $state({ w: innerWidth, h: innerHeight });
  let img = $state<HTMLImageElement>()!;
  let loupe = $state<HTMLCanvasElement>();
  let loaded = $state(false);
  // The color under the crosshair; `C` copies it.
  let hex = $state<string | null>(null);
  // Live selection shows the screen itself through the window; null until known.
  let live = $state<boolean | null>(null);
  let done = false;

  const selection: Rect | null = $derived(
    dragStart
      ? {
          x: Math.min(dragStart.x, mouse.x),
          y: Math.min(dragStart.y, mouse.y),
          w: Math.abs(mouse.x - dragStart.x),
          h: Math.abs(mouse.y - dragStart.y),
        }
      : null,
  );

  // Topmost window under the cursor, in CSS pixels.
  const hovered: Rect | null = $derived.by(() => {
    if (mode !== "window" || !windows) return null;
    const fx = mouse.x / viewport.w;
    const fy = mouse.y / viewport.h;
    const hit = windows.find(([x, y, w, h]) => fx >= x && fx < x + w && fy >= y && fy < y + h);
    if (!hit) return null;
    const x = Math.max(0, hit[0] * viewport.w);
    const y = Math.max(0, hit[1] * viewport.h);
    return {
      x,
      y,
      w: Math.min(viewport.w, (hit[0] + hit[2]) * viewport.w) - x,
      h: Math.min(viewport.h, (hit[1] + hit[3]) * viewport.h) - y,
    };
  });

  const selecting = $derived(mode !== "window");
  const box = $derived(selecting ? selection : hovered);
  const pxRatio = $derived(imageWidth ? imageWidth / viewport.w : devicePixelRatio);

  function finish(rect: Rect | null) {
    if (done) return;
    done = true;
    const fraction = rect && [rect.x / viewport.w, rect.y / viewport.h, rect.w / viewport.w, rect.h / viewport.h];
    invoke("overlay_finish", { index, rect: fraction });
  }

  async function setMode(next: Mode) {
    mode = next;
    dragStart = null;
    if (next === "window" && !windows) windows = await invoke<number[][]>("overlay_windows", { index });
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    if (mode === "window") {
      // Clicking empty desktop captures the whole screen.
      finish(hovered ?? { x: 0, y: 0, w: viewport.w, h: viewport.h });
      return;
    }
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    dragStart = { x: e.clientX, y: e.clientY };
  }

  let lastMove = 0;

  function onPointerMove(e: PointerEvent) {
    lastMove = performance.now();
    mouse = { x: e.clientX, y: e.clientY };
  }

  // The webview gets no mouse moves until KlikSnap is the active app, which
  // can lag behind the overlay showing, nor while the wheel passes through.
  // Until they flow, follow the cursor from Rust so the guides still show.
  async function followCursor() {
    if (done || performance.now() - lastMove < 150) return;
    const p = await invoke<[number, number] | null>("overlay_cursor", { index });
    if (performance.now() - lastMove < 150) return;
    // Off this monitor: hide the guides here, they show on the cursor's monitor.
    mouse = p ? { x: p[0] * viewport.w, y: p[1] * viewport.h } : { x: -1, y: -1 };
  }

  function onPointerUp() {
    if (!selecting || !selection) return;
    if (selection.w >= 3 && selection.h >= 3) finish(selection);
    else dragStart = null;
  }

  // Live selection: scroll the app under the cursor, not the overlay.
  function onWheel() {
    if (live && !dragStart) invoke("overlay_pass_scroll");
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") finish(null);
    else if (e.key.toLowerCase() === "c" && mode === "area" && !recording && hex && !dragStart && !done) {
      done = true;
      invoke("overlay_pick_color", { index, hex });
    }
    else if (e.key === " " && mode !== "text") {
      e.preventDefault();
      setMode(mode === "area" ? "window" : "area");
    }
  }

  // Magnifier for pixel-precise selection.
  $effect(() => {
    if (!loupe || !loaded || live || !selecting || mouse.x < 0) return;
    const span = LOUPE / ZOOM;
    paintLoupe(img, (mouse.x - span / 2) * pxRatio, (mouse.y - span / 2) * pxRatio, span * pxRatio, span * pxRatio);
  });

  // Live selection has no still to magnify: keep fetching the screen under the loupe.
  async function liveLoupe() {
    const scratch = document.createElement("canvas");
    while (!done) {
      if (loupe && selecting && mouse.x >= 0) {
        const span = LOUPE / ZOOM;
        const rect = [
          (mouse.x - span / 2) / viewport.w,
          (mouse.y - span / 2) / viewport.h,
          span / viewport.w,
          span / viewport.h,
        ];
        const buf = await invoke<ArrayBuffer>("overlay_loupe", { index, rect });
        const head = new DataView(buf);
        const w = head.getUint32(0, true);
        const h = head.getUint32(4, true);
        if (w && h && loupe) {
          scratch.width = w;
          scratch.height = h;
          scratch.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(buf, 8, w * h * 4), w, h), 0, 0);
          paintLoupe(scratch, 0, 0, w, h);
        }
      }
      await new Promise((r) => setTimeout(r, 30));
    }
  }

  function paintLoupe(source: CanvasImageSource, sx: number, sy: number, sw: number, sh: number) {
    if (!loupe) return;
    const ctx = loupe.getContext("2d", { willReadFrequently: true })!;
    const dpr = devicePixelRatio;
    loupe.width = LOUPE * dpr;
    loupe.height = LOUPE * dpr;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(source, sx, sy, sw, sh, 0, 0, loupe.width, loupe.height);
    const c = loupe.width / 2;
    try {
      const [r, g, b] = ctx.getImageData(Math.floor(c), Math.floor(c), 1, 1).data;
      hex = "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("").toUpperCase();
    } catch {
      hex = null;
    }
    ctx.strokeStyle = "rgba(37, 99, 235, 0.9)";
    ctx.lineWidth = dpr;
    ctx.beginPath();
    ctx.moveTo(c, 0);
    ctx.lineTo(c, loupe.height);
    ctx.moveTo(0, c);
    ctx.lineTo(loupe.width, c);
    ctx.stroke();
  }

  const loupePos = $derived({
    x: mouse.x + 24 + LOUPE > viewport.w ? mouse.x - 24 - LOUPE : mouse.x + 24,
    y: mouse.y + 24 + LOUPE + 28 > viewport.h ? mouse.y - 24 - LOUPE - 28 : mouse.y + 24,
  });

  onMount(() => {
    const timer = setInterval(followCursor, 30);
    init();
    return () => clearInterval(timer);
  });

  async function init() {
    const info = await invoke<{ mode: Mode | "screen" | "record" | "record_gif"; width: number } | null>("overlay_info", { index });
    if (!info) return finish(null);
    imageWidth = info.width;
    live = info.width === 0;
    if (info.mode === "window" || info.mode === "text") await setMode(info.mode);
    recording = info.mode === "record" || info.mode === "record_gif";
    if (live) {
      ready();
      liveLoupe();
    }
  }

  async function onImageLoad() {
    loaded = true;
    // Show the window only once the capture is decoded, or its black
    // background flashes first. Not requestAnimationFrame: it never fires
    // while the window is hidden. The timeout guards against a stalled decode.
    await Promise.race([img.decode().catch(() => {}), new Promise((r) => setTimeout(r, 300))]);
    ready();
  }
</script>

<svelte:window
  onkeydown={onKeyDown}
  onresize={() => (viewport = { w: innerWidth, h: innerHeight })}
  oncontextmenu={(e) => e.preventDefault()}
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="stage"
  class:window-mode={mode === "window"}
  class:live
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onwheel={onWheel}
>
  {#if live === false}
    <img bind:this={img} {src} alt="" crossorigin="anonymous" draggable="false" onload={onImageLoad} onerror={() => finish(null)} />
  {/if}

  {#if box}
    <div class="box" style="left:{box.x}px; top:{box.y}px; width:{box.w}px; height:{box.h}px">
      <span class="size">{Math.round(box.w * pxRatio)} × {Math.round(box.h * pxRatio)}</span>
    </div>
  {:else}
    <div class="dim"></div>
  {/if}

  {#if selecting && mouse.x >= 0}
    {#if !selection}
      <div class="guide h" style="top:{mouse.y}px"></div>
      <div class="guide v" style="left:{mouse.x}px"></div>
    {/if}
    <div class="loupe" style="left:{loupePos.x}px; top:{loupePos.y}px">
      <canvas bind:this={loupe} style="width:{LOUPE}px; height:{LOUPE}px"></canvas>
      <span>
        {Math.round(mouse.x * pxRatio)}, {Math.round(mouse.y * pxRatio)}
        {#if hex}<i class="swatch" style="background:{hex}"></i>{hex}{/if}
      </span>
    </div>
  {/if}

  <div class="hint">
    {#if mode === "text"}
      Drag over text to copy it
    {:else}
      {#if recording}<strong>Record</strong> ·{/if}
      {mode === "area" ? "Drag to select" : "Click a window"} · <kbd>Space</kbd>
      {mode === "area" ? "window mode" : "area mode"}
      {#if mode === "area" && !recording}· <kbd>C</kbd> copy color{/if}
    {/if}
    {#if live}· Scroll works{/if}
    · <kbd>Esc</kbd> cancel
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .stage {
    position: fixed;
    inset: 0;
    cursor: crosshair;
    overflow: hidden;
  }
  .stage.window-mode {
    cursor: default;
  }
  /* Fully transparent pixels let clicks through to the apps below. */
  .stage.live {
    background: rgba(0, 0, 0, 0.01);
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
  .dim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
  }
  .box {
    position: absolute;
    outline: 1px solid rgba(255, 255, 255, 0.9);
    box-shadow: 0 0 0 100vmax rgba(0, 0, 0, 0.3);
  }
  .window-mode .box {
    outline: 3px solid rgba(37, 99, 235, 0.9);
    outline-offset: -3px;
    background: rgba(37, 99, 235, 0.12);
  }
  .size,
  .loupe span,
  .hint {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: #fff;
    background: rgba(0, 0, 0, 0.72);
    border-radius: 4px;
    padding: 2px 6px;
    white-space: nowrap;
  }
  .size {
    position: absolute;
    left: 0;
    top: -22px;
  }
  .guide {
    position: absolute;
    background: rgba(255, 255, 255, 0.55);
    pointer-events: none;
  }
  .guide.h {
    left: 0;
    right: 0;
    height: 1px;
  }
  .guide.v {
    top: 0;
    bottom: 0;
    width: 1px;
  }
  .swatch {
    display: inline-block;
    width: 9px;
    height: 9px;
    margin: 0 4px 0 6px;
    border: 1px solid rgba(255, 255, 255, 0.8);
    border-radius: 2px;
    vertical-align: -1px;
  }
  .loupe {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    pointer-events: none;
  }
  .loupe canvas {
    border-radius: 50%;
    border: 2px solid #fff;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.45);
    background: #000;
  }
  .hint {
    position: absolute;
    top: 14px;
    left: 50%;
    transform: translateX(-50%);
    padding: 5px 10px;
    font-size: 12px;
    pointer-events: none;
  }
  kbd {
    font: inherit;
    padding: 0 4px;
    border: 1px solid rgba(255, 255, 255, 0.35);
    border-radius: 3px;
  }
</style>
