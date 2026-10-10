import { FILLS, badgeText, fontCss, normalize, type Backdrop, type FrameId, type Rect, type Scene, type Shape } from "./shapes";

let scratch: HTMLCanvasElement | null = null;

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

function arrow(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number, width: number) {
  const angle = Math.atan2(y2 - y1, x2 - x1);
  const head = Math.min(Math.max(width * 3.5, 12), Math.hypot(x2 - x1, y2 - y1));
  const spread = 0.42;
  const baseX = x2 - Math.cos(angle) * head * 0.85;
  const baseY = y2 - Math.sin(angle) * head * 0.85;
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(baseX, baseY);
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(x2, y2);
  ctx.lineTo(x2 - Math.cos(angle - spread) * head, y2 - Math.sin(angle - spread) * head);
  ctx.lineTo(x2 - Math.cos(angle + spread) * head, y2 - Math.sin(angle + spread) * head);
  ctx.closePath();
  ctx.fill();
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

  if (s.kind === "pixelate") {
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
      s.text.split("\n").forEach((line, i) => ctx.fillText(line, s.x, s.y + i * fontSize * 1.25));
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
      arrow(ctx, s.x1, s.y1, s.x2, s.y2, width);
    } else if (s.kind === "line") {
      ctx.beginPath();
      ctx.moveTo(s.x1, s.y1);
      ctx.lineTo(s.x2, s.y2);
      ctx.stroke();
    } else if (s.kind === "rect") {
      const r = normalize(s.x1, s.y1, s.x2, s.y2);
      ctx.beginPath();
      ctx.roundRect(r.x, r.y, r.w, r.h, 2 * scale);
      ctx.stroke();
    } else if (s.kind === "ellipse") {
      const r = normalize(s.x1, s.y1, s.x2, s.y2);
      ctx.beginPath();
      ctx.ellipse(r.x + r.w / 2, r.y + r.h / 2, r.w / 2, r.h / 2, 0, 0, Math.PI * 2);
      ctx.stroke();
    }
  }
  ctx.restore();
}

export function drawScene(
  ctx: CanvasRenderingContext2D,
  base: HTMLImageElement,
  scene: Scene,
  scale: number,
  extra: Shape | null = null,
) {
  ctx.drawImage(base, 0, 0);
  for (const s of scene.shapes) drawShape(ctx, base, s, scale);
  if (extra) drawShape(ctx, base, extra, scale);
}

/** Darkens everything outside the crop rectangle (editing view only). */
export function drawCropMask(ctx: CanvasRenderingContext2D, crop: Rect, scale: number) {
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
  ctx.restore();
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
export function exportPixels(base: HTMLImageElement, scene: Scene, scale: number) {
  const full = { x: 0, y: 0, w: base.naturalWidth, h: base.naturalHeight };
  const c = scene.crop ?? full;
  const r = {
    x: Math.max(0, Math.round(c.x)),
    y: Math.max(0, Math.round(c.y)),
    w: Math.round(Math.min(c.w, full.w - c.x)),
    h: Math.round(Math.min(c.h, full.h - c.y)),
  };
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
