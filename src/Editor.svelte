<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, imageUrl, invoke, isLinux, isMac, loadImage, param, ready } from "./lib/api";
  import { tr } from "./lib/i18n";
  import {
    COLORS,
    DEFAULT_BACKDROP,
    FILLS,
    FLIP_H,
    FLIP_V,
    FONTS,
    FRAMES,
    PADDINGS,
    RADII,
    ROTATE_LEFT,
    ROTATE_RIGHT,
    SIZES,
    UPRIGHT,
    badgeText,
    compose,
    constrain,
    fillCss,
    fontCss,
    moveShape,
    nextStep,
    normalize,
    parseBackdrop,
    textPad,
    turnRect,
    type Backdrop,
    type DragShape,
    type FillId,
    type FontId,
    type FrameId,
    type Orient,
    type Rect,
    type Scene,
    type Shape,
    type TextShape,
    type Tool,
  } from "./lib/editor/shapes";
  import {
    drawCropMask,
    drawFrame,
    drawScene,
    canFlip,
    drawSelection,
    flipShape,
    handleAt,
    rotateShape,
    selectionExtent,
    shapeCenter,
    exportPixels,
    frameInsets,
    frameScreenRadii,
    hitShape,
    turnImage,
    turnShape,
  } from "./lib/editor/render";

  const id = Number(param("id"));

  type ToolGroup = "pick" | "draw" | "hide" | "image";
  const TOOLS: { id: Tool; group: ToolGroup; label: string; key: string; d: string }[] = [
    { id: "select", group: "pick", label: "Select", key: "v", d: "M5 2.5v12l3.2-3 2 4.3 1.8-.8-2-4.3h4.3z" },
    { id: "arrow", group: "draw", label: "Arrow", key: "a", d: "M4 14L14 4M8 4h6v6" },
    { id: "line", group: "draw", label: "Line", key: "l", d: "M4 14L14 4" },
    { id: "rect", group: "draw", label: "Rectangle", key: "r", d: "M3 4h12v10H3z" },
    { id: "ellipse", group: "draw", label: "Ellipse", key: "o", d: "M3 9a6 5 0 1 0 12 0a6 5 0 1 0-12 0" },
    { id: "pen", group: "draw", label: "Pen", key: "d", d: "M3 14c2-1 3-5 5-5s1 4 3 4 2-6 4-8" },
    { id: "text", group: "draw", label: "Text", key: "t", d: "M4 4h10M9 4v11" },
    { id: "step", group: "draw", label: "Number", key: "n", d: "M3 9a6 6 0 1 0 12 0a6 6 0 1 0-12 0M8 7l1.5-1v6" },
    { id: "highlight", group: "hide", label: "Highlight", key: "h", d: "M3 15h12M6 12l5-8 3 2-5 8H6z" },
    { id: "blur", group: "hide", label: "Blur", key: "b", d: "M9 2.5c3 3.5 5 6 5 8.5a5 5 0 01-10 0c0-2.5 2-5 5-8.5z" },
    { id: "pixelate", group: "hide", label: "Pixelate", key: "p", d: "M3 3h4v4H3zM11 3h4v4h-4zM7 7h4v4H7zM3 11h4v4H3zM11 11h4v4h-4z" },
    { id: "crop", group: "image", label: "Crop", key: "c", d: "M5 2v11h11M2 5h11v11" },
  ];
  // Areas cover what is under them, so the drawing tools only move one that is picked.
  const AREAS = new Set<Shape["kind"]>(["highlight", "blur", "pixelate"]);
  const TRASH = "M4 5h10M7.5 5V3.5h3V5M5.5 5l.7 9.5h5.6l.7-9.5";
  const DRAG_TOOLS = new Set<Tool>(["arrow", "line", "rect", "ellipse", "highlight", "blur", "pixelate"]);
  const mod = isMac ? "⌘" : "Ctrl+";
  const TURNS: { op: Orient; label: string; d: string }[] = [
    { op: ROTATE_LEFT, label: "Rotate left", d: "M1.5 3v4.5H6M2.9 11.3a6.5 6.5 0 1 0 1.5-6.8L1.5 7.5" },
    { op: ROTATE_RIGHT, label: "Rotate right", d: "M16.5 3v4.5H12M15.1 11.3a6.5 6.5 0 1 1-1.5-6.8l2.9 3" },
    { op: FLIP_H, label: "Flip horizontal", d: "M9 2v14M7 5l-5 8h5zM11 5l5 8h-5z" },
    { op: FLIP_V, label: "Flip vertical", d: "M2 9h14M5 7l8-5v5zM5 11l8 5v-5z" },
  ];

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
  let textBg = $state<boolean>(prefs.textBg ?? false);
  // The color last picked outside the presets.
  let custom = $state<string>(/^#[0-9a-f]{6}$/i.test(prefs.custom) ? prefs.custom : "#af52de");

  // The backdrop carries over too, on or off.
  let lastBackdrop = $state<Backdrop>(parseBackdrop(prefs.lastBackdrop) ?? DEFAULT_BACKDROP);
  let backdropPanel = $state(false);

  $effect(() => {
    const json = JSON.stringify({ sizes, font, bold, textBg, custom, backdrop: scene.backdrop, lastBackdrop });
    try {
      localStorage.setItem(PREFS, json);
    } catch {
      // Private storage off: the defaults come back next time.
    }
  });

  function nudgeSize(dir: number) {
    if (sizeSpec) setSize(sizeValue + dir * (sizeSpec.max > 40 ? 2 : 1));
  }

  let scene: Scene = $state.raw({ shapes: [], crop: null, backdrop: parseBackdrop(prefs.backdrop), orient: UPRIGHT });
  // The screenshot as turned; only rebuilt when the orientation changes.
  const orient = $derived(scene.orient);
  const image = $derived(base ? turnImage(base, orient) : null);
  let past: Scene[] = $state.raw([]);
  let future: Scene[] = $state.raw([]);
  let draft = $state.raw<Shape | null>(null);
  let cropDraft = $state.raw<Rect | null>(null);
  let dragFrom: { x: number; y: number } | null = null;

  // The picked shape, and the scene while it is being dragged or turned. A
  // shape is picked when it is added, or by pressing on it.
  let selected = $state<number | null>(null);
  const sel = $derived(selected !== null && selected < scene.shapes.length ? selected : null);
  let live = $state.raw<Scene | null>(null);
  const selShape = $derived(sel !== null ? (live ?? scene).shapes[sel] : null);
  let moving: { index: number; from: { x: number; y: number } } | null = null;
  let rotating: { index: number; cx: number; cy: number; from: number } | null = null;
  let hover = $state<"move" | "rotate" | null>(null);

  $effect(() => {
    void tool;
    selected = null;
  });

  // The size and text controls show the picked shape, else the tool.
  const sizeKind = $derived<Tool>(selShape ? selShape.kind : tool);
  const sizeSpec = $derived(SIZES[sizeKind]);
  const sizeValue = $derived(selShape && SIZES[selShape.kind] ? selShape.size : sizes[sizeKind]);
  const shownColor = $derived(selShape?.color ?? color);
  const customShown = $derived(!COLORS.includes(shownColor));
  const textSel = $derived<TextShape | null>(selShape?.kind === "text" ? selShape : null);

  /** Changes the picked shape; repeats under the same `key` undo as one. */
  function editSelected(key: string, change: (s: Shape) => Shape) {
    if (sel === null) return;
    const index = sel;
    const next = { ...scene, shapes: scene.shapes.map((s, i) => (i === index ? change(s) : s)) };
    const id = `${key}:${index}`;
    if (amending === id) {
      scene = next;
      return;
    }
    commit(next);
    amending = id;
  }

  function setColor(c: string) {
    color = c;
    editSelected("color", (s) => ({ ...s, color: c }));
  }

  function setSize(value: number) {
    if (!sizeSpec) return;
    const v = Math.min(sizeSpec.max, Math.max(sizeSpec.min, value));
    sizes[sizeKind] = v;
    editSelected("size", (s) => ({ ...s, size: v }));
  }

  function setText(change: { font?: FontId; bold?: boolean; bg?: boolean }) {
    if (change.font) font = change.font;
    if (change.bold !== undefined) bold = change.bold;
    if (change.bg !== undefined) textBg = change.bg;
    editSelected("text", (s) => (s.kind === "text" ? { ...s, ...change } : s));
  }

  /** The shape a press at `p` would move, as its index: the topmost one there. */
  function grabAt(p: { x: number; y: number }) {
    // Strokes and crops start anywhere, also on a shape.
    if (tool === "pen" || tool === "crop") return null;
    const tol = 5 / ratio;
    for (let i = scene.shapes.length - 1; i >= 0; i--) {
      const s = scene.shapes[i];
      if (tool !== "select" && i !== sel && AREAS.has(s.kind)) continue;
      if (hitShape(s, p.x, p.y, scale, tol)) return i;
    }
    return null;
  }

  /** Whether `p` is on the rotate handle of the picked shape. */
  function onHandle(p: { x: number; y: number }) {
    if (sel === null) return false;
    const h = handleAt(scene.shapes[sel], scale, 1 / ratio, canvas.width, canvas.height);
    return !!h && Math.hypot(p.x - h[0], p.y - h[1]) <= 9 / ratio;
  }

  function moveSelected(dx: number, dy: number) {
    if (sel === null) return;
    const index = sel;
    commit({ ...scene, shapes: scene.shapes.map((s, i) => (i === index ? moveShape(s, dx, dy) : s)) });
  }

  function flipSelected(horizontal: boolean) {
    if (sel === null) return;
    const index = sel;
    commit({ ...scene, shapes: scene.shapes.map((s, i) => (i === index ? flipShape(s, horizontal, scale) : s)) });
  }

  function removeSelected() {
    if (sel === null) return;
    const index = sel;
    // The badges left keep counting from 1.
    let n = 0;
    const shapes = scene.shapes.filter((_, i) => i !== index).map((s) => (s.kind === "step" ? { ...s, n: ++n } : s));
    selected = null;
    commit({ ...scene, shapes });
  }

  let text = $state<{ x: number; y: number; value: string } | null>(null);
  let textArea = $state<HTMLTextAreaElement>();
  let status = $state("");
  let statusTimer: ReturnType<typeof setTimeout> | undefined;

  function flash(message: string) {
    status = message;
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => (status = ""), 2500);
  }

  // Dragging a slider changes one thing many times: that is one step to undo.
  let amending: string | null = null;

  function commit(next: Scene) {
    amending = null;
    past = [...past, scene];
    future = [];
    scene = next;
  }

  function undo() {
    if (!past.length) return;
    amending = null;
    selected = null;
    future = [...future, scene];
    scene = past[past.length - 1];
    past = past.slice(0, -1);
  }

  /** Rotates or mirrors the screenshot, taking the annotations and crop along. */
  function turn(op: Orient) {
    if (!image) return;
    if (text) commitText();
    const { width: w, height: h } = image;
    commit({
      ...scene,
      orient: compose(op, scene.orient),
      shapes: scene.shapes.map((s) => turnShape(s, op, w, h, scale)),
      crop: scene.crop && turnRect(op, w, h, scene.crop),
    });
  }

  function clearAll() {
    text = null;
    selected = null;
    if (scene.shapes.length || scene.crop) commit({ ...scene, shapes: [], crop: null });
  }

  let redacting = $state(false);

  /** Pixelates the email addresses, numbers, keys and passwords the OCR finds. */
  async function autoRedact() {
    if (redacting || !base) return;
    redacting = true;
    try {
      const boxes = await invoke<[number, number, number, number][]>("find_sensitive", { id });
      if (!boxes.length) {
        flash(tr("Nothing sensitive found"));
        return;
      }
      const pad = 2 * scale;
      // The OCR reads the original screenshot, which may since have been turned.
      const { naturalWidth: bw, naturalHeight: bh } = base;
      const shapes: Shape[] = boxes.map(([x, y, w, h]) => {
        const r = turnRect(scene.orient, bw, bh, { x, y, w, h });
        return {
          kind: "pixelate",
          x1: r.x - pad,
          y1: r.y - pad,
          x2: r.x + r.w + pad,
          y2: r.y + r.h + pad,
          color,
          size: sizes.pixelate,
        };
      });
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
    amending = null;
    selected = null;
    past = [...past, scene];
    scene = future[future.length - 1];
    future = future.slice(0, -1);
  }

  function redraw() {
    if (!image || !canvas) return;
    if (canvas.width !== image.width || canvas.height !== image.height) {
      canvas.width = image.width;
      canvas.height = image.height;
    }
    const ctx = canvas.getContext("2d")!;
    const shown = live ?? scene;
    drawScene(ctx, image, shown, scale, draft);
    const crop = cropDraft ?? scene.crop;
    if (crop) drawCropMask(ctx, crop, scale);
    if (sel !== null) {
      const s = shown.shapes[sel];
      const unit = 1 / ratio;
      drawSelection(ctx, s, scale, unit, handleAt(s, scale, unit, canvas.width, canvas.height));
    }
  }

  $effect(() => {
    void [scene, draft, cropDraft, live, sel];
    redraw();
  });

  $effect(() => {
    void [scene.backdrop?.padding, scene.backdrop?.frame, image];
    fit();
  });

  // Backdrop preview in CSS pixels; the canvas itself keeps its coordinates.
  const preview = $derived.by(() => {
    const b = scene.backdrop;
    if (!b || !image) return null;
    const k = (view.w / image.width) * scale;
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
    if (!image) return;
    const natural = { w: image.width / scale, h: image.height / scale };
    const pad = 2 * (scene.backdrop?.padding ?? 0);
    const inset = frameInsets(scene.backdrop?.frame ?? "none", natural.w, 1);
    const k = Math.min(
      1,
      (stage.clientWidth - 32) / (natural.w + pad + inset.left + inset.right),
      (stage.clientHeight - 32) / (natural.h + pad + inset.top + inset.bottom),
    );
    view = { w: Math.round(natural.w * k), h: Math.round(natural.h * k) };
  }

  function toImage(e: PointerEvent, clamp = true) {
    const r = canvas.getBoundingClientRect();
    const x = ((e.clientX - r.left) * canvas.width) / r.width;
    const y = ((e.clientY - r.top) * canvas.height) / r.height;
    if (!clamp) return { x, y };
    return {
      x: Math.max(0, Math.min(canvas.width, x)),
      y: Math.max(0, Math.min(canvas.height, y)),
    };
  }

  async function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !image) return;
    // Keep the browser from moving focus to <body>, which would blur the text box.
    e.preventDefault();
    backdropPanel = false;
    if (text) commitText();
    const p = toImage(e);
    if (sel !== null && onHandle(p)) {
      const [cx, cy] = shapeCenter(scene.shapes[sel], scale);
      canvas.setPointerCapture(e.pointerId);
      rotating = { index: sel, cx, cy, from: Math.atan2(p.y - cy, p.x - cx) };
      return;
    }
    const grabbed = grabAt(p);
    selected = grabbed;
    if (tool === "select" || grabbed !== null) {
      if (grabbed !== null) {
        canvas.setPointerCapture(e.pointerId);
        moving = { index: grabbed, from: p };
      }
      return;
    }
    if (tool === "step") {
      const step = { kind: "step" as const, x: p.x, y: p.y, n: nextStep(scene.shapes), color, size: sizes.step };
      commit({ ...scene, shapes: [...scene.shapes, step] });
      selected = scene.shapes.length - 1;
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
    if (moving) {
      const { index, from } = moving;
      const at = toImage(e);
      const [dx, dy] = [at.x - from.x, at.y - from.y];
      // A click that wobbles is still a click.
      if (!live && Math.hypot(dx, dy) < 3 / ratio) return;
      live = { ...scene, shapes: scene.shapes.map((s, i) => (i === index ? moveShape(s, dx, dy) : s)) };
      return;
    }
    if (rotating) {
      const { index, cx, cy, from } = rotating;
      const at = toImage(e, false);
      let angle = Math.atan2(at.y - cy, at.x - cx) - from;
      // Shift turns in steps of 15°.
      if (e.shiftKey) angle = Math.round(angle / (Math.PI / 12)) * (Math.PI / 12);
      live = { ...scene, shapes: scene.shapes.map((s, i) => (i === index ? rotateShape(s, angle, scale) : s)) };
      return;
    }
    if (dragFrom) hover = null;
    else {
      const at = toImage(e);
      hover = onHandle(at) ? "rotate" : grabAt(at) !== null ? "move" : null;
    }
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
      // Shift's square can reach past the image; the crop is what lies on it.
      const [cx, cy] = [Math.max(0, Math.min(canvas.width, x2)), Math.max(0, Math.min(canvas.height, y2))];
      cropDraft = normalize(dragFrom.x, dragFrom.y, cx, cy);
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
    if (moving || rotating) {
      if (live) commit(live);
      live = null;
      moving = null;
      rotating = null;
      return;
    }
    if (cropDraft && cropDraft.w > 4 && cropDraft.h > 4) {
      commit({ ...scene, crop: cropDraft });
    } else if (draft && "x2" in draft && Math.hypot(draft.x2 - draft.x1, draft.y2 - draft.y1) > 3) {
      commit({ ...scene, shapes: [...scene.shapes, draft] });
      selected = scene.shapes.length - 1;
    } else if (draft?.kind === "pen") {
      // Strokes are not picked: the next one often starts on the last.
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
        shapes: [...scene.shapes, { kind: "text", x: text.x, y: text.y, text: value, color, size: sizes.text, font, bold, bg: textBg }],
      });
      selected = scene.shapes.length - 1;
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
  const ratio = $derived(image && view.w ? view.w / image.width : 1);
  const textFontPx = $derived(sizes.text * scale * ratio);

  // The picked shape's own buttons, in CSS pixels: on the side its rotate handle is not.
  const objectBar = $derived.by(() => {
    if (!selShape || live || !image) return null;
    const unit = 1 / ratio;
    const e = selectionExtent(selShape, scale, unit);
    const h = handleAt(selShape, scale, unit, image.width, image.height);
    const y = !h || h[1] <= e.cy ? e.maxY * ratio + 8 : e.minY * ratio - 38;
    return {
      x: Math.min(Math.max(e.cx * ratio, 48), Math.max(48, view.w - 48)),
      y: Math.min(Math.max(y, 4), Math.max(4, view.h - 34)),
      flip: canFlip(selShape),
    };
  });
  const textWidth = $derived.by(() => {
    if (!text) return 0;
    const ctx = document.createElement("canvas").getContext("2d")!;
    ctx.font = fontCss(font, bold, textFontPx);
    const widest = Math.max(...text.value.split("\n").map((l) => ctx.measureText(l).width));
    return Math.max(textFontPx * 2, widest + textFontPx);
  });

  /** Exports the annotated image and closes the editor, unless Save As is cancelled. */
  let shareButton = $state<HTMLButtonElement>();

  async function exportImage(action: "copy" | "save" | "saveas" | "savecopy" | "pin" | "share" | "explain") {
    if (!image) return;
    if (text) commitText();
    try {
      const { bytes, width, height } = exportPixels(image, scene, scale);
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
    if (e.key === "Escape" && (moving || rotating)) {
      live = null;
      moving = null;
      rotating = null;
      return;
    }
    if (sel !== null && !moving && !rotating) {
      const field = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
      const step = (e.shiftKey ? 10 : 1) * scale;
      const nudge: Record<string, [number, number]> = {
        ArrowLeft: [-step, 0],
        ArrowRight: [step, 0],
        ArrowUp: [0, -step],
        ArrowDown: [0, step],
      };
      if (e.key === "Escape") {
        selected = null;
        return;
      }
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        removeSelected();
        return;
      }
      if (nudge[e.key] && !field) {
        e.preventDefault();
        moveSelected(...nudge[e.key]);
        return;
      }
    }
    if (e.key === "Escape" && dragFrom) {
      dragFrom = null;
      draft = null;
      cropDraft = null;
      return;
    }
    const picked = TOOLS.find((t) => t.key === key);
    if (picked) tool = picked.id;
    else if (["1", "2", "3"].includes(key) && sizeSpec) setSize(sizeSpec.presets[Number(key) - 1]);
    else if (key === "[" || key === "]") nudgeSize(key === "]" ? 1 : -1);
  }

  onMount(() => {
    let observer: ResizeObserver | undefined;
    (async () => {
      const info = await invoke<{ width: number; height: number; scale: number } | null>("shot_info", { id });
      if (!info) return closeWindow();
      scale = info.scale || 1;
      base = await loadImage(imageUrl(`shot-${id}`));
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

{#snippet toolButtons(group: ToolGroup)}
  {#each TOOLS.filter((t) => t.group === group) as t (t.id)}
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
{/snippet}

<div class="app">
  <header>
    <div class="group" role="toolbar" aria-label={tr("Tools")}>
      {@render toolButtons("pick")}
      <span class="rule"></span>
      {@render toolButtons("draw")}
      <span class="rule"></span>
      {@render toolButtons("hide")}
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

    <div class="group" role="radiogroup" aria-label={tr("Color")}>
      {#each COLORS as c (c)}
        <button
          class="swatch"
          class:active={shownColor === c}
          style="--c:{c}"
          title={c}
          aria-label={tr("Color {c}", { c })}
          aria-pressed={shownColor === c}
          onclick={() => setColor(c)}
        ></button>
      {/each}
      <label class="swatch custom" class:active={customShown} style="--c:{customShown ? shownColor : custom}" title={tr("Custom color")}>
        <input
          type="color"
          aria-label={tr("Custom color")}
          value={customShown ? shownColor : custom}
          onclick={() => setColor(custom)}
          oninput={(e) => {
            custom = e.currentTarget.value;
            setColor(custom);
          }}
        />
      </label>
    </div>

    <div class="group size" class:off={!sizeSpec}>
      <label
        title={sizeSpec
          ? tr("{label} (1 2 3, [ ]; double-click to reset)", { label: tr(sizeSpec.label) })
          : tr("This tool has no size")}
        ondblclick={() => sizeSpec && setSize(sizeSpec.presets[1])}
      >
        <span class="muted">{tr(sizeSpec?.label ?? "Size")}</span>
        {#if sizeSpec}
          <input
            type="range"
            min={sizeSpec.min}
            max={sizeSpec.max}
            step="1"
            value={sizeValue}
            oninput={(e) => setSize(Number(e.currentTarget.value))}
          />
          <span class="value">{sizeValue}</span>
        {:else}
          <input type="range" disabled />
          <span class="value">–</span>
        {/if}
      </label>
    </div>

    <div class="group backdrop-group" role="toolbar" aria-label={tr("Image")}>
      <span class="muted caption">{tr("Image")}</span>
      {@render toolButtons("image")}
      {#each TURNS as t (t.label)}
        <button class="icon" title={tr(t.label)} aria-label={tr(t.label)} disabled={!image} onclick={() => turn(t.op)}>
          <svg viewBox="0 0 18 18"><path d={t.d} /></svg>
        </button>
      {/each}
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
        <svg viewBox="0 0 18 18"><path d={TRASH} /></svg>
      </button>
    </div>

    <!-- Last, so the rest of the bar stays put when these come and go. -->
    {#if tool === "text" || textSel}
      <div class="group">
        <select
          aria-label={tr("Font")}
          value={textSel?.font ?? font}
          onchange={(e) => setText({ font: e.currentTarget.value as FontId })}
        >
          {#each Object.entries(FONTS) as [fid, f] (fid)}
            <option value={fid}>{f.label}</option>
          {/each}
        </select>
        <button
          class="icon"
          class:active={textSel?.bold ?? bold}
          title={tr("Bold")}
          aria-label={tr("Bold")}
          aria-pressed={textSel?.bold ?? bold}
          onclick={() => setText({ bold: !(textSel?.bold ?? bold) })}
        >
          <b>B</b>
        </button>
        <button
          class="icon"
          class:active={textSel ? !!textSel.bg : textBg}
          title={tr("Text background")}
          aria-label={tr("Text background")}
          aria-pressed={textSel ? !!textSel.bg : textBg}
          onclick={() => setText({ bg: !(textSel ? !!textSel.bg : textBg) })}
        >
          <svg viewBox="0 0 18 18"><path d="M4.5 2.5h9a2 2 0 012 2v9a2 2 0 01-2 2h-9a2 2 0 01-2-2v-9a2 2 0 012-2zM6.3 12.5L9 5.5l2.7 7M7.2 10.3h3.6" /></svg>
        </button>
      </div>
    {/if}
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
          class:select-tool={tool === "select"}
          class:over={hover === "move"}
          class:turn={hover === "rotate"}
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
            style="left:{text.x * ratio}px; top:{text.y * ratio}px; width:{textWidth}px; color:{textBg ? badgeText(color) : color}; font:{fontCss(font, bold, textFontPx)}; line-height:1.25"
            style:background={textBg ? color : undefined}
            style:box-shadow={textBg ? `0 0 0 ${textPad(textFontPx)}px ${color}` : undefined}
            style:border-radius={textBg ? `${textFontPx * 0.05}px` : undefined}
            onkeydown={onTextKey}
            onblur={commitText}
          ></textarea>
        {/if}
        {#if objectBar}
          <div class="object-bar" role="toolbar" aria-label={tr("Selected object")} style="left:{objectBar.x}px; top:{objectBar.y}px">
            {#if objectBar.flip}
              <button class="icon" title={tr("Flip horizontal")} aria-label={tr("Flip horizontal")} onclick={() => flipSelected(true)}>
                <svg viewBox="0 0 18 18"><path d={TURNS[2].d} /></svg>
              </button>
              <button class="icon" title={tr("Flip vertical")} aria-label={tr("Flip vertical")} onclick={() => flipSelected(false)}>
                <svg viewBox="0 0 18 18"><path d={TURNS[3].d} /></svg>
              </button>
            {/if}
            <button class="icon" title="{tr('Delete')} (⌫)" aria-label={tr("Delete")} onclick={removeSelected}>
              <svg viewBox="0 0 18 18"><path d={TRASH} /></svg>
            </button>
          </div>
        {/if}
      </div>
      </div>
    </div>
    {#if status}
      <div class="status" role="status">{status}</div>
    {/if}
  </main>

  <footer>
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
      <!-- Explains the image with its annotations, which can point the AI at what matters. -->
      <button title={tr("Explain with AI")} onclick={() => exportImage("explain")}>{tr("Explain")}</button>
      <button title="{tr('Keep on screen')} ({mod}P)" onclick={() => exportImage("pin")}>{tr("Pin")}</button>
      <button title="{tr('Save As…')} ({mod}⇧S)" onclick={() => exportImage("saveas")}>{tr("Save As…")}</button>
      <button title="{tr('Save')} ({mod}S)" onclick={() => exportImage("save")}>{tr("Save")}</button>
      <button title="{tr('Save & Copy')} ({mod}⇧C)" onclick={() => exportImage("savecopy")}>{tr("Save & Copy")}</button>
      <button class="primary" title="{tr('Copy')} ({mod}C)" onclick={() => exportImage("copy")}>{tr("Copy")}</button>
    </div>
  </footer>
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
  footer {
    display: flex;
    justify-content: flex-end;
    padding: 8px 12px;
    background: var(--panel);
    border-top: 1px solid var(--border);
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
  .swatch.custom {
    position: relative;
    margin-left: 4px;
    cursor: pointer;
  }
  .swatch.custom:not(.active),
  .swatch.custom:not(.active):hover {
    background: conic-gradient(#ff3b30, #ffcc00, #34c759, #00c7be, #007aff, #af52de, #ff3b30);
  }
  .swatch.custom input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    opacity: 0;
    cursor: pointer;
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
  canvas.select-tool {
    cursor: default;
  }
  canvas.over {
    cursor: move;
  }
  canvas.turn {
    cursor: grab;
  }
  .rule {
    width: 1px;
    height: 18px;
    margin: 0 6px;
    background: var(--border);
  }
  .caption {
    margin-right: 4px;
  }
  .object-bar {
    position: absolute;
    z-index: 5;
    display: flex;
    gap: 2px;
    padding: 2px;
    transform: translateX(-50%);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.2);
  }
  .object-bar .icon {
    width: 26px;
    height: 26px;
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
