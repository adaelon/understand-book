import type { AnswerHtmlPart } from './answer-markdown';
import type { FollowUpAnchor } from './follow-up-conversations';
import { createMarkdownDomSourceMap, markMarkdownDomSourceRanges, type SourceTextRange } from './markdown-source-map';

function excludedEndpoint(node: Node): boolean {
  const element = node.nodeType === Node.ELEMENT_NODE ? node as Element : node.parentElement;
  return !!element?.closest('[data-reader-selection-ignore], .music-example-inline');
}

/** The DOM offsets, source map and persisted offsets all count UTF-16 code units. */
export function answerFollowUpAnchor(range: Range, part: AnswerHtmlPart, root: Node): FollowUpAnchor | null {
  if (range.collapsed || !root.contains(range.startContainer) || !root.contains(range.endContainer)
    || excludedEndpoint(range.startContainer) || excludedEndpoint(range.endContainer)) return null;
  const source = part.markdown.join('');
  const ranges = createMarkdownDomSourceMap(source, root).sourceRangesForRange(range);
  if (!ranges.length) return null;
  const start = ranges[0].start;
  const end = ranges[ranges.length - 1].end;
  const excerpt = source.slice(start, end);
  if (!excerpt.trim()) return null;
  let cursor = 0;
  let first: { index: number; offset: number } | undefined;
  for (let i = 0; i < part.markdown.length; i++) {
    const limit = cursor + part.markdown[i].length;
    if (start < limit && end > cursor) {
      const index = part.sourcePartIndices[i];
      if (index == null) return null;
      first ??= { index, offset: start - cursor };
      if (end <= limit) return {
        start_part: first.index, start_offset: first.offset,
        end_part: index, end_offset: end - cursor, excerpt,
      };
    }
    cursor = limit;
  }
  return null;
}

function sourceRange(part: AnswerHtmlPart, anchor: FollowUpAnchor): SourceTextRange | null {
  const from = part.sourcePartIndices.indexOf(anchor.start_part);
  const to = part.sourcePartIndices.indexOf(anchor.end_part);
  if (from < 0 || to < from || !Number.isInteger(anchor.start_offset) || !Number.isInteger(anchor.end_offset)
    || anchor.start_offset < 0 || anchor.start_offset >= part.markdown[from].length
    || anchor.end_offset <= 0 || anchor.end_offset > part.markdown[to].length
    || part.sourcePartIndices.slice(from, to + 1).includes(null)) return null;
  const start = part.markdown.slice(0, from).join('').length + anchor.start_offset;
  const end = part.markdown.slice(0, to).join('').length + anchor.end_offset;
  if (end <= start || part.markdown.join('').slice(start, end) !== anchor.excerpt) return null;
  return { start, end };
}

/** Restore the stored range only when the original occurrence still matches. */
export function answerFollowUpRange(part: AnswerHtmlPart, root: HTMLElement, anchor: FollowUpAnchor): Range | null {
  const bounds = sourceRange(part, anchor);
  if (!bounds) return null;
  const map = createMarkdownDomSourceMap(part.markdown.join(''), root);
  const visible = map.semanticRangesForSourceRange(bounds.start, bounds.end);
  if (!visible.length) return null;
  const start = visible[0].start, end = visible[visible.length - 1].end;
  const pieces = map.domPieces.filter(piece => piece.start < end && piece.end > start);
  if (!pieces.length) return null;
  const first = pieces[0], last = pieces[pieces.length - 1];
  const range = root.ownerDocument.createRange();
  if (first.kind === 'text') range.setStart(first.node, (first.domStart ?? 0) + Math.max(0, start - first.start));
  else range.setStartBefore(first.node);
  if (last.kind === 'text') range.setEnd(last.node, (last.domStart ?? 0) + Math.min(last.end, end) - last.start);
  else range.setEndAfter(last.node);
  return range;
}

export interface AnswerFollowUpMark { session_id: string; anchor: FollowUpAnchor }

/** Decorate the already-rendered answer. Citation buttons and note attachments stay independent. */
export function markAnswerFollowUps(part: AnswerHtmlPart, entries: readonly AnswerFollowUpMark[]): string {
  const ranges = entries.flatMap(({ anchor }, index) => {
    const bounds = sourceRange(part, anchor);
    return bounds ? [{ ...bounds, className: 'follow-up-range-' + index }] : [];
  });
  if (!ranges.length) return part.html;
  const root = document.createElement('div');
  root.innerHTML = part.html;
  markMarkdownDomSourceRanges(part.markdown.join(''), root, ranges);
  const focusable = new Set<string>();
  for (const mark of root.querySelectorAll<HTMLElement>('mark[class*="follow-up-range-"]')) {
    // Prefer the most specific quote where two branches overlap. The tree exposes both.
    const indexes = Array.from(mark.classList)
      .filter(name => /^follow-up-range-\d+$/.test(name))
      .map(name => Number(name.slice('follow-up-range-'.length)));
    if (!indexes.length) continue;
    const index = indexes.reduce((a,b) => entries[b].anchor.excerpt.length < entries[a].anchor.excerpt.length ? b : a);
    const branch = entries[index];
    mark.className = 'answer-follow-up';
    mark.dataset.followUpId = branch.session_id;
    if (!mark.closest('a,button')) {
      mark.setAttribute('role', 'button');
      mark.tabIndex = focusable.has(branch.session_id) ? -1 : 0;
      focusable.add(branch.session_id);
    }
  }
  return root.innerHTML;
}
