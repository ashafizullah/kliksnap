<script lang="ts">
  import { onMount } from "svelte";
  import { emit, listen } from "@tauri-apps/api/event";
  import { imageUrl, invoke, param, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  type Mode = "area" | "window" | "text";
  type Rect = { x: number; y: number; w: number; h: number };
  /** What a drag on an adjustable selection changes: some of its edges (n, s, e, w) or all of it. */
  type Part = "move" | "n" | "s" | "e" | "w" | "nw" | "ne" | "sw" | "se";

  const index = Number(param("m") ?? 0);
  const src = imageUrl(`frozen-${index}-${param("g") ?? 0}`);
  const LOUPE = 120;
  const ZOOM = 8;

  let mode = $state<Mode>("area");
  // Selecting what to record rather than what to capture.
  let recording = $state(false);
  // Selecting what to scroll and stitch.
  let scrolling = $state(false);
  let imageWidth = $state(0);
  let windows = $state<number[][] | null>(null);
  let mouse = $state({ x: -1, y: -1 });
  let dragStart = $state<{ x: number; y: number } | null>(null);
  let viewport = $state({ w: innerWidth, h: innerHeight });
  let img = $state<HTMLImageElement>()!;
  let loupe = $state<HTMLCanvasElement>();
  let loaded = $state(false);
  // The color under the cursor. `C` turns the cursor into an eyedropper;
  // a click then copies the color.
  let hex = $state<string | null>(null);
  let picking = $state(false);
  // Live selection shows the screen itself through the window; null until known.
  let live = $state<boolean | null>(null);
  let done = false;
  // Adjustable selection: once the mouse is released the area stays, with
  // handles, until Enter takes it.
  let adjust = $state(false);
  let region = $state<Rect | null>(null);
  let edit = $state<{ part: Part; from: { x: number; y: number }; start: Rect } | null>(null);
  // How close to an edge counts as grabbing it, in CSS pixels.
  const GRAB = 6;
  const canPick = $derived(mode === "area" && !recording && !scrolling && !region);

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
  const box = $derived(selecting ? (region ?? selection) : hovered);

  /** The part of the adjustable selection under the cursor, if any. */
  function partAt(x: number, y: number): Part | null {
    if (!region) return null;
    const { x: l, y: t, w, h } = region;
    const r = l + w;
    const b = t + h;
    if (x < l - GRAB || x > r + GRAB || y < t - GRAB || y > b + GRAB) return null;
    // Small selections leave the middle for moving.
    const grab = Math.min(GRAB, w / 3, h / 3);
    const ns = y <= t + grab ? "n" : y >= b - grab ? "s" : "";
    const ew = x <= l + grab ? "w" : x >= r - grab ? "e" : "";
    return ((ns + ew) as Part) || "move";
  }

  const CURSORS: Record<Part, string> = {
    move: "move",
    n: "ns-resize",
    s: "ns-resize",
    e: "ew-resize",
    w: "ew-resize",
    nw: "nwse-resize",
    se: "nwse-resize",
    ne: "nesw-resize",
    sw: "nesw-resize",
  };
  const cursor = $derived.by(() => {
    const part = edit?.part ?? (picking ? null : partAt(mouse.x, mouse.y));
    return part ? CURSORS[part] : null;
  });

  /** `start` with the edges in `part` dragged by (dx, dy), kept on the screen. */
  function adjusted(part: Part, start: Rect, dx: number, dy: number): Rect {
    const clampX = (v: number) => Math.min(viewport.w, Math.max(0, v));
    const clampY = (v: number) => Math.min(viewport.h, Math.max(0, v));
    if (part === "move") {
      return {
        ...start,
        x: Math.min(viewport.w - start.w, Math.max(0, start.x + dx)),
        y: Math.min(viewport.h - start.h, Math.max(0, start.y + dy)),
      };
    }
    let l = start.x;
    let t = start.y;
    let r = start.x + start.w;
    let b = start.y + start.h;
    if (part.includes("w")) l = clampX(l + dx);
    if (part.includes("e")) r = clampX(r + dx);
    if (part.includes("n")) t = clampY(t + dy);
    if (part.includes("s")) b = clampY(b + dy);
    // Dragging an edge past the opposite one flips the selection.
    return { x: Math.min(l, r), y: Math.min(t, b), w: Math.abs(r - l), h: Math.abs(b - t) };
  }

  /** Takes the adjustable selection. */
  function confirm(textOnly = false) {
    if (region && region.w >= 3 && region.h >= 3) finish(region, mode === "text" && textOnly);
  }

  // The confirm buttons sit under the selection, or inside it at the bottom of the screen.
  const TOOLS_H = 30;
  const toolsPos = $derived(
    region && {
      right: Math.max(4, viewport.w - (region.x + region.w)),
      top:
        region.y + region.h + 8 + TOOLS_H <= viewport.h
          ? region.y + region.h + 8
          : Math.max(4, region.y + region.h - 8 - TOOLS_H),
    },
  );
  const pxRatio = $derived(imageWidth ? imageWidth / viewport.w : devicePixelRatio);

  function finish(rect: Rect | null, textOnly = false) {
    if (done) return;
    done = true;
    const fraction = rect && [rect.x / viewport.w, rect.y / viewport.h, rect.w / viewport.w, rect.h / viewport.h];
    invoke("overlay_finish", { index, rect: fraction, textOnly });
  }

  async function setMode(next: Mode) {
    mode = next;
    dragStart = null;
    region = null;
    edit = null;
    picking = false;
    if (next === "window" && !windows) windows = await invoke<number[][]>("overlay_windows", { index });
  }

  /** Space switches every monitor's overlay, not just the one with the keyboard. */
  function toggleMode() {
    const next = mode === "area" ? "window" : "area";
    setMode(next);
    emit("overlay:mode", { from: index, mode: next });
  }

  // The keyboard goes to one overlay: the one the cursor was on when they
  // opened. Moving to another monitor takes it along, for C and Esc too.
  let focusAsked = 0;
  function onPointerEnter() {
    if (document.hasFocus() || performance.now() - focusAsked < 300) return;
    focusAsked = performance.now();
    invoke("overlay_focus");
  }

  function pickColor() {
    if (!hex || done) return;
    done = true;
    invoke("overlay_pick_color", { index, hex });
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    if (picking) return pickColor();
    if (mode === "window") {
      // Clicking empty desktop captures the whole screen.
      finish(hovered ?? { x: 0, y: 0, w: viewport.w, h: viewport.h });
      return;
    }
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const part = partAt(e.clientX, e.clientY);
    if (region && part) {
      edit = { part, from: { x: e.clientX, y: e.clientY }, start: region };
      return;
    }
    // Pressing outside the selection starts a new one.
    region = null;
    dragStart = { x: e.clientX, y: e.clientY };
  }

  let lastMove = 0;

  function onPointerMove(e: PointerEvent) {
    lastMove = performance.now();
    mouse = { x: e.clientX, y: e.clientY };
    if (edit) region = adjusted(edit.part, edit.start, mouse.x - edit.from.x, mouse.y - edit.from.y);
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
    // On it, without pointer events yet: take the keyboard here as well.
    if (p) onPointerEnter();
  }

  function onPointerUp(e: PointerEvent) {
    if (edit) {
      edit = null;
      return;
    }
    if (!selecting || !selection) return;
    if (selection.w < 3 || selection.h < 3) dragStart = null;
    else if (adjust) {
      region = selection;
      dragStart = null;
    }
    // Copy Text: Shift skips QR codes, for text that sits beside one.
    else finish(selection, mode === "text" && e.shiftKey);
  }

  function onDoubleClick(e: MouseEvent) {
    if (partAt(e.clientX, e.clientY) === "move") confirm(e.shiftKey);
  }

  /** Arrows move the selection by a pixel of the screen; with Shift they resize it. */
  function nudge(key: string, resize: boolean) {
    if (!region) return;
    const step = 1 / pxRatio;
    const [dx, dy] = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[key]!;
    const next = adjusted(resize ? "se" : "move", region, dx, dy);
    if (next.w >= 1 && next.h >= 1) region = next;
  }

  // Live selection: scroll the app under the cursor, not the overlay.
  function onWheel() {
    if (live && !dragStart && !edit) invoke("overlay_pass_scroll");
  }

  function onKeyDown(e: KeyboardEvent) {
    // Esc leaves the eyedropper first, then cancels.
    if (e.key === "Escape") {
      if (picking) picking = false;
      else finish(null);
    } else if (region && e.key === "Enter") {
      e.preventDefault();
      confirm(e.shiftKey);
    } else if (region && !edit && e.key.startsWith("Arrow")) {
      e.preventDefault();
      nudge(e.key, e.shiftKey);
    } else if (e.key.toLowerCase() === "c" && canPick && !dragStart && !done && !e.repeat) {
      picking = !picking;
    }
    else if (e.key === " " && mode !== "text") {
      e.preventDefault();
      // Held down, the key repeats, which would flip the mode back and forth.
      if (!e.repeat) toggleMode();
    }
  }

  // Magnifier for pixel-precise selection.
  $effect(() => {
    if (!loupe || !loaded || live || !showLoupe) return;
    const span = LOUPE / ZOOM;
    paintLoupe(img, (mouse.x - span / 2) * pxRatio, (mouse.y - span / 2) * pxRatio, span * pxRatio, span * pxRatio);
  });

  // Live selection has no still to magnify: keep fetching the screen under the loupe.
  async function liveLoupe() {
    const scratch = document.createElement("canvas");
    while (!done) {
      if (loupe && showLoupe) {
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

  // An adjustable selection at rest has no use for it, and it would hide the buttons.
  const showLoupe = $derived(selecting && mouse.x >= 0 && (!region || !!edit));

  const loupePos = $derived({
    x: mouse.x + 24 + LOUPE > viewport.w ? mouse.x - 24 - LOUPE : mouse.x + 24,
    y: mouse.y + 24 + LOUPE + 28 > viewport.h ? mouse.y - 24 - LOUPE - 28 : mouse.y + 24,
  });

  onMount(() => {
    const timer = setInterval(followCursor, 30);
    const unlisten = listen<{ from: number; mode: Mode }>("overlay:mode", (e) => {
      if (e.payload.from !== index && mode !== "text") setMode(e.payload.mode);
    });
    init();
    return () => {
      clearInterval(timer);
      unlisten.then((off) => off());
    };
  });

  async function init() {
    const info = await invoke<{
      mode: Mode | "screen" | "record" | "record_gif" | "scroll";
      width: number;
      adjust: boolean;
    } | null>("overlay_info", { index });
    if (!info) return finish(null);
    imageWidth = info.width;
    adjust = info.adjust;
    live = info.width === 0;
    if (info.mode === "window" || info.mode === "text") await setMode(info.mode);
    recording = info.mode === "record" || info.mode === "record_gif";
    scrolling = info.mode === "scroll";
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
  class:picking
  class:live
  style:cursor
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointerenter={onPointerEnter}
  ondblclick={onDoubleClick}
  onwheel={onWheel}
>
  {#if live === false}
    <img bind:this={img} {src} alt="" crossorigin="anonymous" draggable="false" onload={onImageLoad} onerror={() => finish(null)} />
  {/if}

  {#if box}
    <div class="box" style="left:{box.x}px; top:{box.y}px; width:{box.w}px; height:{box.h}px">
      <span class="size">{Math.round(box.w * pxRatio)} × {Math.round(box.h * pxRatio)}</span>
      {#if region}
        {#each ["nw", "n", "ne", "e", "se", "s", "sw", "w"] as part (part)}
          <i class="handle {part}"></i>
        {/each}
      {/if}
    </div>
    {#if region && toolsPos && !edit}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="tools"
        style="right:{toolsPos.right}px; top:{toolsPos.top}px"
        onpointerdown={(e) => e.stopPropagation()}
        ondblclick={(e) => e.stopPropagation()}
      >
        <button title={tr("Cancel")} aria-label={tr("Cancel")} onclick={() => finish(null)}>
          <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
        </button>
        <button class="ok" title={tr("Capture")} aria-label={tr("Capture")} onclick={(e) => confirm(e.shiftKey)}>
          <svg viewBox="0 0 16 16"><path d="M3 8.5l3.2 3L13 4.5" /></svg>
        </button>
      </div>
    {/if}
  {:else}
    <div class="dim"></div>
  {/if}

  {#if showLoupe}
    {#if !box && !picking}
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
    {#if region}
      {tr("Drag the edges or the area to adjust")} · <kbd>Enter</kbd> {tr("capture")}
      {#if mode === "text"}· <kbd>Shift</kbd><kbd>Enter</kbd> {tr("text only, skip QR codes")}{/if}
    {:else if picking}
      <strong>{tr("Color picker")}</strong> · {tr("Click to copy the color")} · <kbd>C</kbd> {tr("back to selecting")}
    {:else if mode === "text"}
      {tr("Drag over text to copy it")} · <kbd>Shift</kbd> {tr("text only, skip QR codes")}
    {:else}
      {#if recording}<strong>{tr("Record")}</strong> ·{/if}
      {#if scrolling}<strong>{tr("Scrolling capture")}</strong> · {tr("Select the part that scrolls")} ·{/if}
      {mode === "area" ? tr("Drag to select") : tr("Click a window")} · <kbd>Space</kbd>
      {mode === "area" ? tr("window mode") : tr("area mode")}
      {#if canPick}· <kbd>C</kbd> {tr("copy color")}{/if}
    {/if}
    {#if live}· {tr("Scroll works")}{/if}
    · <kbd>Esc</kbd> {tr("cancel")}
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
  /* An eyedropper, its tip on the pixel: white under a dark outline, so it
     shows on light and dark screens alike. */
  .stage.picking {
    cursor:
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cpath d='M19.6 4.4a2.6 2.6 0 0 0-3.7 0l-2.4 2.4-1-1-1.5 1.5 1 1-7.3 7.3-.9 3.6-1.3 1.3 1.5 1.5 1.3-1.3 3.6-.9 7.3-7.3 1 1 1.5-1.5-1-1 2.4-2.4a2.6 2.6 0 0 0 0-3.7z' fill='white' stroke='%23111' stroke-width='1.4' stroke-linejoin='round'/%3E%3C/svg%3E")
        2 22,
      crosshair;
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
  .handle {
    position: absolute;
    width: 8px;
    height: 8px;
    margin: -4px 0 0 -4px;
    background: #fff;
    border: 1px solid rgba(37, 99, 235, 0.9);
    border-radius: 2px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }
  .handle.nw,
  .handle.n,
  .handle.ne {
    top: 0;
  }
  .handle.w,
  .handle.e {
    top: 50%;
  }
  .handle.sw,
  .handle.s,
  .handle.se {
    top: 100%;
  }
  .handle.nw,
  .handle.w,
  .handle.sw {
    left: 0;
  }
  .handle.n,
  .handle.s {
    left: 50%;
  }
  .handle.ne,
  .handle.e,
  .handle.se {
    left: 100%;
  }
  .tools {
    position: absolute;
    display: flex;
    gap: 2px;
    padding: 2px;
    background: rgba(0, 0, 0, 0.72);
    border-radius: 6px;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.35);
    cursor: default;
  }
  .tools button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: #fff;
    cursor: pointer;
  }
  .tools button:hover {
    background: rgba(255, 255, 255, 0.16);
  }
  .tools button.ok {
    background: rgba(37, 99, 235, 0.95);
  }
  .tools button.ok:hover {
    background: rgb(59, 118, 245);
  }
  .tools svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
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
