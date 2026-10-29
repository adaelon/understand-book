import katex from "katex";

export interface BoardMathSegment {
  kind: "text" | "math";
  /** Exact saved text, including delimiters; offsets address the original string. */
  source: string;
  start: number;
  end: number;
  html: string;
  displayMode: boolean;
  /** The TeX passed to KaTeX. Only legacy, un-delimited formulas are normalized. */
  tex?: string;
}

export interface BoardMathLine {
  segments: BoardMathSegment[];
}

export interface ParsedBoardMath {
  lines: BoardMathLine[];
  hasMath: boolean;
}

function escapeHtml(source: string): string {
  return source.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}

function textSegment(source: string, start: number): BoardMathSegment {
  return { kind: "text", source, start, end: start + source.length, html: escapeHtml(source), displayMode: false };
}

function mathSegment(source: string, start: number, tex: string, displayMode: boolean): BoardMathSegment {
  try {
    return {
      kind: "math", source, start, end: start + source.length, tex, displayMode,
      html: katex.renderToString(tex, { displayMode, throwOnError: true, trust: false, strict: "ignore" }),
    };
  } catch {
    // Broken formulas stay editable/readable instead of displaying KaTeX's error panel.
    return textSegment(source, start);
  }
}

function isEscaped(source: string, index: number): boolean {
  let slashes = 0;
  for (let i = index - 1; i >= 0 && source[i] === "\\"; i -= 1) slashes += 1;
  return slashes % 2 === 1;
}

// Old boards used single-letter mathematical variables without delimiters. Require
// both a script and an operator, and never match inside identifiers or paths.
const script = String.raw`(?:[_^](?:\{[A-Za-z0-9+\- ]+\}|\([A-Za-z0-9+\- ]+\)|[A-Za-z0-9]))`;
const atom = String.raw`(?:[A-Za-z]${script}*|\d+(?:\.\d+)?)(?![A-Za-z0-9_])`;
const bareFormula = new RegExp(String.raw`(?<![A-Za-z0-9_\\/])${atom}(?:(?:[ \t]*[=+−\-×·⋅*][ \t]*|[ \t]+)${atom})+`, "g");

function normalizeLegacyMath(source: string): string {
  return source.replace(/([_^])\(([^()]+)\)/g, "$1{$2}")
    .replace(/×/g, "\\times ").replace(/[·⋅]/g, "\\cdot ").replace(/−/g, "-")
    .replace(/\^T(?![A-Za-z0-9])/g, "^{\\top}");
}

function parseLegacyText(source: string, start: number): BoardMathSegment[] {
  const segments: BoardMathSegment[] = [];
  let cursor = 0;
  // A fresh matcher keeps this parser independent of other calls and render frames.
  for (const match of source.matchAll(new RegExp(bareFormula.source, "g"))) {
    const index = match.index!;
    const end = index + match[0].length;
    const before = source[index - 1] ?? "";
    const after = source[end] ?? "";
    if (/[A-Za-z0-9_\\/]/.test(before) || /[A-Za-z0-9_\\/]/.test(after)) continue;
    if (!/[_^]/.test(match[0]) || !/[=+−\-×·⋅*]/.test(match[0])) continue;
    if (index > cursor) segments.push(textSegment(source.slice(cursor, index), start + cursor));
    segments.push(mathSegment(match[0], start + index, normalizeLegacyMath(match[0]), false));
    cursor = end;
  }
  if (cursor < source.length) segments.push(textSegment(source.slice(cursor), start + cursor));
  return segments;
}

function tokenize(source: string): BoardMathSegment[] {
  const segments: BoardMathSegment[] = [];
  let plainStart = 0;
  let cursor = 0;
  const flushPlain = (end: number) => {
    if (end > plainStart) segments.push(...parseLegacyText(source.slice(plainStart, end), plainStart));
  };
  while (cursor < source.length) {
    // Preserve explicit code examples as text, like the answer Markdown renderer.
    if (source[cursor] === "`" && !isEscaped(source, cursor)) {
      const ticks = source.slice(cursor).match(/^`+/)![0];
      const close = source.indexOf(ticks, cursor + ticks.length);
      if (close >= 0) {
        flushPlain(cursor);
        const end = close + ticks.length;
        segments.push(textSegment(source.slice(cursor, end), cursor));
        cursor = plainStart = end;
        continue;
      }
    }
    let open = "";
    let close = "";
    if (!isEscaped(source, cursor)) {
      if (source.startsWith("$$", cursor)) open = close = "$$";
      else if (source[cursor] === "$") open = close = "$";
      else if (source.startsWith("\\(", cursor)) { open = "\\("; close = "\\)"; }
      else if (source.startsWith("\\[", cursor)) { open = "\\["; close = "\\]"; }
    }
    if (!open) { cursor += 1; continue; }
    let closeIndex = source.indexOf(close, cursor + open.length);
    while (closeIndex >= 0 && (isEscaped(source, closeIndex)
      || (open === "$" && (source[closeIndex - 1] === "$" || source[closeIndex + 1] === "$")))) {
      closeIndex = source.indexOf(close, closeIndex + close.length);
    }
    if (closeIndex < 0) {
      flushPlain(cursor);
      segments.push(textSegment(source.slice(cursor), cursor));
      return segments;
    }
    flushPlain(cursor);
    const end = closeIndex + close.length;
    const raw = source.slice(cursor + open.length, closeIndex);
    const tex = raw.trim();
    // Match md.ts's treatment of ordinary spaced currency, e.g. "$ 10 $".
    const spacedCurrency = open === "$" && raw !== tex && !/[\\_^]/.test(tex);
    segments.push(tex && !spacedCurrency
      ? mathSegment(source.slice(cursor, end), cursor, tex, open === "$$" || open === "\\[" || /\r?\n/.test(raw))
      : textSegment(source.slice(cursor, end), cursor));
    cursor = plainStart = end;
  }
  flushPlain(source.length);
  return segments;
}

/** Parse once per text change; reveal math segments atomically using start/end. */
export function parseBoardMath(text: string): ParsedBoardMath {
  const lines: BoardMathLine[] = [{ segments: [] }];
  let afterDisplay = false;
  const nextLine = () => lines.push({ segments: [] });
  for (const segment of tokenize(text)) {
    if (segment.kind === "math") {
      if (afterDisplay || (segment.displayMode && lines[lines.length - 1].segments.length > 0)) nextLine();
      lines[lines.length - 1].segments.push(segment);
      afterDisplay = segment.displayMode;
      continue;
    }
    let localStart = 0;
    if (afterDisplay) {
      nextLine();
      // A display formula already ends its visual line; consume one source break.
      localStart = segment.source.match(/^\r?\n/)?.[0].length ?? 0;
      afterDisplay = false;
    }
    const newline = /\r?\n/g;
    newline.lastIndex = localStart;
    let match: RegExpExecArray | null;
    while ((match = newline.exec(segment.source))) {
      if (match.index > localStart) lines[lines.length - 1].segments.push(textSegment(segment.source.slice(localStart, match.index), segment.start + localStart));
      nextLine();
      localStart = match.index + match[0].length;
    }
    if (localStart < segment.source.length) lines[lines.length - 1].segments.push(textSegment(segment.source.slice(localStart), segment.start + localStart));
  }
  return { lines, hasMath: lines.some((line) => line.segments.some((segment) => segment.kind === "math")) };
}
