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

// DOM serialization normalizes emphasis markers, list numbering and code fences.
// Align that projection with the delivered source, then return only a verified
// contiguous slice of the source. No reconstructed Markdown is sent as evidence.
export function answerNoteExcerpt(range: Range, sources: string[]): string | null {
  const clone = range.cloneContents();
  clone.querySelectorAll('.agent-source-button').forEach(node => node.remove());
  const clean = document.createRange(); clean.selectNodeContents(clone);
  const selected = rangeToMarkdown(clean);
  if (!selected) return null;
  for (const source of sources) {
    if (source.includes(selected)) return selected;
    const projection = canonical(source);
    const start = projection.indexOf(selected);
    if (start < 0) continue;
    const text = document.createTextNode(projection);
    const selectedRange = document.createRange();
    selectedRange.setStart(text, start); selectedRange.setEnd(text, start + selected.length);
    const ranges = createMarkdownDomSourceMap(source, text).sourceRangesForRange(selectedRange);
    if (!ranges.length) continue;
    const excerpt = source.slice(ranges[0].start, ranges[ranges.length - 1].end);
    if (excerpt && canonical(excerpt) === selected) return excerpt;
  }
  return null;
}
