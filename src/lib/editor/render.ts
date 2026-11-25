import {
  FILLS,
  SIZES,
  badgeText,
  constrain,
  fontCss,
  normalize,
  textPad,
  turnPoint,
  turnedSize,
  type Backdrop,
  type FrameId,
  type Orient,
  type Rect,
  type Scene,
  type Shape,
  type TextShape,
} from "./shapes";

let scratch: HTMLCanvasElement | null = null;
let measurer: CanvasRenderingContext2D | null = null;

/** `base` turned by `m`, as a canvas the size of the result. */
export function turnImage(base: HTMLImageElement, m: Orient): HTMLCanvasElement {
  const w = base.naturalWidth;
  const h = base.naturalHeight;
  const t = turnedSize(m, w, h);
  const canvas = document.createElement("canvas");
  canvas.width = t.w;
  canvas.height = t.h;
  const ctx = canvas.getContext("2d")!;
  ctx.setTransform(m[0], m[2], m[1], m[3], t.w / 2, t.h / 2);
  ctx.drawImage(base, -w / 2, -h / 2);
  return canvas;
}

/** The size of the box a text shape covers, in image pixels. */
function textBox(s: TextShape, scale: number) {
  measurer ??= document.createElement("canvas").getContext("2d")!;
  const size = s.size * scale;
  measurer.font = fontCss(s.font, s.bold, size);
  const lines = s.text.split("\n");
  return { w: Math.max(...lines.map((l) => measurer!.measureText(l).width)), h: lines.length * size * 1.25 };
}

/**
 * Moves a shape along with a `w`×`h` image that is turned by `m`. Text keeps
 * reading upright: only its box moves.
 */
export function turnShape(s: Shape, m: Orient, w: number, h: number, scale: number): Shape {
  const at = (x: number, y: number) => turnPoint(m, w, h, x, y);
  if (s.kind === "pen") {
    const points: number[] = [];
    for (let i = 0; i < s.points.length; i += 2) points.push(...at(s.points[i], s.points[i + 1]));
    return { ...s, points };
  }
  if (s.kind === "step") {
    const [x, y] = at(s.x, s.y);
    return { ...s, x, y };
  }
  // A mirrored image turns its shapes the other way.
  const rot = s.rot && m[0] * m[3] - m[1] * m[2] < 0 ? -s.rot : s.rot;
  if (s.kind === "text") {
    const box = textBox(s, scale);
    const [cx, cy] = at(s.x + box.w / 2, s.y + box.h / 2);
    return { ...s, x: cx - box.w / 2, y: cy - box.h / 2, rot };
  }
  const [x1, y1] = at(s.x1, s.y1);
  const [x2, y2] = at(s.x2, s.y2);
  return { ...s, x1, y1, x2, y2, rot };
}

/** The box a shape covers, in image pixels. */
export function shapeBounds(s: Shape, scale: number): Rect {
  const half = (s.size * scale) / 2;
  if (s.kind === "pen") {
    let [x1, y1, x2, y2] = [Infinity, Infinity, -Infinity, -Infinity];
    for (let i = 0; i < s.points.length; i += 2) {
      x1 = Math.min(x1, s.points[i]);
      x2 = Math.max(x2, s.points[i]);
      y1 = Math.min(y1, s.points[i + 1]);
      y2 = Math.max(y2, s.points[i + 1]);
    }
    return { x: x1 - half, y: y1 - half, w: x2 - x1 + 2 * half, h: y2 - y1 + 2 * half };
  }
  if (s.kind === "step") {
    const r = s.size * scale;
    return { x: s.x - r, y: s.y - r, w: 2 * r, h: 2 * r };
  }
  if (s.kind === "text") {
    const box = textBox(s, scale);
    const pad = s.bg ? textPad(s.size * scale) : 0;
    return { x: s.x - pad, y: s.y - pad, w: box.w + 2 * pad, h: box.h + 2 * pad };
  }
  const r = normalize(s.x1, s.y1, s.x2, s.y2);
  if (s.kind === "highlight" || s.kind === "blur" || s.kind === "pixelate" || s.kind === "spotlight") return r;
  return { x: r.x - half, y: r.y - half, w: r.w + 2 * half, h: r.h + 2 * half };
}

/** The angle a shape is drawn at about its center; lines and strokes turn their points instead. */
function spin(s: Shape): number {
  return s.kind === "rect" || s.kind === "ellipse" || s.kind === "highlight" || s.kind === "text" ? (s.rot ?? 0) : 0;
}

export function shapeCenter(s: Shape, scale: number): [number, number] {
  const b = shapeBounds(s, scale);
  return [b.x + b.w / 2, b.y + b.h / 2];
}

/** Blurs, pixelation, spotlights and magnifiers act on what is under them, and badges are round: these don't turn. */
export const canRotate = (s: Shape) =>
  s.kind !== "blur" && s.kind !== "pixelate" && s.kind !== "step" && s.kind !== "spotlight" && s.kind !== "loupe";

/** Whether mirroring a shape changes how it looks. */
export const canFlip = (s: Shape) =>
  s.kind === "arrow" || s.kind === "line" || s.kind === "pen" || s.kind === "text" || spin(s) !== 0;

/** `s` turned by `angle` radians about its center. */
export function rotateShape(s: Shape, angle: number, scale: number): Shape {
  if (!canRotate(s)) return s;
  if (s.kind === "rect" || s.kind === "ellipse" || s.kind === "highlight" || s.kind === "text") {
    return { ...s, rot: (s.rot ?? 0) + angle };
  }
  const [cx, cy] = shapeCenter(s, scale);
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  const at = (x: number, y: number) => [cx + (x - cx) * cos - (y - cy) * sin, cy + (x - cx) * sin + (y - cy) * cos];
  if (s.kind === "pen") {
    const points: number[] = [];
    for (let i = 0; i < s.points.length; i += 2) points.push(...at(s.points[i], s.points[i + 1]));
    return { ...s, points };
  }
  if (s.kind === "step") return s;
  const [x1, y1] = at(s.x1, s.y1);
  const [x2, y2] = at(s.x2, s.y2);
  return { ...s, x1, y1, x2, y2 };
}

/** `s` mirrored about its center, left to right when `horizontal`, else top to bottom. */
export function flipShape(s: Shape, horizontal: boolean, scale: number): Shape {
  if (s.kind === "step" || !canRotate(s)) return s;
  if (s.kind === "text") {
    const flipped = horizontal ? { flipX: !s.flipX } : { flipY: !s.flipY };
    return { ...s, ...flipped, rot: -(s.rot ?? 0) };
  }
  if (s.kind === "rect" || s.kind === "ellipse" || s.kind === "highlight") return { ...s, rot: -(s.rot ?? 0) };
  const [cx, cy] = shapeCenter(s, scale);
  const mx = (x: number) => (horizontal ? 2 * cx - x : x);
  const my = (y: number) => (horizontal ? y : 2 * cy - y);
  if (s.kind === "pen") return { ...s, points: s.points.map((v, i) => (i % 2 ? my(v) : mx(v))) };
  return { ...s, x1: mx(s.x1), y1: my(s.y1), x2: mx(s.x2), y2: my(s.y2) };
}

function segmentDistance(px: number, py: number, x1: number, y1: number, x2: number, y2: number) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = dx * dx + dy * dy;
  const t = len ? Math.max(0, Math.min(1, ((px - x1) * dx + (py - y1) * dy) / len)) : 0;
  return Math.hypot(px - (x1 + t * dx), py - (y1 + t * dy));
}

/**
 * Whether `x, y` is on a shape, give or take `tol` image pixels. Outlines are
 * hit on their stroke only, so what they surround stays reachable.
 */
export function hitShape(s: Shape, x: number, y: number, scale: number, tol: number): boolean {
  const angle = spin(s);
  if (angle) {
    // Test in the shape's own frame.
    const [cx, cy] = shapeCenter(s, scale);
    const cos = Math.cos(-angle);
    const sin = Math.sin(-angle);
    [x, y] = [cx + (x - cx) * cos - (y - cy) * sin, cy + (x - cx) * sin + (y - cy) * cos];
  }
  const reach = tol + (s.size * scale) / 2;
  if (s.kind === "pen") {
    const p = s.points;
    if (p.length === 2) return Math.hypot(x - p[0], y - p[1]) <= reach;
    for (let i = 0; i < p.length - 2; i += 2) {
      if (segmentDistance(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) <= reach) return true;
    }
    return false;
  }
  if (s.kind === "line" || s.kind === "arrow") return segmentDistance(x, y, s.x1, s.y1, s.x2, s.y2) <= reach;
  if (s.kind === "step") return Math.hypot(x - s.x, y - s.y) <= s.size * scale + tol;
  const b = shapeBounds(s, scale);
  const inside = (pad: number) => x >= b.x - pad && x <= b.x + b.w + pad && y >= b.y - pad && y <= b.y + b.h + pad;
  if (s.kind === "rect") {
    const r = normalize(s.x1, s.y1, s.x2, s.y2);
    const inner = x > r.x + reach && x < r.x + r.w - reach && y > r.y + reach && y < r.y + r.h - reach;
    return inside(tol) && (!!s.fill || !inner);
  }
  if (s.kind === "ellipse" || s.kind === "loupe") {
    const r = normalize(s.x1, s.y1, s.x2, s.y2);
    const rx = r.w / 2;
    const ry = r.h / 2;
    if (rx < 1 || ry < 1) return inside(tol);
    const d = Math.hypot((x - r.x - rx) / rx, (y - r.y - ry) / ry);
    if (s.kind === "loupe" || s.fill) return (d - 1) * Math.min(rx, ry) <= reach;
    return Math.abs(d - 1) * Math.min(rx, ry) <= reach;
  }
  return inside(tol);
}

const SELECTION_PAD = 4;
const HANDLE_REACH = 20;

/**
 * Where the rotate handle of a selected shape sits, or null when the shape
 * can't be turned. `unit` is image pixels per CSS pixel. The handle goes above
 * the shape, or below when that would put it off a `w`×`h` image.
 */
export function handleAt(s: Shape, scale: number, unit: number, w: number, h: number): [number, number] | null {
  // Lines turn by their ends.
  if (!canRotate(s) || s.kind === "line" || s.kind === "arrow") return null;
  const b = shapeBounds(s, scale);
  const [cx, cy] = [b.x + b.w / 2, b.y + b.h / 2];
  const reach = b.h / 2 + (SELECTION_PAD + HANDLE_REACH) * unit;
  const angle = spin(s);
  const at = (side: number): [number, number] => [cx - side * Math.sin(angle) * -reach, cy + side * Math.cos(angle) * -reach];
  const on = ([x, y]: [number, number]) => x >= 0 && x <= w && y >= 0 && y <= h;
  const above = at(1);
  const below = at(-1);
  return on(above) || !on(below) ? above : below;
}

/** The upright box around a selected shape and its outline, in image pixels. */
export function selectionExtent(s: Shape, scale: number, unit: number) {
  const b = shapeBounds(s, scale);
  const pad = SELECTION_PAD * unit;
  const [cx, cy] = [b.x + b.w / 2, b.y + b.h / 2];
  const angle = spin(s);
  const hw = (Math.abs(Math.cos(angle)) * (b.w + 2 * pad) + Math.abs(Math.sin(angle)) * (b.h + 2 * pad)) / 2;
  const hh = (Math.abs(Math.sin(angle)) * (b.w + 2 * pad) + Math.abs(Math.cos(angle)) * (b.h + 2 * pad)) / 2;
  return { cx, cy, minX: cx - hw, maxX: cx + hw, minY: cy - hh, maxY: cy + hh };
}

/** A resize handle: an end of a line, or a corner or side of a box (`hx`, `hy` are -1, 0 or 1). */
export type Grip = { x: number; y: number; hx: number; hy: number; end?: 1 | 2 };

/** The box resizing works on: the drawn rectangle, without the stroke around it. */
function core(s: Shape, scale: number): Rect {
  return s.kind === "pen" || s.kind === "text" || s.kind === "step" ? shapeBounds(s, scale) : normalize(s.x1, s.y1, s.x2, s.y2);
}

/** The resize handles of a selected shape, where they are drawn. `unit` is image pixels per CSS pixel. */
export function grips(s: Shape, scale: number, unit: number): Grip[] {
  if (s.kind === "line" || s.kind === "arrow") {
    return [
      { x: s.x1, y: s.y1, hx: 0, hy: 0, end: 1 },
      { x: s.x2, y: s.y2, hx: 0, hy: 0, end: 2 },
    ];
  }
  const b = shapeBounds(s, scale);
  const pad = SELECTION_PAD * unit;
  const [cx, cy] = [b.x + b.w / 2, b.y + b.h / 2];
  const cos = Math.cos(spin(s));
  const sin = Math.sin(spin(s));
  // Text and badges keep their shape, and short sides have no room for a handle.
  const sides = s.kind !== "text" && s.kind !== "step";
  const out: Grip[] = [];
  for (const hy of [-1, 0, 1]) {
    for (const hx of [-1, 0, 1]) {
      if (!hx && !hy) continue;
      if ((!hx || !hy) && !sides) continue;
      if (!hx && b.w < 28 * unit) continue;
      if (!hy && b.h < 28 * unit) continue;
      const lx = hx * (b.w / 2 + pad);
      const ly = hy * (b.h / 2 + pad);
      out.push({ x: cx + lx * cos - ly * sin, y: cy + lx * sin + ly * cos, hx, hy });
    }
  }
  return out;
}

/** The point of the shape a handle stands for. */
export function gripPoint(s: Shape, g: Grip, scale: number): [number, number] {
  if (s.kind === "line" || s.kind === "arrow") return g.end === 1 ? [s.x1, s.y1] : [s.x2, s.y2];
  const b = core(s, scale);
  const cos = Math.cos(spin(s));
  const sin = Math.sin(spin(s));
  const lx = (g.hx * b.w) / 2;
  const ly = (g.hy * b.h) / 2;
  return [b.x + b.w / 2 + lx * cos - ly * sin, b.y + b.h / 2 + lx * sin + ly * cos];
}

/** The cursor for a handle: the resize arrow nearest the way it pulls. */
export function gripCursor(s: Shape, g: Grip) {
  if (g.end) return "crosshair";
  const turns = Math.round((Math.atan2(g.hy, g.hx) + spin(s)) / (Math.PI / 4));
  return ["ew-resize", "nwse-resize", "ns-resize", "nesw-resize"][((turns % 4) + 4) % 4];
}

/**
 * `s` with handle `g` pulled to `px, py`; the opposite corner, side or end
 * stays put. Shift keeps the proportions of a box, and lines to steps of 45°.
 */
export function resizeShape(s: Shape, g: Grip, px: number, py: number, scale: number, shift: boolean): Shape {
  if (s.kind === "line" || s.kind === "arrow") {
    const [ox, oy] = g.end === 1 ? [s.x2, s.y2] : [s.x1, s.y1];
    const [x, y] = shift ? constrain(s.kind, ox, oy, px, py) : [px, py];
    return g.end === 1 ? { ...s, x1: x, y1: y } : { ...s, x2: x, y2: y };
  }
  const b = core(s, scale);
  const [cx, cy] = [b.x + b.w / 2, b.y + b.h / 2];
  const cos = Math.cos(spin(s));
  const sin = Math.sin(spin(s));
  const world = (x: number, y: number): [number, number] => [x * cos - y * sin, x * sin + y * cos];
  const [ox, oy] = world((-g.hx * b.w) / 2, (-g.hy * b.h) / 2);
  // The fixed point, and the pull from it in the shape's own frame.
  const [ax, ay] = [cx + ox, cy + oy];
  let dx = (px - ax) * cos + (py - ay) * sin;
  let dy = -(px - ax) * sin + (py - ay) * cos;

  if (s.kind === "text" || s.kind === "step") {
    const spec = SIZES[s.kind]!;
    const k = Math.max(b.w ? (g.hx * dx) / b.w : 0, b.h ? (g.hy * dy) / b.h : 0);
    const size = Math.min(spec.max, Math.max(spec.min, Math.round(s.size * k)));
    const sized = { ...s, size };
    const nb = shapeBounds(sized, scale);
    const [mx, my] = world((g.hx * nb.w) / 2, (g.hy * nb.h) / 2);
    if (sized.kind === "step") return { ...sized, x: ax + mx, y: ay + my };
    const box = textBox(sized, scale);
    return { ...sized, x: ax + mx - box.w / 2, y: ay + my - box.h / 2 };
  }
  if (s.kind === "pen") {
    // Scale the stroke's points, not the width around them: the box is that much larger on every side.
    const half = (s.size * scale) / 2;
    const [rw, rh] = [b.w - 2 * half, b.h - 2 * half];
    const [px0, py0] = [ax + g.hx * half, ay + g.hy * half];
    let kx = g.hx && rw > 1e-6 ? (dx - 2 * g.hx * half) / (g.hx * rw) : 1;
    let ky = g.hy && rh > 1e-6 ? (dy - 2 * g.hy * half) / (g.hy * rh) : 1;
    if (shift && g.hx && g.hy) {
      const k = Math.max(Math.abs(kx), Math.abs(ky));
      kx = Math.sign(kx || 1) * k;
      ky = Math.sign(ky || 1) * k;
    }
    return { ...s, points: s.points.map((v, i) => (i % 2 ? py0 + (v - py0) * ky : px0 + (v - px0) * kx)) };
  }
  if (shift && g.hx && g.hy && b.w && b.h) {
    const k = Math.max(Math.abs(dx) / b.w, Math.abs(dy) / b.h);
    dx = Math.sign(dx || 1) * b.w * k;
    dy = Math.sign(dy || 1) * b.h * k;
  }
  const w = g.hx ? Math.abs(dx) : b.w;
  const h = g.hy ? Math.abs(dy) : b.h;
  const [mx, my] = world(g.hx ? dx / 2 : 0, g.hy ? dy / 2 : 0);
  const [nx, ny] = [ax + mx, ay + my];
  return { ...s, x1: nx - w / 2, y1: ny - h / 2, x2: nx + w / 2, y2: ny + h / 2 };
}

/** Marks the selected shape with its resize and rotate handles (editing view only). */
export function drawSelection(
  ctx: CanvasRenderingContext2D,
  s: Shape,
  scale: number,
  unit: number,
  handle: [number, number] | null,
  handles: Grip[] = [],
) {
  const b = shapeBounds(s, scale);
  const pad = SELECTION_PAD * unit;
  const [cx, cy] = [b.x + b.w / 2, b.y + b.h / 2];
  const strokes: [string, number[]][] = [
    ["#ffffff", []],
    ["#007aff", [4 * unit, 3 * unit]],
  ];
  ctx.save();
  ctx.lineWidth = unit;
  ctx.save();
  ctx.translate(cx, cy);
  ctx.rotate(spin(s));
  // A line shows by its ends alone.
  for (const [color, dash] of s.kind === "line" || s.kind === "arrow" ? [] : strokes) {
    ctx.strokeStyle = color;
    ctx.setLineDash(dash);
    ctx.strokeRect(-b.w / 2 - pad, -b.h / 2 - pad, b.w + 2 * pad, b.h + 2 * pad);
  }
  ctx.restore();
  if (handle) {
    const [hx, hy] = handle;
    const len = Math.hypot(cx - hx, cy - hy) || 1;
    const stem = HANDLE_REACH * unit;
    ctx.strokeStyle = "#007aff";
    ctx.beginPath();
    ctx.moveTo(hx, hy);
    ctx.lineTo(hx + ((cx - hx) / len) * stem, hy + ((cy - hy) / len) * stem);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(hx, hy, 5 * unit, 0, Math.PI * 2);
    ctx.fillStyle = "#ffffff";
    ctx.fill();
    ctx.lineWidth = 1.5 * unit;
    ctx.stroke();
  }
  ctx.strokeStyle = "#007aff";
  ctx.fillStyle = "#ffffff";
  ctx.lineWidth = 1.5 * unit;
  const half = 4 * unit;
  for (const g of handles) {
    ctx.beginPath();
    if (g.end) ctx.arc(g.x, g.y, half * 1.2, 0, Math.PI * 2);
    else ctx.rect(g.x - half, g.y - half, 2 * half, 2 * half);
    ctx.fill();
    ctx.stroke();
  }
  ctx.restore();
}

function pixelate(ctx: CanvasRenderingContext2D, base: CanvasImageSource, r: Rect, block: number) {
  if (r.w < 1 || r.h < 1) return;
  scratch ??= document.createElement("canvas");
  const bw = Math.max(1, Math.ceil(r.w / block));
  const bh = Math.max(1, Math.ceil(r.h / block));
  scratch.width = bw;
  scratch.height = bh;
  // Always sample the original pixels, so annotations below can't leak through.
  scratch.getContext("2d")!.drawImage(base, r.x, r.y, r.w, r.h, 0, 0, bw, bh);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(scratch, 0, 0, bw, bh, r.x, r.y, r.w, r.h);
  ctx.imageSmoothingEnabled = true;
}

function blur(ctx: CanvasRenderingContext2D, base: CanvasImageSource, r: Rect, radius: number) {
  if (r.w < 1 || r.h < 1) return;
  ctx.save();
  ctx.beginPath();
  ctx.rect(r.x, r.y, r.w, r.h);
  ctx.clip();
  ctx.filter = `blur(${radius}px)`;
  // Sample a margin around the region too, so its edges don't fade to transparent.
  const m = radius * 2;
  ctx.drawImage(base, r.x - m, r.y - m, r.w + 2 * m, r.h + 2 * m, r.x - m, r.y - m, r.w + 2 * m, r.h + 2 * m);
  ctx.restore();
}

/** Smooths the stroke by curving through the midpoints between samples. */
function pen(ctx: CanvasRenderingContext2D, pts: number[]) {
  ctx.beginPath();
  ctx.moveTo(pts[0], pts[1]);
  if (pts.length === 2) ctx.lineTo(pts[0] + 0.01, pts[1]);
  for (let i = 2; i < pts.length - 2; i += 2) {
    ctx.quadraticCurveTo(pts[i], pts[i + 1], (pts[i] + pts[i + 2]) / 2, (pts[i + 1] + pts[i + 3]) / 2);
  }
  if (pts.length > 2) ctx.lineTo(pts[pts.length - 2], pts[pts.length - 1]);
  ctx.stroke();
}

function arrow(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number, width: number, both: boolean) {
  const angle = Math.atan2(y2 - y1, x2 - x1);
  const len = Math.hypot(x2 - x1, y2 - y1);
  const head = Math.min(Math.max(width * 3.5, 12), both ? len / 2 : len);
  const spread = 0.42;
  const back = head * 0.85;
  const [cos, sin] = [Math.cos(angle), Math.sin(angle)];
  ctx.beginPath();
  ctx.moveTo(both ? x1 + cos * back : x1, both ? y1 + sin * back : y1);
  ctx.lineTo(x2 - cos * back, y2 - sin * back);
  ctx.stroke();
  const tip = (x: number, y: number, a: number) => {
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x - Math.cos(a - spread) * head, y - Math.sin(a - spread) * head);
    ctx.lineTo(x - Math.cos(a + spread) * head, y - Math.sin(a + spread) * head);
    ctx.closePath();
    ctx.fill();
  };
  tip(x2, y2, angle);
  if (both) tip(x1, y1, angle + Math.PI);
}

/** Shows what is under an ellipse at twice its size, inside a ring. */
function loupe(ctx: CanvasRenderingContext2D, base: CanvasImageSource, r: Rect, width: number, scale: number) {
  if (r.w < 2 || r.h < 2) return;
  const [cx, cy] = [r.x + r.w / 2, r.y + r.h / 2];
  const ring = () => {
    ctx.beginPath();
    ctx.ellipse(cx, cy, r.w / 2, r.h / 2, 0, 0, Math.PI * 2);
  };
  ctx.save();
  ctx.shadowColor = "rgba(0, 0, 0, 0.4)";
  ctx.shadowBlur = 10 * scale;
  ctx.shadowOffsetY = 3 * scale;
  ctx.fillStyle = "#000000";
  ring();
  ctx.fill();
  ctx.restore();
  ctx.save();
  ring();
  ctx.clip();
  // Always the original pixels, like blur: other annotations don't show in it.
  ctx.drawImage(base, cx - r.w / 4, cy - r.h / 4, r.w / 2, r.h / 2, r.x, r.y, r.w, r.h);
  ctx.restore();
  ctx.lineWidth = width;
  ring();
  ctx.stroke();
}

let dimmer: HTMLCanvasElement | null = null;

/** Darkens the image outside the spotlights. */
function dim(ctx: CanvasRenderingContext2D, w: number, h: number, spots: Shape[], scale: number) {
  dimmer ??= document.createElement("canvas");
  dimmer.width = w;
  dimmer.height = h;
  const d = dimmer.getContext("2d")!;
  d.fillStyle = "rgba(0, 0, 0, 0.6)";
  d.fillRect(0, 0, w, h);
  d.globalCompositeOperation = "destination-out";
  d.fillStyle = "#000000";
  for (const s of spots) {
    if (s.kind !== "spotlight") continue;
    const r = normalize(s.x1, s.y1, s.x2, s.y2);
    d.beginPath();
    d.roundRect(r.x, r.y, r.w, r.h, 6 * scale);
    d.fill();
  }
  ctx.drawImage(dimmer, 0, 0);
}

/** `scale` converts the logical sizes stored on shapes into image pixels. */
export function drawShape(ctx: CanvasRenderingContext2D, base: CanvasImageSource, s: Shape, scale: number) {
  ctx.save();
  ctx.strokeStyle = s.color;
  ctx.fillStyle = s.color;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  const width = s.size * scale;
  ctx.lineWidth = width;
  const angle = spin(s);
  const [fx, fy] = s.kind === "text" ? [s.flipX ? -1 : 1, s.flipY ? -1 : 1] : [1, 1];
  if (angle || fx < 0 || fy < 0) {
    const [cx, cy] = shapeCenter(s, scale);
    ctx.translate(cx, cy);
    ctx.rotate(angle);
    ctx.scale(fx, fy);
    ctx.translate(-cx, -cy);
  }

  if ((s.kind === "pen" || s.kind === "line" || s.kind === "arrow" || s.kind === "rect" || s.kind === "ellipse") && s.dash) {
    ctx.setLineDash([width * 2.5, width * 2.2]);
  }
  if (s.kind === "spotlight") {
    // Drawn by the scene, under every shape.
  } else if (s.kind === "loupe") {
    loupe(ctx, base, normalize(s.x1, s.y1, s.x2, s.y2), width, scale);
  } else if (s.kind === "pixelate") {
    pixelate(ctx, base, normalize(s.x1, s.y1, s.x2, s.y2), Math.max(2, s.size * scale));
  } else if (s.kind === "blur") {
    blur(ctx, base, normalize(s.x1, s.y1, s.x2, s.y2), Math.max(1, s.size * scale));
  } else if (s.kind === "highlight") {
    const r = normalize(s.x1, s.y1, s.x2, s.y2);
    ctx.globalCompositeOperation = "multiply";
    ctx.globalAlpha = 0.45;
    ctx.fillRect(r.x, r.y, r.w, r.h);
  } else {
    ctx.shadowColor = "rgba(0, 0, 0, 0.3)";
    ctx.shadowBlur = 3 * scale;
    ctx.shadowOffsetY = 1 * scale;
    if (s.kind === "text") {
      const fontSize = s.size * scale;
      ctx.font = fontCss(s.font, s.bold, fontSize);
      ctx.textBaseline = "top";
      if (s.bg) {
        const box = textBox(s, scale);
        const pad = textPad(fontSize);
        ctx.beginPath();
        ctx.roundRect(s.x - pad, s.y - pad, box.w + 2 * pad, box.h + 2 * pad, fontSize * 0.3);
        ctx.fill();
        ctx.shadowColor = "transparent";
        ctx.fillStyle = badgeText(s.color);
      }
      const lines = s.text.split("\n");
      if (s.outline && !s.bg) {
        ctx.strokeStyle = badgeText(s.color);
        ctx.lineWidth = fontSize * 0.16;
        lines.forEach((line, i) => ctx.strokeText(line, s.x, s.y + i * fontSize * 1.25));
        ctx.shadowColor = "transparent";
      }
      lines.forEach((line, i) => ctx.fillText(line, s.x, s.y + i * fontSize * 1.25));
    } else if (s.kind === "step") {
      const r = s.size * scale;
      ctx.beginPath();
      ctx.arc(s.x, s.y, r, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowColor = "transparent";
      ctx.fillStyle = badgeText(s.color);
      ctx.font = fontCss("sans", true, r * (s.n > 9 ? 1.05 : 1.25));
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      // Digits sit a little high when centered on their em box.
      ctx.fillText(String(s.n), s.x, s.y + r * 0.06);
    } else if (s.kind === "pen") {
      pen(ctx, s.points);
    } else if (s.kind === "arrow") {
      arrow(ctx, s.x1, s.y1, s.x2, s.y2, width, !!s.both);
    } else if (s.kind === "line") {
      ctx.beginPath();
      ctx.moveTo(s.x1, s.y1);
      ctx.lineTo(s.x2, s.y2);
      ctx.stroke();
    } else if (s.kind === "rect") {
      const r = normalize(s.x1, s.y1, s.x2, s.y2);
      ctx.beginPath();
      ctx.roundRect(r.x, r.y, r.w, r.h, 2 * scale);
      if (s.fill) ctx.fill();
      else ctx.stroke();
    } else if (s.kind === "ellipse") {
      const r = normalize(s.x1, s.y1, s.x2, s.y2);
      ctx.beginPath();
      ctx.ellipse(r.x + r.w / 2, r.y + r.h / 2, r.w / 2, r.h / 2, 0, 0, Math.PI * 2);
      if (s.fill) ctx.fill();
      else ctx.stroke();
    }
  }
  ctx.restore();
}

export function drawScene(
  ctx: CanvasRenderingContext2D,
  base: HTMLCanvasElement,
  scene: Scene,
  scale: number,
  extra: Shape | null = null,
) {
  ctx.drawImage(base, 0, 0);
  const spots = [...scene.shapes, ...(extra ? [extra] : [])].filter((s) => s.kind === "spotlight");
  if (spots.length) dim(ctx, base.width, base.height, spots, scale);
  for (const s of scene.shapes) drawShape(ctx, base, s, scale);
  if (extra) drawShape(ctx, base, extra, scale);
}

/** Darkens everything outside the crop rectangle (editing view only). */
/** Darkens what the crop leaves out; with `unit` (image pixels per CSS pixel), also draws its resize handles. */
export function drawCropMask(ctx: CanvasRenderingContext2D, crop: Rect, scale: number, unit = 0) {
  const { width, height } = ctx.canvas;
  ctx.save();
  ctx.fillStyle = "rgba(0, 0, 0, 0.55)";
  ctx.beginPath();
  ctx.rect(0, 0, width, height);
  ctx.rect(crop.x, crop.y, crop.w, crop.h);
  ctx.fill("evenodd");
  ctx.strokeStyle = "#fff";
  ctx.lineWidth = scale;
  ctx.setLineDash([6 * scale, 4 * scale]);
  ctx.strokeRect(crop.x, crop.y, crop.w, crop.h);
  if (unit) {
    ctx.setLineDash([]);
    ctx.fillStyle = "#fff";
    ctx.strokeStyle = "rgba(0, 0, 0, 0.5)";
    ctx.lineWidth = unit;
    const half = 4.5 * unit;
    for (const fy of [0, 0.5, 1]) {
      for (const fx of [0, 0.5, 1]) {
        if (fx === 0.5 && fy === 0.5) continue;
        // Kept whole on the canvas when the crop reaches its edge.
        const x = Math.min(Math.max(crop.x + fx * crop.w, half), width - half);
        const y = Math.min(Math.max(crop.y + fy * crop.h, half), height - half);
        ctx.beginPath();
        ctx.rect(x - half, y - half, 2 * half, 2 * half);
        ctx.fill();
        ctx.stroke();
      }
    }
  }
  ctx.restore();
}

/** The sides of a crop a press takes hold of: one or two to resize it, "move" (inside) to drag all of it. */
export type CropHold = { l: boolean; r: boolean; t: boolean; b: boolean } | "move";

/** What a press at `x, y` takes hold of on crop `c`, within `tol` of a side; null outside it. */
export function cropHoldAt(c: Rect, x: number, y: number, tol: number): CropHold | null {
  if (x < c.x - tol || x > c.x + c.w + tol || y < c.y - tol || y > c.y + c.h + tol) return null;
  const [dl, dr, dt, db] = [Math.abs(x - c.x), Math.abs(x - c.x - c.w), Math.abs(y - c.y), Math.abs(y - c.y - c.h)];
  // On a small crop both sides are in reach: the nearer one wins.
  const hold = { l: dl <= tol && dl <= dr, r: dr <= tol && dr < dl, t: dt <= tol && dt <= db, b: db <= tol && db < dt };
  return hold.l || hold.r || hold.t || hold.b ? hold : "move";
}

export function cropCursor(h: CropHold) {
  if (h === "move") return "move";
  if ((h.l && h.t) || (h.r && h.b)) return "nwse-resize";
  if ((h.r && h.t) || (h.l && h.b)) return "nesw-resize";
  return h.l || h.r ? "ew-resize" : "ns-resize";
}

/** Crop `c` with the sides `h` holds pulled by `dx, dy`, kept on a `w`×`h` image and at least `min` across. */
export function pullCrop(c: Rect, hold: CropHold, dx: number, dy: number, w: number, h: number, min: number): Rect {
  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);
  if (hold === "move") return { ...c, x: clamp(c.x + dx, 0, w - c.w), y: clamp(c.y + dy, 0, h - c.h) };
  const [mw, mh] = [Math.min(min, c.w), Math.min(min, c.h)];
  let [x1, y1, x2, y2] = [c.x, c.y, c.x + c.w, c.y + c.h];
  if (hold.l) x1 = clamp(x1 + dx, 0, x2 - mw);
  if (hold.r) x2 = clamp(x2 + dx, x1 + mw, w);
  if (hold.t) y1 = clamp(y1 + dy, 0, y2 - mh);
  if (hold.b) y2 = clamp(y2 + dy, y1 + mh, h);
  return { x: x1, y: y1, w: x2 - x1, h: y2 - y1 };
}

/** The part of crop `c` on a `w`×`h` image, in whole pixels: what the export keeps. */
export function cropOnImage(c: Rect, w: number, h: number): Rect {
  const x = Math.max(0, Math.round(c.x));
  const y = Math.max(0, Math.round(c.y));
  return {
    x,
    y,
    w: Math.max(1, Math.min(w, Math.round(c.x + c.w)) - x),
    h: Math.max(1, Math.min(h, Math.round(c.y + c.h)) - y),
  };
}

export type Insets = { top: number; right: number; bottom: number; left: number };

/**
 * Room a frame takes around a `w`-wide screenshot, in the same pixels as `w`.
 * `unit` is pixels per logical px: the MacBook scales with the screenshot,
 * the window's title bar with the display.
 */
export function frameInsets(frame: FrameId, w: number, unit: number): Insets {
  if (frame === "macbook") {
    const side = Math.round(w * 0.028 + w * 0.08);
    return { top: Math.round(w * 0.04), right: side, bottom: Math.round(w * 0.045 + w * 0.028), left: side };
  }
  if (frame === "desktop") {
    const side = Math.round(w * 0.025);
    return { top: side, right: side, bottom: Math.round(w * (0.09 + 0.12 + 0.015)), left: side };
  }
  if (frame === "phone") {
    const bezel = Math.round(w * 0.05);
    return { top: bezel, right: bezel, bottom: bezel, left: bezel };
  }
  if (frame === "window") return { top: Math.round(28 * unit), right: 0, bottom: 0, left: 0 };
  if (frame === "browser") return { top: Math.round(40 * unit), right: 0, bottom: 0, left: 0 };
  return { top: 0, right: 0, bottom: 0, left: 0 };
}

export type FrameShadow = { blur: number; offsetY: number } | null;

/**
 * Draws `frame` around a screenshot that will sit at `x, y, w, h`; the
 * screenshot itself goes on top, clipped by `frameScreenRadii`.
 */
export function drawFrame(
  ctx: CanvasRenderingContext2D,
  frame: FrameId,
  x: number,
  y: number,
  w: number,
  h: number,
  unit: number,
  radius: number,
  shadow: FrameShadow,
) {
  const silhouette = (paths: Path2D) => {
    if (!shadow) return;
    ctx.save();
    ctx.shadowColor = "rgba(0, 0, 0, 0.35)";
    ctx.shadowBlur = shadow.blur;
    ctx.shadowOffsetY = shadow.offsetY;
    ctx.fillStyle = "#000";
    ctx.fill(paths);
    ctx.restore();
  };
  if (frame === "macbook") {
    const side = w * 0.028;
    const top = w * 0.04;
    const chin = w * 0.045;
    const over = w * 0.08;
    const baseH = w * 0.028;
    const lid = new Path2D();
    lid.roundRect(x - side, y - top, w + 2 * side, h + top + chin, w * 0.03);
    const baseY = y + h + chin;
    const base = new Path2D();
    base.roundRect(x - side - over, baseY, w + 2 * (side + over), baseH, [baseH * 0.15, baseH * 0.15, baseH * 0.5, baseH * 0.5]);
    const all = new Path2D(lid);
    all.addPath(base);
    silhouette(all);
    ctx.save();
    ctx.fillStyle = "#0d0d0f";
    ctx.fill(lid);
    ctx.strokeStyle = "#4a4a4e";
    ctx.lineWidth = Math.max(1, w * 0.002);
    ctx.stroke(lid);
    ctx.beginPath();
    ctx.arc(x + w / 2, y - top / 2, w * 0.0035, 0, Math.PI * 2);
    ctx.fillStyle = "#2a2a30";
    ctx.fill();
    const metal = ctx.createLinearGradient(0, baseY, 0, baseY + baseH);
    metal.addColorStop(0, "#e3e5e8");
    metal.addColorStop(1, "#9fa3a8");
    ctx.fillStyle = metal;
    ctx.fill(base);
    // The thumb notch for opening the lid.
    const nw = w * 0.14;
    const nh = baseH * 0.35;
    ctx.beginPath();
    ctx.roundRect(x + (w - nw) / 2, baseY, nw, nh, [0, 0, nh, nh]);
    ctx.fillStyle = "#a4a7ac";
    ctx.fill();
    ctx.restore();
  } else if (frame === "desktop") {
    const side = w * 0.025;
    const chin = w * 0.09;
    const neckH = w * 0.12;
    const footH = w * 0.015;
    const body = new Path2D();
    body.roundRect(x - side, y - side, w + 2 * side, h + side + chin, w * 0.012);
    const neckTop = y + h + chin;
    const neck = new Path2D();
    neck.moveTo(x + w * 0.43, neckTop);
    neck.lineTo(x + w * 0.57, neckTop);
    neck.lineTo(x + w * 0.59, neckTop + neckH);
    neck.lineTo(x + w * 0.41, neckTop + neckH);
    neck.closePath();
    const foot = new Path2D();
    foot.roundRect(x + w * 0.35, neckTop + neckH, w * 0.3, footH, [footH * 0.3, footH * 0.3, footH * 0.5, footH * 0.5]);
    const all = new Path2D(body);
    all.addPath(neck);
    all.addPath(foot);
    silhouette(all);
    ctx.save();
    const metal = ctx.createLinearGradient(0, neckTop, 0, neckTop + neckH + footH);
    metal.addColorStop(0, "#c9ccd1");
    metal.addColorStop(1, "#9fa3a8");
    ctx.fillStyle = metal;
    ctx.fill(neck);
    ctx.fill(foot);
    ctx.fillStyle = "#e3e5e8";
    ctx.fill(body);
    // The black glass around the screen, above the aluminum chin.
    ctx.beginPath();
    ctx.roundRect(x - side, y - side, w + 2 * side, h + 2 * side, [w * 0.012, w * 0.012, 0, 0]);
    ctx.fillStyle = "#0d0d0f";
    ctx.fill();
    ctx.restore();
  } else if (frame === "phone") {
    const bezel = w * 0.05;
    const body = new Path2D();
    body.roundRect(x - bezel, y - bezel, w + 2 * bezel, h + 2 * bezel, w * 0.16);
    silhouette(body);
    ctx.save();
    ctx.fillStyle = "#0d0d0f";
    ctx.fill(body);
    ctx.strokeStyle = "#5a5a60";
    ctx.lineWidth = Math.max(1, w * 0.008);
    ctx.stroke(body);
    ctx.restore();
  } else if (frame === "window" || frame === "browser") {
    const bar = (frame === "browser" ? 40 : 28) * unit;
    const win = new Path2D();
    win.roundRect(x, y - bar, w, h + bar, radius);
    silhouette(win);
    ctx.save();
    ctx.beginPath();
    ctx.roundRect(x, y - bar, w, bar, [radius, radius, 0, 0]);
    ctx.fillStyle = "#ebebed";
    ctx.fill();
    ctx.fillStyle = "#d0d0d3";
    ctx.fillRect(x, y - Math.max(1, unit * 0.5), w, Math.max(1, unit * 0.5));
    ["#ff5f57", "#febc2e", "#28c840"].forEach((c, i) => {
      ctx.beginPath();
      ctx.arc(x + (14 + i * 20) * unit, y - bar / 2, 6 * unit, 0, Math.PI * 2);
      ctx.fillStyle = c;
      ctx.fill();
    });
    if (frame === "browser") {
      const left = x + 80 * unit;
      const right = x + w - 16 * unit;
      const fieldH = 24 * unit;
      if (right - left > 40 * unit) {
        ctx.beginPath();
        ctx.roundRect(left, y - (bar + fieldH) / 2, right - left, fieldH, fieldH / 2);
        ctx.fillStyle = "#ffffff";
        ctx.fill();
      }
    }
    ctx.restore();
  }
}

/** Corner radii of the screenshot inside a frame (top left, top right, bottom right, bottom left). */
export function frameScreenRadii(frame: FrameId, radius: number, w: number): number[] {
  if (frame === "macbook" || frame === "desktop") return [0, 0, 0, 0];
  if (frame === "phone") return [w * 0.11, w * 0.11, w * 0.11, w * 0.11];
  if (frame === "window" || frame === "browser") return [0, 0, radius, radius];
  return [radius, radius, radius, radius];
}

/** The backdrop's shadow, sized from its padding in pixels. */
function shadowFor(b: Backdrop, pad: number): FrameShadow {
  return b.shadow ? { blur: Math.max(8, pad * 0.5), offsetY: Math.max(2, pad * 0.12) } : null;
}

/** Draws `image` centered on the backdrop, which fills the whole canvas. */
function drawBackdrop(ctx: CanvasRenderingContext2D, image: CanvasImageSource, w: number, h: number, b: Backdrop, scale: number) {
  const pad = Math.round(b.padding * scale);
  const radius = b.radius * scale;
  const { width, height } = ctx.canvas;
  const stops = FILLS[b.fill].stops;
  if (stops.length === 1) {
    ctx.fillStyle = stops[0];
    ctx.fillRect(0, 0, width, height);
  } else if (stops.length > 1) {
    // 135deg, like the CSS preview: top left to bottom right.
    const g = ctx.createLinearGradient(0, 0, width, height);
    stops.forEach((c, i) => g.addColorStop(i / (stops.length - 1), c));
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, width, height);
  }
  if (b.frame !== "none") {
    const inset = frameInsets(b.frame, w, scale);
    const x = pad + inset.left;
    const y = pad + inset.top;
    drawFrame(ctx, b.frame, x, y, w, h, scale, radius, shadowFor(b, pad));
    ctx.save();
    ctx.beginPath();
    ctx.roundRect(x, y, w, h, frameScreenRadii(b.frame, radius, w));
    ctx.clip();
    ctx.drawImage(image, x, y);
    ctx.restore();
    return;
  }
  ctx.save();
  ctx.beginPath();
  ctx.roundRect(pad, pad, w, h, radius);
  if (b.shadow) {
    ctx.save();
    ctx.shadowColor = "rgba(0, 0, 0, 0.35)";
    ctx.shadowBlur = Math.max(8, pad * 0.5);
    ctx.shadowOffsetY = Math.max(2, pad * 0.12);
    ctx.fillStyle = "#000";
    ctx.fill();
    ctx.restore();
  }
  ctx.clip();
  ctx.drawImage(image, pad, pad);
  ctx.restore();
}

/** Renders the final image (annotations applied, crop and backdrop honored) as RGBA bytes. */
export function exportPixels(base: HTMLCanvasElement, scene: Scene, scale: number) {
  const r = cropOnImage(scene.crop ?? { x: 0, y: 0, w: base.width, h: base.height }, base.width, base.height);
  const canvas = document.createElement("canvas");
  canvas.width = r.w;
  canvas.height = r.h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  ctx.translate(-r.x, -r.y);
  drawScene(ctx, base, scene, scale);
  if (!scene.backdrop) {
    const data = ctx.getImageData(0, 0, r.w, r.h).data;
    return { bytes: new Uint8Array(data.buffer), width: r.w, height: r.h };
  }
  const pad = Math.round(scene.backdrop.padding * scale);
  const inset = frameInsets(scene.backdrop.frame, r.w, scale);
  const out = document.createElement("canvas");
  out.width = r.w + 2 * pad + inset.left + inset.right;
  out.height = r.h + 2 * pad + inset.top + inset.bottom;
  const octx = out.getContext("2d", { willReadFrequently: true })!;
  drawBackdrop(octx, canvas, r.w, r.h, scene.backdrop, scale);
  const data = octx.getImageData(0, 0, out.width, out.height).data;
  return { bytes: new Uint8Array(data.buffer), width: out.width, height: out.height };
}
