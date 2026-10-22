import type { PdfGeometryChar, PdfGeometryLine, PdfGeometryPage, PdfTextGeometry } from "./pdf-geometry";
import { pageLinesInReadingOrder } from "./pdf-reading-order";
import type { SourceBlock } from "./segment";

export interface PdfSourceBinding {
  char: PdfGeometryChar;
  source_span: { start: number; end: number };
}
export interface PdfNativeSource {
  source: string;
  blocks: SourceBlock[];
  bindings: PdfSourceBinding[];
  paragraphs: Array<{ block: SourceBlock; lines: PdfGeometryLine[] }>;
}

/** Preserve literal PDF text when the canonical reader interprets Markdown. */
function literal(text: string, lineStart: boolean): string {
  const escaped = text.replace(/([\\`*_[\]<>#$|~!])/gu, "\\$1");
  return lineStart ? escaped.replace(/^([+\-])/u, "\\$1") : escaped;
}

function joinsParagraph(previous: PdfGeometryLine, current: PdfGeometryLine, page: PdfGeometryPage): boolean {
  const height = Math.max(previous.bbox[3] - previous.bbox[1], current.bbox[3] - current.bbox[1], 1);
  const drop = previous.bbox[3] - current.bbox[3];
  return drop > height * 0.5 && drop < height * 1.8
    && Math.abs(previous.bbox[0] - current.bbox[0]) < Math.max(8, page.width * 0.02)
    && Math.abs((previous.bbox[3] - previous.bbox[1]) - (current.bbox[3] - current.bbox[1])) < height * 0.25;
}

/** PDF.js may put two columns in one line separated by a wide synthetic space. */
function separateColumns(page: PdfGeometryPage): PdfGeometryPage {
  const lines: PdfGeometryLine[] = [], byIndex = new Map(page.chars.map(char => [char.charIndex, char]));
  let active: PdfGeometryChar[] = [];
  const flush = () => {
    if (!active.some(char => char.text.trim())) { active = []; return; }
    lines.push({ pageIndex: page.pageIndex, lineIndex: lines.length, text: active.map(char => char.text).join(''),
      char_start: active[0].charIndex, char_end: active.at(-1)!.charIndex + 1,
      bbox: [Math.min(...active.map(char => char.bbox[0])), Math.min(...active.map(char => char.bbox[1])), Math.max(...active.map(char => char.bbox[2])), Math.max(...active.map(char => char.bbox[3]))] });
    active = [];
  };
  for (const line of page.lines) {
    const height = Math.max(1, line.bbox[3] - line.bbox[1]);
    for (let index = line.char_start; index < line.char_end; index++) {
      const char = byIndex.get(index); if (!char) continue;
      if (!char.text.trim() && char.bbox[2] - char.bbox[0] > height * 2.5) { flush(); continue; }
      if (active.length && char.bbox[0] - active.at(-1)!.bbox[2] > height * 2.5) flush();
      active.push(char);
    }
    flush();
  }
  return { ...page, lines };
}

/** Text and spans are emitted together: repeated sentences never need to be searched for again. */
export function sourceFromPdfGeometry(geometry: PdfTextGeometry): PdfNativeSource {
  let source = "";
  const bindings: PdfSourceBinding[] = [], blocks: SourceBlock[] = [];
  const paragraphs: PdfNativeSource["paragraphs"] = [];
  for (const original of geometry.pages) {
    const page = separateColumns(original);
    const ordered = pageLinesInReadingOrder(page, true);
    // The alignment adapter excludes oversized lines. They are real source text too.
    const included = new Set(ordered.map(line => line.lineIndex));
    const lines = ordered.concat(page.lines.filter(line => !included.has(line.lineIndex)));
    const groups: PdfGeometryLine[][] = [];
    for (const line of lines) {
      if (!line.text.trim()) continue;
      const previous = groups.at(-1)?.at(-1);
      if (previous && joinsParagraph(previous, line, page)) groups.at(-1)!.push(line);
      else groups.push([line]);
    }
    const byIndex = new Map(page.chars.map(char => [char.charIndex, char]));
    for (const group of groups) {
      const start = source.length;
      for (const [lineIndex, line] of group.entries()) {
        if (lineIndex) source += "\n";
        const lineStart = source.length;
        const chars: PdfGeometryChar[] = [];
        for (let index = line.char_start; index < line.char_end; index++) {
          const char = byIndex.get(index);
          if (char) chars.push(char);
        }
        while (chars.length && !chars[0].text.trim()) chars.shift();
        while (chars.length && !chars.at(-1)!.text.trim()) chars.pop();
        for (const char of chars) {
          const from = source.length;
          source += literal(char.text, from === lineStart);
          bindings.push({ char, source_span: { start: from, end: source.length } });
        }
        // Prevent a PDF line such as "1. Example" becoming a Markdown list.
        const raw = source.slice(lineStart);
        const numbered = /^\d+[.)](?=\s)/u.exec(raw);
        if (numbered) {
          const at = lineStart + numbered[0].length - 1;
          source = source.slice(0, at) + "\\" + source.slice(at);
          for (let index = bindings.length - 1; index >= 0 && bindings[index].source_span.end > at; index--) {
            const span = bindings[index].source_span;
            if (span.start > at) span.start++;
            span.end++;
          }
        }
      }
      const block: SourceBlock = { kind: "leaf", text: group.map(line => line.text.trim()).join("\n"), span: { start, end: source.length } };
      blocks.push(block); paragraphs.push({ block, lines: group });
      source += "\n\n";
    }
  }
  return { source, blocks, bindings, paragraphs };
}
