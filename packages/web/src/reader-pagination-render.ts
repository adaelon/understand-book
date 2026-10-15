import { renderInlineMarkdown, renderMarkdown, renderFormulaSource, renderTableSource, isDisplayFormulaSource } from './md';
import { typographyHtml } from './reader-typography';
import type { PaginationSource } from './reader-pagination';
import { isReaderFlowSegment } from './reader-flow';

export interface PaginationRenderSource extends PaginationSource { imageAsset?: { url_path?: string | null; original_src?: string | null; status?: string } | null; preparedFormulaHtml?: string }

/** Uses the same Markdown, KaTeX and typography renderers as ReaderPane. */
export function renderPaginationSource(source: PaginationRenderSource): HTMLElement {
  const block = /^\s{0,3}(?:#{1,6}\s|[-*+]\s|\d+[.)]\s|>\s)/m.test(source.text);
  const flow = isReaderFlowSegment(source) && !block;
  const root = document.createElement(source.kind === 'code' ? 'pre' : flow ? 'span' : 'div');
  root.dataset.lid = source.lid;
  if (source.kind === 'code') { root.className = 'focus-object'; root.textContent = source.text; }
  else if (source.kind === 'table') { root.className = 'focus-object reader-table asset-table'; root.innerHTML = renderTableSource(source.text); }
  else if (source.kind === 'image') {
    root.className = 'focus-object';
    const image = document.createElement('img');
    image.alt = source.text.match(/!\[([^\]]*)\]/)?.[1] ?? '';
    const asset = source.imageAsset;
    image.src = asset?.url_path ?? (asset?.status === 'external' ? asset.original_src : null) ?? source.text.match(/!\[[^\]]*\]\(([^)]+)\)/)?.[1] ?? '';
    root.append(image);
  } else if (source.kind === 'formula') {
    root.className = isDisplayFormulaSource(source.text) ? 'focus-object formula-display' : 'focus-flow-leaf';
    root.innerHTML = source.preparedFormulaHtml ?? renderFormulaSource(source.text);
    root.setAttribute('role', 'button'); root.tabIndex = 0; root.title = '查看公式语义剖面';
  } else {
    if (flow) root.className = 'focus-flow-leaf';
    root.innerHTML = typographyHtml(block ? renderMarkdown(source.text) : flow ? renderInlineMarkdown(source.text) : `<p class="flow-paragraph">${renderInlineMarkdown(source.text)}</p>`);
  }
  return root;
}
