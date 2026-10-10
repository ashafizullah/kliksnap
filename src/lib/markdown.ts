export type Part = { text: string; code?: boolean; bold?: boolean; em?: boolean; strike?: boolean };
/** `marker` is the item's own number ("3.") or a bullet, so a numbered list
 * keeps the model's numbers even with bullets nested inside. */
export type Item = { parts: Part[]; depth: number; marker: string };
export type Align = "left" | "center" | "right" | null;
export type Block =
  | { kind: "h"; level: number; parts: Part[] }
  | { kind: "p"; parts: Part[] }
  | { kind: "list"; items: Item[] }
  | { kind: "pre"; text: string }
  | { kind: "quote"; parts: Part[] }
  | { kind: "hr" }
  | { kind: "table"; align: Align[]; head: Part[][]; rows: Part[][][] };

const RULE = /^([-*_])(\s*\1){2,}$/;
const SEPARATOR = /^\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?$/;

/** Markdown for release notes and AI answers: headings, lists (numbered and
 * nested), tables, quotes, rules, code blocks, and bold, italic, struck and
 * inline code. Links keep their text only: these windows don't navigate.
 * It builds data, not HTML, so model output can't inject markup. */
export function parse(md: string): Block[] {
  const blocks: Block[] = [];
  const lines = md.replace(/\r\n?/g, "\n").split("\n");
  let i = 0;
  while (i < lines.length) {
    const raw = lines[i];
    const line = raw.trim();
    if (line.startsWith("```")) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith("```")) code.push(lines[i++]);
      i++; // The closing fence; an answer cut off inside the block still shows it.
      blocks.push({ kind: "pre", text: code.join("\n") });
      continue;
    }
    if (!line) {
      i++;
      continue;
    }
    if (isTable(lines, i)) {
      const head = cells(line);
      const align = cells(lines[i + 1].trim()).map(alignOf);
      const rows: Part[][][] = [];
      i += 2;
      while (i < lines.length && lines[i].trim().includes("|")) {
        const row = cells(lines[i++].trim());
        rows.push(head.map((_, c) => row[c] ?? []));
      }
      blocks.push({ kind: "table", align, head, rows });
      continue;
    }
    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    if (heading) {
      blocks.push({ kind: "h", level: heading[1].length, parts: inline(heading[2].replace(/\s+#+$/, "")) });
      i++;
      continue;
    }
    if (RULE.test(line)) {
      blocks.push({ kind: "hr" });
      i++;
      continue;
    }
    if (line.startsWith(">")) {
      const quote: string[] = [];
      while (i < lines.length && lines[i].trim().startsWith(">")) quote.push(lines[i++].trim().replace(/^>\s?/, ""));
      blocks.push({ kind: "quote", parts: inline(quote.join(" ")) });
      continue;
    }
    const item = listItem(raw);
    if (item) {
      const list: Block = { kind: "list", items: [] };
      while (i < lines.length) {
        const next = listItem(lines[i]);
        if (next) {
          list.items.push({ parts: inline(next.text), depth: next.depth, marker: next.marker });
          i++;
        } else if (lines[i].trim() && /^\s{2,}/.test(lines[i]) && list.items.length) {
          // A wrapped line of the item above.
          list.items[list.items.length - 1].parts.push(...inline(" " + lines[i++].trim()));
        } else break;
      }
      blocks.push(list);
      continue;
    }
    // A paragraph runs until a blank line or another kind of block.
    const para: string[] = [line];
    i++;
    while (i < lines.length) {
      const next = lines[i].trim();
      if (!next || /^(#{1,6}\s|```|>|\|)/.test(next) || RULE.test(next) || listItem(lines[i]) || isTable(lines, i)) break;
      para.push(next);
      i++;
    }
    blocks.push({ kind: "p", parts: inline(para.join("\n")) });
  }
  return blocks;
}

/** Whether a table starts at line `i`: a row, then a separator with as many
 * columns, so "a | b" over a "---" rule stays a paragraph. */
function isTable(lines: string[], i: number) {
  const head = lines[i].trim();
  const sep = lines[i + 1]?.trim() ?? "";
  return head.includes("|") && SEPARATOR.test(sep) && cells(head).length === cells(sep).length;
}

function listItem(raw: string) {
  const m = raw.match(/^(\s*)(?:([-*+•])|(\d+)[.)])\s+(.*)$/);
  if (!m) return null;
  const depth = Math.min(Math.floor(m[1].replace(/\t/g, "  ").length / 2), 3);
  return { depth, marker: m[3] ? `${m[3]}.` : depth % 2 ? "◦" : "•", text: m[4] };
}

function cells(row: string): Part[][] {
  // An escaped \| stays in its cell. (No lookbehind: Safari 15 lacks it.)
  return row
    .replace(/\\\|/g, "\u0000")
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((c) => inline(c.trim().replace(/\u0000/g, "|")));
}

function alignOf(cell: Part[]): Align {
  const t = cell.map((p) => p.text).join("");
  const left = t.startsWith(":");
  const right = t.endsWith(":");
  return left && right ? "center" : right ? "right" : left ? "left" : null;
}

function inline(text: string): Part[] {
  text = text.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
  const parts: Part[] = [];
  for (const piece of text.split(/(`[^`]+`|\*\*[^*]+\*\*|__[^_]+__|~~[^~]+~~|\*[^*\s][^*]*\*)/)) {
    if (!piece) continue;
    if (piece.startsWith("`")) parts.push({ text: piece.slice(1, -1), code: true });
    else if (piece.startsWith("**") || piece.startsWith("__")) parts.push({ text: piece.slice(2, -2), bold: true });
    else if (piece.startsWith("~~")) parts.push({ text: piece.slice(2, -2), strike: true });
    else if (piece.startsWith("*")) parts.push({ text: piece.slice(1, -1), em: true });
    else parts.push({ text: piece });
  }
  return parts;
}
