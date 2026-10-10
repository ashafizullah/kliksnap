<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, imageUrl, invoke, isLinux, isMac, loadImage, param, ready } from "./lib/api";
  import { tr } from "./lib/i18n";
  import {
    COLORS,
    DEFAULT_BACKDROP,
    FILLS,
    FONTS,
    FRAMES,
    PADDINGS,
    RADII,
    SIZES,
    constrain,
    fillCss,
    fontCss,
    nextStep,
    normalize,
    parseBackdrop,
    type Backdrop,
    type DragShape,
    type FillId,
    type FontId,
    type FrameId,
    type Rect,
    type Scene,
    type Shape,
    type Tool,
  } from "./lib/editor/shapes";
  import { drawCropMask, drawFrame, drawScene, exportPixels, frameInsets, frameScreenRadii } from "./lib/editor/render";

  const id = Number(param("id"));

  const TOOLS: { id: Tool; label: string; key: string; d: string }[] = [
    { id: "arrow", label: "Arrow", key: "a", d: "M4 14L14 4M8 4h6v6" },
    { id: "line", label: "Line", key: "l", d: "M4 14L14 4" },
    { id: "rect", label: "Rectangle", key: "r", d: "M3 4h12v10H3z" },
    { id: "ellipse", label: "Ellipse", key: "o", d: "M3 9a6 5 0 1 0 12 0a6 5 0 1 0-12 0" },
    { id: "pen", label: "Pen", key: "d", d: "M3 14c2-1 3-5 5-5s1 4 3 4 2-6 4-8" },
    { id: "text", label: "Text", key: "t", d: "M4 4h10M9 4v11" },
    { id: "step", label: "Number", key: "n", d: "M3 9a6 6 0 1 0 12 0a6 6 0 1 0-12 0M8 7l1.5-1v6" },
    { id: "highlight", label: "Highlight", key: "h", d: "M3 15h12M6 12l5-8 3 2-5 8H6z" },
    { id: "blur", label: "Blur", key: "b", d: "M9 2.5c3 3.5 5 6 5 8.5a5 5 0 01-10 0c0-2.5 2-5 5-8.5z" },
    { id: "pixelate", label: "Pixelate", key: "p", d: "M3 3h4v4H3zM11 3h4v4h-4zM7 7h4v4H7zM3 11h4v4H3zM11 11h4v4h-4z" },
    { id: "crop", label: "Crop", key: "c", d: "M5 2v11h11M2 5h11v11" },
  ];
  const DRAG_TOOLS = new Set<Tool>(["arrow", "line", "rect", "ellipse", "highlight", "blur", "pixelate"]);
  const mod = isMac ? "⌘" : "Ctrl+";

  let base = $state.raw<HTMLImageElement | null>(null);
  let scale = $state(1);
  let canvas: HTMLCanvasElement;
  let frameCanvas = $state<HTMLCanvasElement>();
  let stage: HTMLElement;
  let view = $state({ w: 0, h: 0 });

  let tool: Tool = $state("arrow");
  let color = $state(COLORS[0]);

  // Sizes and text style carry over to the next editor.
  const PREFS = "editor-prefs";
  const prefs = (() => {
    try {
      return JSON.parse(localStorage.getItem(PREFS) ?? "{}");
    } catch {
      return {};
    }
  })();
  let sizes = $state(
    Object.fromEntries(
      Object.entries(SIZES).map(([t, spec]) => {
        const saved = Number(prefs.sizes?.[t]);
        return [t, saved >= spec.min && saved <= spec.max ? saved : spec.presets[1]];
      }),
    ) as Record<Tool, number>,
  );
  let font = $state<FontId>(prefs.font in FONTS ? prefs.font : "sans");
  let bold = $state<boolean>(prefs.bold ?? true);
  const sizeSpec = $derived(SIZES[tool]);

  // The backdrop carries over too, on or off.
  let lastBackdrop = $state<Backdrop>(parseBackdrop(prefs.lastBackdrop) ?? DEFAULT_BACKDROP);
  let backdropPanel = $state(false);

  $effect(() => {
    const json = JSON.stringify({ sizes, font, bold, backdrop: scene.backdrop, lastBackdrop });
    try {
      localStorage.setItem(PREFS, json);
    } catch {
      // Private storage off: the defaults come back next time.
    }
  });

  function nudgeSize(dir: number) {
    const spec = SIZES[tool];
    if (!spec) return;
    const step = spec.max > 40 ? 2 : 1;
    sizes[tool] = Math.min(spec.max, Math.max(spec.min, sizes[tool] + dir * step));
  }

  let scene: Scene = $state.raw({ shapes: [], crop: null, backdrop: parseBackdrop(prefs.backdrop) });
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

  function clearAll() {
    text = null;
    if (scene.shapes.length || scene.crop) commit({ ...scene, shapes: [], crop: null });
  }

  let redacting = $state(false);

  /** Pixelates the email addresses, numbers, keys and passwords the OCR finds. */
  async function autoRedact() {
    if (redacting) return;
    redacting = true;
    try {
      const boxes = await invoke<[number, number, number, number][]>("find_sensitive", { id });
      if (!boxes.length) {
        flash(tr("Nothing sensitive found"));
        return;
      }
      const pad = 2 * scale;
      const shapes: Shape[] = boxes.map(([x, y, w, h]) => ({
        kind: "pixelate",
        x1: x - pad,
        y1: y - pad,
        x2: x + w + pad,
        y2: y + h + pad,
        color,
        size: sizes.pixelate,
      }));
      commit({ ...scene, shapes: [...scene.shapes, ...shapes] });
      flash(tr("Hid {n} item(s). Check the result, OCR can miss things", { n: boxes.length }));
    } catch (e) {
      flash(tr("Redact failed: {e}", { e: String(e) }));
    } finally {
      redacting = false;
    }
  }

  function setBackdrop(change: Partial<Backdrop> | null) {
    const next = change && { ...lastBackdrop, ...scene.backdrop, ...change };
    if (next) lastBackdrop = next;
    commit({ ...scene, backdrop: next });
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

  $effect(() => {
    void [scene.backdrop?.padding, scene.backdrop?.frame];
    fit();
  });

  // Backdrop preview in CSS pixels; the canvas itself keeps its coordinates.
  const preview = $derived.by(() => {
    const b = scene.backdrop;
    if (!b || !base) return null;
    const k = (view.w / base.naturalWidth) * scale;
    const pad = b.padding * k;
    const radius = b.radius * k;
    const framed = b.frame !== "none";
    return {
      pad,
      k,
      fill: fillCss(b.fill),
      frame: b.frame,
      inset: frameInsets(b.frame, view.w, k),
      radius,
      screenRadius: frameScreenRadii(b.frame, radius, view.w)
        .map((r) => `${r}px`)
        .join(" "),
      shadow: b.shadow && !framed ? `0 ${Math.max(2, pad * 0.12)}px ${Math.max(8, pad * 0.5)}px rgba(0, 0, 0, 0.35)` : "none",
      frameShadow: b.shadow ? { blur: Math.max(8, pad * 0.5), offsetY: Math.max(2, pad * 0.12) } : null,
    };
  });

  // The frame preview, drawn by the export's own code. It spills into the
  // padding so its shadow shows.
  $effect(() => {
    const p = preview;
    if (!p || p.frame === "none" || !frameCanvas) return;
    const dpr = devicePixelRatio;
    const { top, right, bottom, left } = p.inset;
    const cssW = view.w + left + right + 2 * p.pad;
    const cssH = view.h + top + bottom + 2 * p.pad;
    frameCanvas.width = Math.round(cssW * dpr);
    frameCanvas.height = Math.round(cssH * dpr);
    frameCanvas.style.width = `${cssW}px`;
    frameCanvas.style.height = `${cssH}px`;
    const ctx = frameCanvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, cssH);
    drawFrame(ctx, p.frame, p.pad + left, p.pad + top, view.w, view.h, p.k, p.radius, p.frameShadow);
  });

  function fit() {
    if (!base) return;
    const natural = { w: base.naturalWidth / scale, h: base.naturalHeight / scale };
    const pad = 2 * (scene.backdrop?.padding ?? 0);
    const inset = frameInsets(scene.backdrop?.frame ?? "none", natural.w, 1);
    const k = Math.min(
      1,
      (stage.clientWidth - 32) / (natural.w + pad + inset.left + inset.right),
      (stage.clientHeight - 32) / (natural.h + pad + inset.top + inset.bottom),
    );
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
    backdropPanel = false;
    if (text) commitText();
    const p = toImage(e);
    if (tool === "step") {
      const step = { kind: "step" as const, x: p.x, y: p.y, n: nextStep(scene.shapes), color, size: sizes.step };
      commit({ ...scene, shapes: [...scene.shapes, step] });
      return;
    }
    if (tool === "text") {
      text = { x: p.x, y: p.y, value: "" };
      await tick();
      textArea?.focus();
      return;
    }
    canvas.setPointerCapture(e.pointerId);
    dragFrom = p;
    if (tool === "pen") draft = { kind: "pen", points: [p.x, p.y], color, size: sizes.pen };
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragFrom) return;
    const p = toImage(e);
    if (draft?.kind === "pen") {
      const pts = draft.points;
      const minGap = 1.5 * scale;
      if (Math.hypot(p.x - pts[pts.length - 2], p.y - pts[pts.length - 1]) >= minGap) {
        draft = { ...draft, points: [...pts, p.x, p.y] };
      }
      return;
    }
    const [x2, y2] = e.shiftKey ? constrain(tool, dragFrom.x, dragFrom.y, p.x, p.y) : [p.x, p.y];
    if (tool === "crop") {
      cropDraft = normalize(dragFrom.x, dragFrom.y, x2, y2);
    } else if (DRAG_TOOLS.has(tool)) {
      draft = {
        kind: tool as DragShape["kind"],
        x1: dragFrom.x,
        y1: dragFrom.y,
        x2,
        y2,
        color: tool === "highlight" && color === "#ff3b30" ? "#ffcc00" : color,
        size: sizes[tool],
      };
    }
  }

  function onPointerUp() {
    if (cropDraft && cropDraft.w > 4 && cropDraft.h > 4) {
      commit({ ...scene, crop: cropDraft });
    } else if (draft && "x2" in draft && Math.hypot(draft.x2 - draft.x1, draft.y2 - draft.y1) > 3) {
      commit({ ...scene, shapes: [...scene.shapes, draft] });
    } else if (draft?.kind === "pen") {
      // A click without moving leaves a dot.
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
        shapes: [...scene.shapes, { kind: "text", x: text.x, y: text.y, text: value, color, size: sizes.text, font, bold }],
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
  const textFontPx = $derived(sizes.text * scale * ratio);
  const textWidth = $derived.by(() => {
    if (!text) return 0;
    const ctx = document.createElement("canvas").getContext("2d")!;
    ctx.font = fontCss(font, bold, textFontPx);
    const widest = Math.max(...text.value.split("\n").map((l) => ctx.measureText(l).width));
    return Math.max(textFontPx * 2, widest + textFontPx);
  });

  /** Exports the annotated image and closes the editor, unless Save As is cancelled. */
  let shareButton = $state<HTMLButtonElement>();

  async function exportImage(action: "copy" | "save" | "saveas" | "savecopy" | "pin" | "share") {
    if (!base) return;
    if (text) commitText();
    try {
      const { bytes, width, height } = exportPixels(base, scene, scale);
      const r = shareButton?.getBoundingClientRect();
      const anchor = action === "share" && r ? [r.left, r.top, r.width, r.height] : null;
      const path = await invoke<string | null>("export_image", bytes, {
        headers: { "x-ks": JSON.stringify({ action, width, height, anchor }) },
      });
      if (action === "copy" || action === "pin" || path) closeWindow();
    } catch (e) {
      flash(tr("Export failed: {e}", { e: String(e) }));
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
        p: () => exportImage("pin"),
        w: closeWindow,
        backspace: clearAll,
      };
      if (action[key]) {
        e.preventDefault();
        action[key]();
      }
      return;
    }
    if (e.key === "Escape" && backdropPanel) {
      backdropPanel = false;
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
    else if (["1", "2", "3"].includes(key) && sizeSpec) sizes[tool] = sizeSpec.presets[Number(key) - 1];
    else if (key === "[" || key === "]") nudgeSize(key === "]" ? 1 : -1);
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
    <div class="group" role="toolbar" aria-label={tr("Tools")}>
      {#each TOOLS as t (t.id)}
        <button
          class="icon"
          class:active={tool === t.id}
          title="{tr(t.label)} ({t.key.toUpperCase()})"
          aria-label={tr(t.label)}
          aria-pressed={tool === t.id}
          onclick={() => (tool = t.id)}
        >
          <svg viewBox="0 0 18 18" class:filled={t.id === "pixelate"}><path d={t.d} /></svg>
        </button>
      {/each}
    </div>

    <div class="group" role="radiogroup" aria-label={tr("Color")}>
      {#each COLORS as c (c)}
        <button
          class="swatch"
          class:active={color === c}
          style="--c:{c}"
          title={c}
          aria-label={tr("Color {c}", { c })}
          aria-pressed={color === c}
          onclick={() => (color = c)}
        ></button>
      {/each}
    </div>

    <div class="group">
      <button class="icon" title="{tr('Undo')} ({mod}Z)" aria-label={tr("Undo")} disabled={!past.length} onclick={undo}>
        <svg viewBox="0 0 18 18"><path d="M5 8h7a3 3 0 010 6H9M5 8l3-3M5 8l3 3" /></svg>
      </button>
      <button class="icon" title="{tr('Redo')} ({mod}⇧Z)" aria-label={tr("Redo")} disabled={!future.length} onclick={redo}>
        <svg viewBox="0 0 18 18"><path d="M13 8H6a3 3 0 000 6h3M13 8l-3-3M13 8l-3 3" /></svg>
      </button>
      <button
        class="icon"
        title="{tr('Clear All')} ({mod}⌫)"
        aria-label={tr("Clear all")}
        disabled={!scene.shapes.length && !scene.crop}
        onclick={clearAll}
      >
        <svg viewBox="0 0 18 18"><path d="M4 5h10M7.5 5V3.5h3V5M5.5 5l.7 9.5h5.6l.7-9.5" /></svg>
      </button>
    </div>

    <div class="group size" class:off={!sizeSpec}>
      <label
        title={sizeSpec
          ? tr("{label} (1 2 3, [ ]; double-click to reset)", { label: tr(sizeSpec.label) })
          : tr("This tool has no size")}
        ondblclick={() => sizeSpec && (sizes[tool] = sizeSpec.presets[1])}
      >
        <span class="muted">{tr(sizeSpec?.label ?? "Size")}</span>
        {#if sizeSpec}
          <input type="range" min={sizeSpec.min} max={sizeSpec.max} step="1" bind:value={sizes[tool]} />
          <span class="value">{sizes[tool]}</span>
        {:else}
          <input type="range" disabled />
          <span class="value">–</span>
        {/if}
      </label>
    </div>

    <div class="group">
      <button
        class="icon"
        title={tr("Hide emails, numbers, keys and passwords (OCR)")}
        aria-label={tr("Auto redact")}
        disabled={redacting}
        onclick={autoRedact}
      >
        <svg viewBox="0 0 18 18"><path d="M2 9s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5zM3 15L15 3" /></svg>
      </button>
    </div>

    <div class="group backdrop-group">
      <button
        class="icon"
        class:active={!!scene.backdrop}
        title={tr("Background")}
        aria-label={tr("Background")}
        aria-expanded={backdropPanel}
        onclick={() => (backdropPanel = !backdropPanel)}
      >
        <svg viewBox="0 0 18 18"><path d="M2.5 2.5h13v13h-13zM6 6h6v6H6z" /></svg>
      </button>
      {#if backdropPanel}
        <div class="panel" role="dialog" aria-label={tr("Background")}>
          <div class="row fills">
            <button class="fill off" class:active={!scene.backdrop} title={tr("No background")} onclick={() => setBackdrop(null)}>{tr("Off")}</button>
            {#each Object.entries(FILLS) as [fid, f] (fid)}
              <button
                class="fill"
                class:active={scene.backdrop?.fill === fid}
                class:clear={fid === "clear"}
                style="--fill:{fillCss(fid as FillId)}"
                title={tr(f.label)}
                aria-label={tr("{label} background", { label: tr(f.label) })}
                onclick={() => setBackdrop({ fill: fid as FillId })}
              ></button>
            {/each}
          </div>
          <div class="row">
            <span class="muted">{tr("Padding")}</span>
            {#each PADDINGS as p (p.value)}
              <button
                class="chip"
                class:active={(scene.backdrop ?? lastBackdrop).padding === p.value}
                onclick={() => setBackdrop({ padding: p.value })}>{p.label}</button
              >
            {/each}
          </div>
          <div class="row">
            <span class="muted">{tr("Corners")}</span>
            {#each RADII as r (r.value)}
              <button
                class="chip"
                class:active={(scene.backdrop ?? lastBackdrop).radius === r.value}
                onclick={() => setBackdrop({ radius: r.value })}>{tr(r.label)}</button
              >
            {/each}
          </div>
          <div class="row">
            <span class="muted">{tr("Frame")}</span>
            {#each FRAMES as f (f.id)}
              <button
                class="chip"
                class:active={(scene.backdrop ?? lastBackdrop).frame === f.id}
                onclick={() => setBackdrop({ frame: f.id as FrameId })}>{tr(f.label)}</button
              >
            {/each}
          </div>
          <label class="row">
            <input
              type="checkbox"
              checked={(scene.backdrop ?? lastBackdrop).shadow}
              onchange={(e) => setBackdrop({ shadow: e.currentTarget.checked })}
            />
            <span>{tr("Shadow")}</span>
          </label>
        </div>
      {/if}
    </div>

    {#if tool === "text"}
      <div class="group">
        <select aria-label={tr("Font")} bind:value={font}>
          {#each Object.entries(FONTS) as [fid, f] (fid)}
            <option value={fid}>{f.label}</option>
          {/each}
        </select>
        <button class="icon" class:active={bold} title={tr("Bold")} aria-label={tr("Bold")} aria-pressed={bold} onclick={() => (bold = !bold)}>
          <b>B</b>
        </button>
      </div>
    {/if}

    <div class="spacer"></div>

    <div class="group actions">
      <!-- Linux has no system share sheet. -->
      {#if !isLinux}
        <button
          bind:this={shareButton}
          title={isMac ? tr("Share (AirDrop, Messages, Mail…)") : tr("Share")}
          onclick={() => exportImage("share")}
        >
          {tr("Share")}
        </button>
      {/if}
      <button title="{tr('Keep on screen')} ({mod}P)" onclick={() => exportImage("pin")}>{tr("Pin")}</button>
      <button title="{tr('Save As…')} ({mod}⇧S)" onclick={() => exportImage("saveas")}>{tr("Save As…")}</button>
      <button title="{tr('Save')} ({mod}S)" onclick={() => exportImage("save")}>{tr("Save")}</button>
      <button title="{tr('Save & Copy')} ({mod}⇧C)" onclick={() => exportImage("savecopy")}>{tr("Save & Copy")}</button>
      <button class="primary" title="{tr('Copy')} ({mod}C)" onclick={() => exportImage("copy")}>{tr("Copy")}</button>
    </div>
  </header>

  <main bind:this={stage}>
    <div
      class="backdrop"
      class:clear={scene.backdrop?.fill === "clear"}
      style:padding="{preview?.pad ?? 0}px"
      style:background={scene.backdrop?.fill === "clear" ? undefined : preview?.fill}
    >
      <div
        class="device"
        style:padding={preview ? `${preview.inset.top}px ${preview.inset.right}px ${preview.inset.bottom}px ${preview.inset.left}px` : "0"}
      >
      {#if preview && preview.frame !== "none"}
        <canvas bind:this={frameCanvas} class="frame" style:left="{-preview.pad}px" style:top="{-preview.pad}px"></canvas>
      {/if}
      <div
        class="canvas-wrap"
        style="width:{view.w}px; height:{view.h}px"
        style:border-radius={preview?.screenRadius ?? "0"}
        style:box-shadow={preview?.shadow}
      >
        <canvas
          bind:this={canvas}
          class:text-tool={tool === "text"}
          style="width:{view.w}px; height:{view.h}px"
          style:border-radius={preview?.screenRadius ?? "0"}
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
            aria-label={tr("Text")}
            style="left:{text.x * ratio}px; top:{text.y * ratio}px; width:{textWidth}px; color:{color}; font:{fontCss(font, bold, textFontPx)}; line-height:1.25"
            onkeydown={onTextKey}
            onblur={commitText}
          ></textarea>
        {/if}
      </div>
      </div>
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
  .size label {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .size.off {
    opacity: 0.4;
  }
  .muted {
    color: var(--muted);
    font-size: 12px;
  }
  .size input {
    width: 96px;
    accent-color: var(--accent);
  }
  .value {
    min-width: 2ch;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  select {
    height: 28px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    color: inherit;
    padding: 0 6px;
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
  .backdrop-group {
    position: relative;
  }
  .panel {
    position: absolute;
    top: 36px;
    left: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    width: max-content;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .row .muted {
    min-width: 56px;
  }
  .fill {
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: 6px;
    background: var(--fill);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
  }
  .fill:hover:not(:disabled) {
    background: var(--fill);
    transform: scale(1.08);
  }
  .fill.off {
    width: auto;
    padding: 0 8px;
    font-size: 12px;
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .fill.clear,
  .fill.clear:hover:not(:disabled),
  .backdrop.clear {
    background-image: repeating-conic-gradient(#ccc 0 25%, #fff 0 50%);
    background-size: 10px 10px;
  }
  .fill.active {
    box-shadow:
      0 0 0 2px var(--panel),
      0 0 0 4px var(--accent);
  }
  .chip {
    height: 26px;
    min-width: 34px;
    padding: 0 8px;
    font-size: 12px;
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .chip.active {
    background: var(--accent);
    color: var(--accent-text);
    box-shadow: none;
  }
  .chip.active:hover {
    background: var(--accent);
  }
  .device {
    position: relative;
  }
  canvas.frame {
    position: absolute;
    pointer-events: none;
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
