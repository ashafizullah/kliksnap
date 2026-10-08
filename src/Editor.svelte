<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, imageUrl, invoke, isMac, loadImage, param, ready } from "./lib/api";
  import {
    COLORS,
    FONT_SIZES,
    STROKES,
    constrain,
    normalize,
    type Rect,
    type Scene,
    type Shape,
    type Tool,
  } from "./lib/editor/shapes";
  import { FONT, drawCropMask, drawScene, exportPixels } from "./lib/editor/render";

  const id = Number(param("id"));

  const TOOLS: { id: Tool; label: string; key: string; d: string }[] = [
    { id: "arrow", label: "Arrow", key: "a", d: "M4 14L14 4M8 4h6v6" },
    { id: "line", label: "Line", key: "l", d: "M4 14L14 4" },
    { id: "rect", label: "Rectangle", key: "r", d: "M3 4h12v10H3z" },
    { id: "ellipse", label: "Ellipse", key: "o", d: "M3 9a6 5 0 1 0 12 0a6 5 0 1 0-12 0" },
    { id: "text", label: "Text", key: "t", d: "M4 4h10M9 4v11" },
    { id: "highlight", label: "Highlight", key: "h", d: "M3 15h12M6 12l5-8 3 2-5 8H6z" },
    { id: "pixelate", label: "Pixelate", key: "p", d: "M3 3h4v4H3zM11 3h4v4h-4zM7 7h4v4H7zM3 11h4v4H3zM11 11h4v4h-4z" },
    { id: "crop", label: "Crop", key: "c", d: "M5 2v11h11M2 5h11v11" },
  ];
  const DRAG_TOOLS = new Set<Tool>(["arrow", "line", "rect", "ellipse", "highlight", "pixelate"]);
  const mod = isMac ? "⌘" : "Ctrl+";

  let base = $state.raw<HTMLImageElement | null>(null);
  let scale = $state(1);
  let canvas: HTMLCanvasElement;
  let stage: HTMLElement;
  let view = $state({ w: 0, h: 0 });

  let tool: Tool = $state("arrow");
  let color = $state(COLORS[0]);
  let sizeIndex = $state(1);

  let scene: Scene = $state.raw({ shapes: [], crop: null });
  let past: Scene[] = $state.raw([]);
  let future: Scene[] = $state.raw([]);
  let draft = $state.raw<Shape | null>(null);
  let cropDraft = $state.raw<Rect | null>(null);
  let dragFrom: { x: number; y: number } | null = null;

  let text = $state<{ x: number; y: number; value: string } | null>(null);
  let textArea = $state<HTMLTextAreaElement>();
  let status = $state("");
  let statusTimer: ReturnType<typeof setTimeout> | undefined;

  function flash(message: string) {
    status = message;
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => (status = ""), 2500);
  }

  function commit(next: Scene) {
    past = [...past, scene];
    future = [];
    scene = next;
  }

  function undo() {
    if (!past.length) return;
    future = [...future, scene];
    scene = past[past.length - 1];
    past = past.slice(0, -1);
  }

  function redo() {
    if (!future.length) return;
    past = [...past, scene];
    scene = future[future.length - 1];
    future = future.slice(0, -1);
  }

  function redraw() {
    if (!base || !canvas) return;
    const ctx = canvas.getContext("2d")!;
    drawScene(ctx, base, scene, scale, draft);
    const crop = cropDraft ?? scene.crop;
    if (crop) drawCropMask(ctx, crop, scale);
  }

  $effect(() => {
    void [scene, draft, cropDraft];
    redraw();
  });

  function fit() {
    if (!base) return;
    const natural = { w: base.naturalWidth / scale, h: base.naturalHeight / scale };
    const k = Math.min(1, (stage.clientWidth - 32) / natural.w, (stage.clientHeight - 32) / natural.h);
    view = { w: Math.round(natural.w * k), h: Math.round(natural.h * k) };
  }

  function toImage(e: PointerEvent) {
    const r = canvas.getBoundingClientRect();
    const x = ((e.clientX - r.left) * canvas.width) / r.width;
    const y = ((e.clientY - r.top) * canvas.height) / r.height;
    return {
      x: Math.max(0, Math.min(canvas.width, x)),
      y: Math.max(0, Math.min(canvas.height, y)),
    };
  }

  async function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !base) return;
    // Keep the browser from moving focus to <body>, which would blur the text box.
    e.preventDefault();
    if (text) commitText();
    const p = toImage(e);
    if (tool === "text") {
      text = { x: p.x, y: p.y, value: "" };
      await tick();
      textArea?.focus();
      return;
    }
    canvas.setPointerCapture(e.pointerId);
    dragFrom = p;
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragFrom) return;
    const p = toImage(e);
    const [x2, y2] = e.shiftKey ? constrain(tool, dragFrom.x, dragFrom.y, p.x, p.y) : [p.x, p.y];
    if (tool === "crop") {
      cropDraft = normalize(dragFrom.x, dragFrom.y, x2, y2);
    } else if (DRAG_TOOLS.has(tool)) {
      draft = {
        kind: tool as Exclude<Tool, "text" | "crop">,
        x1: dragFrom.x,
        y1: dragFrom.y,
        x2,
        y2,
        color: tool === "highlight" && color === "#ff3b30" ? "#ffcc00" : color,
        size: STROKES[sizeIndex],
      };
    }
  }

  function onPointerUp() {
    if (cropDraft && cropDraft.w > 4 && cropDraft.h > 4) {
      commit({ ...scene, crop: cropDraft });
    } else if (draft && draft.kind !== "text" && Math.hypot(draft.x2 - draft.x1, draft.y2 - draft.y1) > 3) {
      commit({ ...scene, shapes: [...scene.shapes, draft] });
    }
    dragFrom = null;
    draft = null;
    cropDraft = null;
  }

  function commitText() {
    if (!text) return;
    const value = text.value.replace(/\s+$/, "");
    if (value) {
      commit({
        ...scene,
        shapes: [...scene.shapes, { kind: "text", x: text.x, y: text.y, text: value, color, size: FONT_SIZES[sizeIndex] }],
      });
    }
    text = null;
  }

  function onTextKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      commitText();
    } else if (e.key === "Escape") {
      text = null;
    }
  }

  // Display ratio between CSS pixels and image pixels.
  const ratio = $derived(base && view.w ? view.w / base.naturalWidth : 1);
  const textFontPx = $derived(FONT_SIZES[sizeIndex] * scale * ratio);
  const textWidth = $derived.by(() => {
    if (!text) return 0;
    const ctx = document.createElement("canvas").getContext("2d")!;
    ctx.font = `600 ${textFontPx}px ${FONT}`;
    const widest = Math.max(...text.value.split("\n").map((l) => ctx.measureText(l).width));
    return Math.max(textFontPx * 2, widest + textFontPx);
  });

  /** Exports the annotated image and closes the editor, unless Save As is cancelled. */
  async function exportImage(action: "copy" | "save" | "saveas" | "savecopy") {
    if (!base) return;
    if (text) commitText();
    try {
      const { bytes, width, height } = exportPixels(base, scene, scale);
      const path = await invoke<string | null>("export_image", bytes, {
        headers: { "x-ks": JSON.stringify({ action, width, height }) },
      });
      if (action === "copy" || path) closeWindow();
    } catch (e) {
      flash(`Export failed: ${e}`);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    const cmd = isMac ? e.metaKey : e.ctrlKey;
    const key = e.key.toLowerCase();
    if (cmd) {
      const action: Record<string, () => void> = {
        z: () => (e.shiftKey ? redo() : undo()),
        y: redo,
        c: () => exportImage(e.shiftKey ? "savecopy" : "copy"),
        s: () => exportImage(e.shiftKey ? "saveas" : "save"),
        w: closeWindow,
      };
      if (action[key]) {
        e.preventDefault();
        action[key]();
      }
      return;
    }
    if (e.key === "Escape" && dragFrom) {
      dragFrom = null;
      draft = null;
      cropDraft = null;
      return;
    }
    const picked = TOOLS.find((t) => t.key === key);
    if (picked) tool = picked.id;
    else if (["1", "2", "3"].includes(key)) sizeIndex = Number(key) - 1;
  }

  onMount(() => {
    let observer: ResizeObserver | undefined;
    (async () => {
      const info = await invoke<{ width: number; height: number; scale: number } | null>("shot_info", { id });
      if (!info) return closeWindow();
      scale = info.scale || 1;
      base = await loadImage(imageUrl(`shot-${id}`));
      canvas.width = base.naturalWidth;
      canvas.height = base.naturalHeight;
      fit();
      redraw();
      observer = new ResizeObserver(fit);
      observer.observe(stage);
      await tick();
      ready();
    })().catch((e) => {
      flash(String(e));
      ready();
    });
    return () => observer?.disconnect();
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<div class="app">
  <header>
    <div class="group" role="toolbar" aria-label="Tools">
      {#each TOOLS as t (t.id)}
        <button
          class="icon"
          class:active={tool === t.id}
          title="{t.label} ({t.key.toUpperCase()})"
          aria-label={t.label}
          aria-pressed={tool === t.id}
          onclick={() => (tool = t.id)}
        >
          <svg viewBox="0 0 18 18" class:filled={t.id === "pixelate"}><path d={t.d} /></svg>
        </button>
      {/each}
    </div>

    <div class="group" role="radiogroup" aria-label="Color">
      {#each COLORS as c (c)}
        <button
          class="swatch"
          class:active={color === c}
          style="--c:{c}"
          title={c}
          aria-label="Color {c}"
          aria-pressed={color === c}
          onclick={() => (color = c)}
        ></button>
      {/each}
    </div>

    <div class="group" role="radiogroup" aria-label="Size">
      {#each STROKES as _, i (i)}
        <button
          class="icon"
          class:active={sizeIndex === i}
          title="Size {i + 1} ({i + 1})"
          aria-label="Size {i + 1}"
          aria-pressed={sizeIndex === i}
          onclick={() => (sizeIndex = i)}
        >
          <span class="dot" style="--d:{4 + i * 3}px"></span>
        </button>
      {/each}
    </div>

    <div class="group">
      <button class="icon" title="Undo ({mod}Z)" aria-label="Undo" disabled={!past.length} onclick={undo}>
        <svg viewBox="0 0 18 18"><path d="M5 8h7a3 3 0 010 6H9M5 8l3-3M5 8l3 3" /></svg>
      </button>
      <button class="icon" title="Redo ({mod}⇧Z)" aria-label="Redo" disabled={!future.length} onclick={redo}>
        <svg viewBox="0 0 18 18"><path d="M13 8H6a3 3 0 000 6h3M13 8l-3-3M13 8l-3 3" /></svg>
      </button>
    </div>

    <div class="spacer"></div>

    <div class="group actions">
      <button title="Save As… ({mod}⇧S)" onclick={() => exportImage("saveas")}>Save As…</button>
      <button title="Save ({mod}S)" onclick={() => exportImage("save")}>Save</button>
      <button title="Save & Copy ({mod}⇧C)" onclick={() => exportImage("savecopy")}>Save & Copy</button>
      <button class="primary" title="Copy ({mod}C)" onclick={() => exportImage("copy")}>Copy</button>
    </div>
  </header>

  <main bind:this={stage}>
    <div class="canvas-wrap" style="width:{view.w}px; height:{view.h}px">
      <canvas
        bind:this={canvas}
        class:text-tool={tool === "text"}
        style="width:{view.w}px; height:{view.h}px"
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
      ></canvas>
      {#if text}
        <textarea
          bind:this={textArea}
          bind:value={text.value}
          rows={text.value.split("\n").length}
          wrap="off"
          spellcheck="false"
          aria-label="Text"
          style="left:{text.x * ratio}px; top:{text.y * ratio}px; width:{textWidth}px; color:{color}; font:600 {textFontPx}px/1.25 {FONT}"
          onkeydown={onTextKey}
          onblur={commitText}
        ></textarea>
      {/if}
    </div>
    {#if status}
      <div class="status" role="status">{status}</div>
    {/if}
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    padding: 8px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  .group {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .spacer {
    flex: 1;
  }
  button {
    border: 0;
    background: transparent;
    border-radius: 6px;
    height: 30px;
  }
  button:hover:not(:disabled) {
    background: var(--hover);
  }
  button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .icon {
    width: 30px;
    display: grid;
    place-items: center;
    padding: 0;
  }
  .icon.active {
    background: var(--accent);
    color: var(--accent-text);
  }
  .icon svg {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .icon svg.filled {
    fill: currentColor;
    stroke: none;
  }
  .dot {
    width: var(--d);
    height: var(--d);
    border-radius: 50%;
    background: currentColor;
  }
  .swatch {
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    background: var(--c);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.2);
  }
  .swatch:hover:not(:disabled) {
    background: var(--c);
    transform: scale(1.1);
  }
  .swatch.active {
    box-shadow:
      0 0 0 2px var(--panel),
      0 0 0 4px var(--accent);
  }
  .actions button {
    padding: 0 12px;
    font-weight: 500;
  }
  .actions .primary {
    background: var(--accent);
    color: var(--accent-text);
  }
  .actions .primary:hover {
    background: var(--accent);
    filter: brightness(1.1);
  }
  main {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    background: var(--bg);
    background-image: radial-gradient(circle, var(--border) 1px, transparent 1px);
    background-size: 16px 16px;
  }
  .canvas-wrap {
    position: relative;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.18);
  }
  canvas {
    display: block;
    cursor: crosshair;
    touch-action: none;
  }
  canvas.text-tool {
    cursor: text;
  }
  textarea {
    position: absolute;
    margin: 0;
    padding: 0;
    border: 1px dashed var(--accent);
    outline: none;
    background: transparent;
    resize: none;
    overflow: hidden;
    white-space: pre;
  }
  .status {
    position: absolute;
    bottom: 14px;
    left: 50%;
    transform: translateX(-50%);
    max-width: calc(100% - 32px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 6px 12px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.8);
    color: #fff;
  }
</style>
