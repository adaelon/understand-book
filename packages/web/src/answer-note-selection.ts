import { createMarkdownDomSourceMap } from './markdown-source-map';
import { renderMarkdown } from './md';
import { rangeToMarkdown } from './selection';

function canonical(source: string): string {
  const root = document.createElement('div');
  root.innerHTML = renderMarkdown(source);
  const range = document.createRange();
  range.selectNodeContents(root);
  return rangeToMarkdown(range);
}

// Map the actual DOM endpoints, not a serialized selection: a clipped <strong>
// or <li> does not serialize to a substring of its original Markdown.
// Keep the server contract: an excerpt belongs to one delivered Markdown part.
export function answerNoteExcerpt(range: Range, sources: string[], root: Node): string | null {
  if (!root.contains(range.startContainer) || !root.contains(range.endContainer)) return null;
  const source = sources.join('');
  const map = createMarkdownDomSourceMap(source, root);
  const ranges = map.sourceRangesForRange(range);
  if (!ranges.length) return null;
  const start = ranges[0].start, end = ranges[ranges.length - 1].end;
  let offset = 0;
  const part = sources.find(text => {
    const contains = start >= offset && end <= offset + text.length;
    offset += text.length;
    return contains;
  });
  if (!part) return null;
  const clone = range.cloneContents();
  clone.querySelectorAll('.agent-source-button').forEach(node => node.remove());
  const clean = document.createRange(); clean.selectNodeContents(clone);
  const selected = rangeToMarkdown(clean);
  if (!selected) return null;
  // Whole-part selections retain their original list markers, fences and format.
  if (canonical(part) === selected) return part;
  // Keep surrounding syntax when a formatted span is fully selected. A partial
  // span still returns the exact source slice, without inventing closing markers.
  const before = map.spans.find(span => span.kind === 'source-only' && span.sourceEnd === start);
  const after = map.spans.find(span => span.kind === 'source-only' && span.sourceStart === end);
  const starts = before ? [Math.max(offset - part.length, before.sourceStart), start] : [start];
  const ends = after ? [Math.min(offset, after.sourceEnd), end] : [end];
  for (const from of starts) {
    for (const to of ends) {
      const excerpt = source.slice(from, to);
      if (canonical(excerpt) === selected) return excerpt;
    }
  }
  return source.slice(start, end);
}
