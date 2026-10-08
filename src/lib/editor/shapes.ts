export type Tool = "arrow" | "line" | "rect" | "ellipse" | "text" | "highlight" | "pixelate" | "crop";

export type DragShape = {
  kind: Exclude<Tool, "text" | "crop">;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  size: number;
};

export type TextShape = { kind: "text"; x: number; y: number; text: string; color: string; size: number };

export type Shape = DragShape | TextShape;

export type Rect = { x: number; y: number; w: number; h: number };

export type Scene = { shapes: Shape[]; crop: Rect | null };

export const COLORS = ["#ff3b30", "#ff9500", "#ffcc00", "#34c759", "#007aff", "#000000", "#ffffff"];

/** Stroke widths (logical px) for the S/M/L presets; text uses FONT_SIZES. */
export const STROKES = [2, 4, 7];
export const FONT_SIZES = [16, 24, 36];

export function normalize(x1: number, y1: number, x2: number, y2: number): Rect {
  return { x: Math.min(x1, x2), y: Math.min(y1, y2), w: Math.abs(x2 - x1), h: Math.abs(y2 - y1) };
}

/** Shift-drag: squares for boxes, 45° steps for lines. */
export function constrain(kind: Tool, x1: number, y1: number, x2: number, y2: number): [number, number] {
  const dx = x2 - x1;
  const dy = y2 - y1;
  if (kind === "arrow" || kind === "line") {
    const step = Math.PI / 4;
    const angle = Math.round(Math.atan2(dy, dx) / step) * step;
    const len = Math.hypot(dx, dy);
    return [x1 + Math.cos(angle) * len, y1 + Math.sin(angle) * len];
  }
  const side = Math.max(Math.abs(dx), Math.abs(dy));
  return [x1 + Math.sign(dx || 1) * side, y1 + Math.sign(dy || 1) * side];
}
