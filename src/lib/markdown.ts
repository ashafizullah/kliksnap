export type Block = { kind: "h" | "li" | "p" | "pre"; parts: Part[] };
export type Part = { text: string; code?: boolean; bold?: boolean; em?: boolean };

/** Just enough markdown for release notes and AI answers: headings, list
 * items, paragraphs, code blocks, bold, italics and code. */
export function parse(md: string): Block[] {
  const blocks: Block[] = [];
  let fence: string[] | null = null;
  for (const raw of md.split("\n")) {
    if (raw.trim().startsWith("```")) {
      if (fence) blocks.push({ kind: "pre", parts: [{ text: fence.join("\n"), code: true }] });
      fence = fence ? null : [];
      continue;
    }
    if (fence) {
      fence.push(raw);
      continue;
    }
    const line = raw.trim();
    if (!line) continue;
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    const item = line.match(/^(?:[-*•]|\d+[.)])\s+(.*)$/);
    if (heading) blocks.push({ kind: "h", parts: inline(heading[1]) });
    else if (item) blocks.push({ kind: "li", parts: inline(item[1]) });
    else blocks.push({ kind: "p", parts: inline(line) });
  }
  // An answer cut off inside a code block still shows it.
  if (fence?.length) blocks.push({ kind: "pre", parts: [{ text: fence.join("\n"), code: true }] });
  return blocks;
}

function inline(text: string): Part[] {
  // Links keep their text only: these windows don't navigate.
  text = text.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
  const parts: Part[] = [];
  for (const piece of text.split(/(`[^`]+`|\*\*[^*]+\*\*|\*[^*\s][^*]*\*)/)) {
    if (!piece) continue;
    if (piece.startsWith("`")) parts.push({ text: piece.slice(1, -1), code: true });
    else if (piece.startsWith("**")) parts.push({ text: piece.slice(2, -2), bold: true });
    else if (piece.startsWith("*")) parts.push({ text: piece.slice(1, -1), em: true });
    else parts.push({ text: piece });
  }
  return parts;
}
