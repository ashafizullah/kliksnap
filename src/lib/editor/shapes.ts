export type Tool = "arrow" | "line" | "rect" | "ellipse" | "text" | "step" | "highlight" | "pixelate" | "crop";

export type DragShape = {
  kind: Exclude<Tool, "text" | "step" | "crop">;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  size: number;
};

export type TextShape = {
  kind: "text";
  x: number;
  y: number;
  text: string;
  color: string;
  size: number;
  font: FontId;
  bold: boolean;
};

/** A numbered badge; `n` is fixed when placed, counting the badges before it. */
export type StepShape = { kind: "step"; x: number; y: number; n: number; color: string; size: number };

export type Shape = DragShape | TextShape | StepShape;

export type Rect = { x: number; y: number; w: number; h: number };

export type Scene = { shapes: Shape[]; crop: Rect | null };

export const COLORS = ["#ff3b30", "#ff9500", "#ffcc00", "#34c759", "#007aff", "#000000", "#ffffff"];

export type SizeSpec = { label: string; min: number; max: number; presets: number[] };

const STROKE: SizeSpec = { label: "Thickness", min: 1, max: 20, presets: [2, 4, 7] };

/** The size control of each tool that has one, in logical px; keys 1 2 3 pick the presets. */
export const SIZES: Partial<Record<Tool, SizeSpec>> = {
  arrow: STROKE,
  line: STROKE,
  rect: STROKE,
  ellipse: STROKE,
  text: { label: "Font size", min: 10, max: 96, presets: [16, 24, 36] },
  step: { label: "Size", min: 8, max: 40, presets: [10, 14, 20] },
  pixelate: { label: "Block size", min: 3, max: 30, presets: [5, 8, 14] },
};

export const FONTS = {
  sans: { label: "Sans", stack: `"Plus Jakarta Sans Variable", -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif` },
  serif: { label: "Serif", stack: `Georgia, "Times New Roman", serif` },
  mono: { label: "Mono", stack: `ui-monospace, Menlo, Consolas, monospace` },
  hand: { label: "Hand", stack: `"Chalkboard SE", "Comic Sans MS", cursive` },
};

export type FontId = keyof typeof FONTS;

export const fontCss = (font: FontId, bold: boolean, px: number) =>
  `${bold ? 700 : 400} ${px}px ${(FONTS[font] ?? FONTS.sans).stack}`;

/** The number the next badge gets. */
export const nextStep = (shapes: Shape[]) => shapes.filter((s) => s.kind === "step").length + 1;

/** Black text on light badges, white on dark ones. */
export function badgeText(color: string) {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(color.slice(i, i + 2), 16));
  return 0.299 * r + 0.587 * g + 0.114 * b > 170 ? "#000000" : "#ffffff";
}

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
