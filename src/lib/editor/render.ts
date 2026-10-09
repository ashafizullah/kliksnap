import { FONTS, badgeText, fontCss, normalize, type Rect, type Scene, type Shape } from "./shapes";

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

/** Renders the final image (annotations applied, crop honored) as RGBA bytes. */
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
  const data = ctx.getImageData(0, 0, r.w, r.h).data;
  return { bytes: new Uint8Array(data.buffer), width: r.w, height: r.h };
}
