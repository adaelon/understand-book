import { alignMarkdownSource, bindMarkdownSourceMap, projectMarkdownDom, markMarkdownDomSemanticRanges, type MarkedSourceRange, type AlignmentSpan, type DomProjection } from './markdown-source-map';
import { FOCUS_READING_BUDGET as B, focusPageGeometry } from './reader-pagination-budget';
import { yieldReaderTask } from './reader-cooperative';
import { shouldJoinReaderFlow } from './reader-flow';

export interface PaginationSource { lid: string; text: string; kind: string }
export interface SourceCursor { index: number; offset: number; lid?: string }
export interface PaginationLayout { width: number; height: number; fontSize: number; mobile?: boolean; fontFamily: string; lineHeight: number;
  letterSpacingEm?: number; paragraphGapEm?: number; maxPageWidthEm?: number }
export interface PageFragment { index: number; lid: string; start: number; end: number; semanticStart: number; semanticEnd: number; element: HTMLElement }
export interface DisplayPage { element: HTMLElement; fragments: PageFragment[] }
export interface PreparedScreen { id: number; start: SourceCursor; end: SourceCursor; pages: DisplayPage[]; element: HTMLElement; layout: PaginationLayout; eof: boolean }
interface CompiledSource { source: PaginationSource; html: string; semantic: string; spans: AlignmentSpan[]; atomic: boolean }
export interface PaginationResources { screens: number; pages: number; sourceBytes: number; parsedBytes: number; domNodes: number; decodedImagePixels: number; preparationTasks: number }
export interface PaginationOptions {
  host: HTMLElement;
  read(index: number, signal?: AbortSignal): Promise<PaginationSource | null>;
  render(source: PaginationSource): HTMLElement;
  /** Resolve fonts/images before measuring. FP3 owns acquisition and scheduling. */
  ready?(element: HTMLElement, source: PaginationSource, layout: PaginationLayout): Promise<void>;
  externalResources?(): { sourceBytes: number; parsedBytes: number };
  decorations?(lid: string, source: PaginationSource): { marks: MarkedSourceRange[]; markers: { id: string; start?: number; end?: number }[] };
}
export interface PreparationControl { signal?: AbortSignal; checkpoint?: () => Promise<void>; end?: SourceCursor }
export const compareCursor = (a: SourceCursor, b: SourceCursor) => a.index - b.index || a.offset - b.offset;

function nodeCount(root: Node): number {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_ALL);
  let count = 1; while (walker.nextNode()) count++; return count;
}
function imagePixels(root: HTMLElement): number {
  return Array.from(root.querySelectorAll('img')).reduce((n, image) => n + image.naturalWidth * image.naturalHeight, 0);
}
function domResources(root: HTMLElement) {
  return { parsedBytes: root.innerHTML.length * 2, domNodes: nodeCount(root), decodedImagePixels: imagePixels(root) };
}
function point(projection: DomProjection, offset: number, root: HTMLElement): [Node, number] {
  for (const piece of projection.pieces) {
    if (piece.end <= offset) continue;
    if (piece.kind === 'text') return [piece.node, (piece.domStart ?? 0) + Math.max(0, offset - piece.start)];
    const parent = piece.node.parentNode!;
    return [parent, Array.prototype.indexOf.call(parent.childNodes, piece.node)];
  }
  return [root, root.childNodes.length];
}
function cutPrefix(root: HTMLElement, projection: DomProjection, offset: number) {
  if (!offset) return;
  // Removing earlier <li>s must not restart an ordered list at one.
  for (const ol of root.querySelectorAll('ol')) {
    let value = ol.start || 1;
    for (const child of ol.children) {
      if (child instanceof HTMLLIElement) { value = child.hasAttribute('value') ? child.value : value; child.value = value++; }
    }
  }
  const [container] = point(projection, offset, root);
  const li = (container instanceof Element ? container : container.parentElement)?.closest('li');
  const first = li && projection.pieces.find(piece => li.contains(piece.node));
  if (first && offset > first.start) li!.classList.add('focus-list-continuation');
  const range = document.createRange(); range.selectNodeContents(root);
  range.setEnd(...point(projection, offset, root)); range.deleteContents();
}
function cutSuffix(root: HTMLElement, projection: DomProjection, offset: number) {
  const range = document.createRange(); range.selectNodeContents(root);
  range.setStart(...point(projection, offset, root)); range.deleteContents();
}
function semanticAtSource(compiled: CompiledSource, offset: number): number {
  if (!offset) return 0;
  for (const span of compiled.spans) {
    if (span.sourceEnd <= offset) continue;
    if (span.kind === 'equal') return span.semanticStart + Math.max(0, offset - span.sourceStart);
    return span.semanticStart;
  }
  return compiled.semantic.length;
}
function sourceAtSemantic(compiled: CompiledSource, offset: number): number {
  if (!offset) return 0;
  if (offset >= compiled.semantic.length) return compiled.source.text.length;
  for (const span of compiled.spans) {
    if (span.semanticEnd <= offset || span.kind === 'source-only') continue;
    return span.kind === 'equal' ? span.sourceStart + offset - span.semanticStart : span.sourceStart;
  }
  return compiled.source.text.length;
}

/** Find the first overflowing semantic character, with logarithmic text probes.
 * Collapsed whitespace belongs to the preceding page even when it has no rect.
 * KaTeX and hard breaks stay atomic. Never use a spanning element's bounding box.
 */
function firstOverflow(root: HTMLElement, projection: DomProjection, right: number): number {
  for (const piece of projection.pieces) {
    if (piece.kind !== 'text') {
      const rect = (piece.node as Element).getBoundingClientRect();
      if (rect.height && rect.left >= right - .5) return piece.start;
      continue;
    }
    const node = piece.node as Text, base = piece.domStart ?? 0;
    const range = document.createRange();
    const over = (offset: number) => {
      range.setStart(node, base + offset); range.setEnd(node, base + offset + 1);
      const rect = range.getBoundingClientRect();
      return !!(rect.width || rect.height) && rect.left >= right - .5;
    };
    let last = piece.end - piece.start - 1;
    // Column-end spaces may be unpainted. They cannot hide later visible text.
    while (last >= 0 && /\s/.test(node.data[base + last])) last--;
    if (last < 0 || !over(last)) continue;
    let low = 0, high = last;
    while (low < high) { const mid = (low + high) >>> 1; if (over(mid)) high = mid; else low = mid + 1; }
    if (low > 0 && /[\uDC00-\uDFFF]/.test(node.data[base + low])) low--;
    return piece.start + low;
  }
  return projection.text.length;
}

/** Local pagination only. No book-wide DOM, page-number identity, network policy,
 * progress writes or animation. A screen is immutable after prepare resolves.
 */
export class ReaderPagination {
  private compiled = new Map<number, CompiledSource>();
  private screens: PreparedScreen[] = [];
  private current: PreparedScreen | null = null;
  private pending: PreparedScreen | null = null;
  private scratch: HTMLElement | null = null;
  private sequence = 0;
  private disposed = false;
  private busy = false;
  private protectedScreens = new Set<number>();
  private screenResources = new Map<number, ReturnType<typeof domResources>>();
  private pendingResources?: ReturnType<typeof domResources>;
  private completedPages = new Map<HTMLElement, ReturnType<typeof domResources>>();

  private measurePending(root: HTMLElement) {
    const total = { parsedBytes: 0, domNodes: 1, decodedImagePixels: 0 };
    for (const child of Array.from(root.children) as HTMLElement[]) {
      let value = this.completedPages.get(child);
      if (!value && (child.classList.contains('focus-page') || child.classList.contains('focus-flow'))) {
        value = this.measurePending(child);
        value.parsedBytes += (child.cloneNode(false) as HTMLElement).outerHTML.length * 2;
      }
      value ??= { parsedBytes: child.outerHTML.length * 2, domNodes: nodeCount(child), decodedImagePixels: imagePixels(child) };
      total.parsedBytes += value.parsedBytes; total.domNodes += value.domNodes; total.decodedImagePixels += value.decodedImagePixels;
    }
    return total;
  }
  readonly metrics = { alignments: 0, preparations: 0, shows: 0, geometryProbes: 0, decorations: 0, maxAlignmentMs: 0, maxRenderMs: 0 };
  readonly peak: PaginationResources = { screens: 0, pages: 0, sourceBytes: 0, parsedBytes: 0, domNodes: 0, decodedImagePixels: 0, preparationTasks: 0 };

  constructor(private options: PaginationOptions) { options.host.classList.add('focus-pagination'); }

  resources(): PaginationResources {
    const screens = [...this.screens, ...(this.pending ? [this.pending] : [])];
    // Prepared DOM is immutable except for explicit decoration refreshes. Do
    // not serialize and walk every resident math page for each incoming leaf.
    const roots = screens.map(screen => this.screenResources.get(screen.id) ??
      (screen === this.pending ? (this.pendingResources ??= this.measurePending(screen.element)) : domResources(screen.element)));
    if (this.scratch && !screens.some(screen => screen.element.contains(this.scratch!))) roots.push(domResources(this.scratch));
    const entries = [...this.compiled.values()];
    return {
      screens: screens.length, pages: screens.reduce((n, s) => n + s.pages.length, 0),
      sourceBytes: entries.reduce((n, c) => n + c.source.text.length * 2, 0) + (this.options.externalResources?.().sourceBytes ?? 0),
      parsedBytes: entries.reduce((n, c) => n + (c.html.length + c.semantic.length) * 2 + c.spans.length * 40, 0)
        + roots.reduce((n, root) => n + root.parsedBytes, 0)
        + screens.reduce((n, s) => n + s.pages.reduce((m, p) => m + p.fragments.length * 64, 0), 0)
        + (this.options.externalResources?.().parsedBytes ?? 0),
      domNodes: roots.reduce((n, root) => n + root.domNodes, 0),
      decodedImagePixels: roots.reduce((n, root) => n + root.decodedImagePixels, 0),
      preparationTasks: this.busy ? 1 : 0,
    };
  }
  private fits(r: PaginationResources, extraSourceBytes = 0) {
    return r.screens <= B.maxResidentScreens && r.sourceBytes + extraSourceBytes <= B.maxSourceBytes && r.parsedBytes <= B.maxParsedBytes
      && r.domNodes <= B.maxDomNodes && r.decodedImagePixels <= B.maxDecodedImagePixels;
  }
  private pruneCompiled(keep?: number) {
    const used = new Set([...this.screens, ...(this.pending ? [this.pending] : [])].flatMap(s => s.pages.flatMap(p => p.fragments.map(f => f.index))));
    if (keep !== undefined) used.add(keep);
    for (const index of this.compiled.keys()) if (!used.has(index)) this.compiled.delete(index);
  }
  private victim() { return this.screens.find(s => s !== this.current && !this.protectedScreens.has(s.id)) ?? this.screens.find(s => s !== this.current); }
  protect(screens: PreparedScreen[]) { this.protectedScreens = new Set(screens.map(s => s.id)); }
  reserveSource(bytes: number) { this.reserve(undefined, bytes); }
  private reserve(keep?: number, extraSourceBytes = 0) {
    let sampled = this.resources();
    for (const key of Object.keys(sampled) as (keyof PaginationResources)[]) this.peak[key] = Math.max(this.peak[key], sampled[key]);
    while (!this.fits(sampled, extraSourceBytes)) {
      this.pruneCompiled(keep);
      if (this.fits(this.resources(), extraSourceBytes)) break;
      const victim = this.victim();
      if (!victim) throw new Error('Current screen and required content exceed the focus reading budget');
      this.release(victim, keep);
      sampled = this.resources();
    }
  }
  release(screen: PreparedScreen, keep?: number) {
    if (screen === this.current) throw new Error('Cannot release the visible screen');
    screen.element.remove();
    this.screens = this.screens.filter(s => s !== screen);
    this.screenResources.delete(screen.id);
    // Callers may retain cursors, but released handles must not retain DOM trees.
    screen.pages.length = 0; screen.element.replaceChildren();
    this.pruneCompiled(keep);
  }
  visible() { return this.current; }
  resident() { return this.screens.slice(); }

  async prepare(start: SourceCursor, layout: PaginationLayout, control: PreparationControl = {}): Promise<PreparedScreen> {
    if (this.busy || this.disposed) throw new Error('Pagination is unavailable');
    if (layout.width <= 0 || layout.height <= 0) throw new Error('Pagination needs a visible viewport');
    // Reserve the preparation slot before creating it; it counts as a screen.
    if (this.screens.length === B.maxResidentScreens) this.release(this.victim()!);
    let deadline = performance.now() + 4;
    const checkpoint = async (force = false) => {
      control.signal?.throwIfAborted();
      if (force || performance.now() >= deadline) {
        await yieldReaderTask(); deadline = performance.now() + 4;
      }
      await control.checkpoint?.();
      control.signal?.throwIfAborted();
      if (this.disposed) throw new Error('Pagination disposed');
    };
    this.busy = true;
    const element = document.createElement('div');
    element.className = 'focus-screen'; element.inert = true; element.setAttribute('aria-hidden', 'true');
    element.style.visibility = 'hidden';
    const screen: PreparedScreen = { id: ++this.sequence, start: { ...start }, end: { ...start }, pages: [], element, layout: { ...layout }, eof: false };
    this.pending = screen;
    this.pendingResources = undefined;
    this.completedPages.clear();
    const geometry = focusPageGeometry(layout.width, layout.fontSize, layout.mobile);
    if (layout.maxPageWidthEm) geometry.pageWidth = Math.min(geometry.pageWidth, layout.maxPageWidthEm * layout.fontSize);
    element.style.width = `${layout.width}px`; element.style.height = `${layout.height}px`; element.style.gap = `${geometry.gap}px`;
    this.options.host.append(element);
    let cursor = { ...start };
    try {
      this.reserve();
      for (let pageIndex = 0; pageIndex < geometry.pagesPerScreen; pageIndex++) {
        const article = document.createElement('article'); article.className = 'prose focus-page';
        Object.assign(article.style, { width: `${geometry.pageWidth}px`, height: `${layout.height}px`, columnWidth: `${geometry.pageWidth}px`, columnGap: `${geometry.gap}px`, fontSize: `${layout.fontSize}px`, lineHeight: `${layout.lineHeight}`, fontFamily: layout.fontFamily });
        article.style.setProperty('--focus-page-height', `${layout.height}px`);
        article.style.setProperty('--reader-cjk-spacing', `${layout.letterSpacingEm ?? .015}em`);
        article.style.setProperty('--reader-paragraph-gap', `${layout.paragraphGapEm ?? 1.2}em`);
        const page: DisplayPage = { element: article, fragments: [] };
        screen.pages.push(page); element.append(article); this.pendingResources = undefined;
        let full = false;
        while (!full) {
          control.signal?.throwIfAborted();
          if (control.checkpoint) await control.checkpoint();
          if (control.end && compareCursor(cursor, control.end) >= 0) break;
          if (this.disposed) throw new Error('Pagination disposed');
          let compiled = this.compiled.get(cursor.index);
          const source = compiled?.source ?? await this.options.read(cursor.index, control.signal);
          await checkpoint();
          if (this.disposed) throw new Error('Pagination disposed');
          if (!source) { screen.eof = true; break; }
          if (cursor.lid && cursor.lid !== source.lid) throw new Error('Source cursor belongs to another leaf');
          if (cursor.offset < 0 || cursor.offset > source.text.length) throw new Error('Invalid source offset');
          if (cursor.offset === source.text.length) { cursor = { index: cursor.index + 1, offset: 0 }; continue; }
          const renderStart = performance.now();
          let root = compiled ? this.fromHtml(compiled.html) : this.options.render(source);
          this.metrics.maxRenderMs = Math.max(this.metrics.maxRenderMs, performance.now() - renderStart);
          this.scratch = root;
          await checkpoint();
          if (!compiled) {
            const atomic = ['table', 'code', 'image'].includes(source.kind) || root.classList.contains('formula-display');
            const projection = projectMarkdownDom(root);
            compiled = { source: { lid: source.lid, text: source.text, kind: source.kind }, html: root.outerHTML, semantic: projection?.text ?? '', spans: [], atomic };
            this.compiled.set(cursor.index, compiled);
            this.reserve(cursor.index);
            await checkpoint();
            // A formula is one source object, including when it flows inline;
            // its selection/decorations use the complete canonical object.
            if (projection && source.kind !== 'formula') {
              const alignmentStart = performance.now();
              compiled.spans = await alignMarkdownSource(source.text, projection.text, checkpoint);
              this.metrics.maxAlignmentMs = Math.max(this.metrics.maxAlignmentMs, performance.now() - alignmentStart);
              this.metrics.alignments++;
            }
          }
          this.reserve(cursor.index);
          if ((compiled.atomic || source.kind === 'formula') && cursor.offset) {
            cursor.offset = 0;
            if (!pageIndex && !page.fragments.length) screen.start = { ...cursor };
          }
          let semanticStart = compiled.atomic ? 0 : semanticAtSource(compiled, cursor.offset);
          if (!compiled.atomic) {
            const projection = projectMarkdownDom(root);
            const atomic = projection.pieces.find(p => p.kind === 'atomic' && p.start < semanticStart && p.end > semanticStart);
            if (atomic) {
              semanticStart = atomic.start;
              cursor.offset = sourceAtSemantic(compiled, semanticStart);
              if (!page.fragments.length && !pageIndex) screen.start = { ...cursor };
            }
          }
          // Grow only the candidate prefix, never lay out a whole long leaf.
          // The original HTML is retained so emphasis/list/formula context survives.
          const semanticLimit = control.end?.index === cursor.index ? semanticAtSource(compiled, control.end.offset) : compiled.semantic.length;
          let candidateEnd = Math.min(semanticLimit, semanticStart + 768);
          let projection: DomProjection | null, overflow: number, length: number;
          let container: HTMLElement = article;
          if (root.classList.contains('focus-flow-leaf')) {
            const previous = page.fragments.at(-1);
            if (previous?.element.parentElement?.classList.contains('focus-flow') && shouldJoinReaderFlow(this.compiled.get(previous.index)!.source, source)) {
              container = previous.element.parentElement;
            } else {
              container = document.createElement('p'); container.className = 'flow-paragraph focus-flow';
              if (cursor.offset) container.classList.add('focus-continuation');
              article.append(container);
            }
          }
          for (;;) {
            if (!compiled.atomic) {
              const fullProjection = projectMarkdownDom(root);
              const atomicEnd = fullProjection.pieces.find(p => p.kind === 'atomic' && p.start < candidateEnd && p.end > candidateEnd);
              if (atomicEnd) candidateEnd = atomicEnd.end;
              cutSuffix(root, fullProjection, candidateEnd);
              cutPrefix(root, projectMarkdownDom(root), semanticStart);
            }
            root.dataset.lid = source.lid; root.dataset.sourceStart = `${cursor.offset}`;
            if (semanticStart) root.classList.add('focus-continuation');
            container.append(root);
            await this.options.ready?.(root, source, layout);
            this.pendingResources = undefined;
            // Yield when this bounded batch has used its time slice, then
            // measure only the current candidate with fonts/images ready.
            await checkpoint();
            if (this.disposed) throw new Error('Pagination disposed');
            this.reserve(cursor.index);
            const right = article.getBoundingClientRect().right + geometry.gap / 2;
            projection = compiled.atomic ? null : projectMarkdownDom(root);
            this.metrics.geometryProbes++;
            overflow = compiled.atomic ? (root.getBoundingClientRect().left >= right ? 0 : 1) : firstOverflow(root, projection!, right);
            length = projection?.text.length ?? 1;
            if (compiled.atomic || overflow < length || candidateEnd === semanticLimit) break;
            candidateEnd = Math.min(semanticLimit, candidateEnd + 768);
            root.remove();
            this.pendingResources = undefined;
            root = this.fromHtml(compiled.html); this.scratch = root;
            this.reserve(cursor.index);
            await checkpoint();
          }
          if (overflow === 0 && page.fragments.length) {
            root.remove(); if (container !== article && !container.childNodes.length) container.remove();
            this.pendingResources = undefined;
            this.scratch = null; full = true; break;
          }
          if (overflow === 0 && length) throw new Error(`Content cannot fit the page: ${source.lid}`);
          let end = control.end?.index === cursor.index ? control.end.offset : source.text.length;
          if (overflow < length) {
            cutSuffix(root, projection!, overflow);
            end = sourceAtSemantic(compiled, semanticStart + overflow);
            full = true;
          }
          if (end <= cursor.offset) throw new Error(`Pagination made no source progress: ${source.lid}`);
          root.dataset.sourceEnd = `${end}`;
          this.pendingResources = undefined;
          this.completedPages.set(root, { parsedBytes: root.outerHTML.length * 2, domNodes: nodeCount(root), decodedImagePixels: imagePixels(root) });
          page.fragments.push({ index: cursor.index, lid: source.lid, start: cursor.offset, end, semanticStart, semanticEnd: compiled.atomic ? 0 : semanticStart + overflow, element: root });
          this.scratch = null;
          cursor = end === source.text.length ? { index: cursor.index + 1, offset: 0 } : { index: cursor.index, offset: end, lid: source.lid };
          // Small inline leaves share a bounded 4ms task slice.
          await checkpoint();
        }
        if (!page.fragments.length) { article.remove(); screen.pages.pop(); this.pendingResources = undefined; }
        else this.completedPages.set(article, { parsedBytes: article.outerHTML.length * 2, domNodes: nodeCount(article), decodedImagePixels: imagePixels(article) });
        if (screen.eof || (control.end && compareCursor(cursor, control.end) >= 0)) break;
      }
      await checkpoint();
      screen.end = cursor;
      this.decorate(screen);
      this.pendingResources = undefined;
      this.completedPages.clear();
      this.reserve();
      element.hidden = true; element.style.visibility = '';
      this.screens.push(screen); this.pending = null;
      this.screenResources.set(screen.id, domResources(element));
      this.metrics.preparations++;
      return screen;
    } catch (error) {
      element.remove(); element.replaceChildren(); screen.pages.length = 0;
      throw error;
    } finally {
      this.scratch = null; this.pending = null; this.pendingResources = undefined; this.completedPages.clear(); this.busy = false; this.pruneCompiled();
    }
  }
  private fromHtml(html: string) {
    const template = document.createElement('template'); template.innerHTML = html;
    return template.content.firstElementChild as HTMLElement;
  }
  /** Constant-size state change; no layout reads, parsing, or source acquisition. */
  show(screen: PreparedScreen) {
    if (!this.screens.includes(screen) || !screen.pages.length) throw new Error('Screen is not resident');
    if (this.current) { this.current.element.hidden = true; this.current.element.inert = true; this.current.element.setAttribute('aria-hidden', 'true'); }
    screen.element.hidden = false; screen.element.inert = false; screen.element.removeAttribute('aria-hidden');
    this.current = screen; this.metrics.shows++;
  }
  /** Canonical prose selection. Atomic objects expose page ranges only; their
   * finer selection adapters remain with the existing asset handlers (FP5). */
  sourceRanges(fragment: PageFragment, range: Range) {
    const compiled = this.compiled.get(fragment.index)!;
    if (compiled.source.kind === 'image') return [];
    if (compiled.source.kind === 'formula') return [{ start: fragment.start, end: fragment.end }];
    const projection = projectMarkdownDom(fragment.element);
    const start = projection.offsetForPoint(range.startContainer, range.startOffset, 'start') + fragment.semanticStart;
    const end = projection.offsetForPoint(range.endContainer, range.endOffset, 'end') + fragment.semanticStart;
    return bindMarkdownSourceMap(compiled.source.text, { ...projection, text: compiled.semantic }, compiled.spans).sourceRangesForSemanticRange(start, end);
  }
  source(lid: string): PaginationSource | undefined {
    const fragment = this.current?.pages.flatMap(p => p.fragments).find(f => f.lid === lid);
    return fragment ? this.compiled.get(fragment.index)?.source : undefined;
  }
  visibleRanges() {
    return this.current?.pages.flatMap(p => p.fragments.map(f => ({ lid: f.lid, start: f.start, end: f.end }))) ?? [];
  }
  selection(range: Range) {
    const screen = this.current;
    if (!screen || screen.element.inert || screen.element.hidden || !screen.element.contains(range.startContainer) || !screen.element.contains(range.endContainer)) return null;
    const ranges: { lid: string; start: number; end: number }[] = [];
    for (const fragment of screen.pages.flatMap(p => p.fragments)) {
      if (!range.intersectsNode(fragment.element)) continue;
      const local = document.createRange(); local.selectNodeContents(fragment.element);
      if (fragment.element.contains(range.startContainer)) local.setStart(range.startContainer, range.startOffset);
      if (fragment.element.contains(range.endContainer)) local.setEnd(range.endContainer, range.endOffset);
      for (const part of this.sourceRanges(fragment, local)) {
        const previous = ranges.at(-1);
        if (previous?.lid === fragment.lid && previous.end === part.start) previous.end = part.end;
        else ranges.push({ lid: fragment.lid, ...part });
      }
    }
    return { ranges, resolvedText: ranges.map(r => this.source(r.lid)!.text.slice(r.start, r.end)).join('') };
  }
  private semanticRanges(fragment: PageFragment, start: number, end: number) {
    const compiled = this.compiled.get(fragment.index)!;
    const projection = projectMarkdownDom(fragment.element);
    if (compiled.source.kind === 'formula') return { projection, ranges: start < fragment.end && end > fragment.start ? [{ start: 0, end: projection.text.length }] : [] };
    const map = bindMarkdownSourceMap(compiled.source.text, { ...projection, text: compiled.semantic }, compiled.spans);
    const ranges = map.semanticRangesForSourceRange(Math.max(start, fragment.start), Math.min(end, fragment.end))
      .map(r => ({ start: Math.max(0, r.start - fragment.semanticStart), end: Math.min(projection.text.length, r.end - fragment.semanticStart) })).filter(r => r.end > r.start);
    return { projection, ranges };
  }
  /** Fragment rects, never the bounding box spanning both columns. */
  rangeRects(lid: string, start: number, end: number, screen = this.current) {
    const rects: DOMRect[] = [];
    for (const f of screen?.pages.flatMap(p => p.fragments) ?? []) {
      if (f.lid !== lid || end <= f.start || start >= f.end) continue;
      const { projection, ranges } = this.semanticRanges(f, start, end);
      for (const r of ranges) {
        const range = document.createRange(); range.setStart(...point(projection, r.start, f.element)); range.setEnd(...point(projection, r.end, f.element));
        rects.push(...Array.from(range.getClientRects()).filter(rect => rect.width && rect.height));
      }
    }
    return rects;
  }
  decorate(screen: PreparedScreen) {
    if (!this.options.decorations) return;
    screen.element.querySelectorAll('.focus-annotation-marker').forEach(el => el.remove());
    let screenBox: DOMRect | undefined;
    for (const page of screen.pages) for (const fragment of page.fragments) {
      const { element, lid, start, end } = fragment;
      const decoration = this.options.decorations(lid, this.compiled.get(fragment.index)!.source);
      if (!decoration.marks.length && !decoration.markers.length && !element.querySelector('mark.hl-mark, mark.source-focus-mark')) continue;
      for (const mark of element.querySelectorAll('mark.hl-mark, mark.source-focus-mark')) mark.replaceWith(...mark.childNodes);
      element.normalize();
      const projection = projectMarkdownDom(element);
      const ranges = decoration.marks.flatMap(m => this.semanticRanges(fragment, m.start, m.end).ranges.map(r => ({ ...r, className: m.className })));
      markMarkdownDomSemanticRanges(projection, ranges);
      const markers = decoration.markers.filter(m => m.start === undefined ? start === 0 : m.start < end && (m.end ?? m.start + 1) > start);
      if (!markers.length) continue;
      const marker = markers[0];
      const rect = marker.start === undefined ? Array.from(element.getClientRects()).find(r => r.height && r.width)
        : this.rangeRects(lid, Math.max(start, marker.start), Math.min(end, marker.end ?? marker.start + 1), screen)[0];
      if (!rect) continue;
      screenBox ??= screen.element.getBoundingClientRect();
      const button = document.createElement('button'); button.className = 'annotation-marker focus-annotation-marker';
      button.dataset.annotationLid = lid; button.dataset.annotationId = marker.id;
      button.setAttribute('data-reader-selection-ignore', ''); button.setAttribute('data-note-placement-ignore', '');
      button.setAttribute('aria-label', `查看此处 ${markers.length} 条批注`); button.textContent = `✎${markers.length}`;
      button.style.top = `${rect.top - screenBox.top}px`;
      button.style.left = `${Math.max(0, page.element.getBoundingClientRect().left - screenBox.left)}px`;
      screen.element.append(button);
    }
    this.metrics.decorations++;
    if (this.screenResources.has(screen.id)) this.screenResources.set(screen.id, domResources(screen.element));
  }
  anchor(): SourceCursor | null {
    const fragment = this.current?.pages[0]?.fragments[0];
    return fragment ? { index: fragment.index, lid: fragment.lid, offset: fragment.start } : null;
  }
  readingPosition() {
    const cursor = this.anchor(); if (!cursor?.lid) return null;
    const source = this.compiled.get(cursor.index)?.source;
    const length = (source?.text.codePointAt(cursor.offset) ?? 0) > 0xffff ? 2 : 1;
    return { lid: cursor.lid, top: 0, ...(source && ['paragraph', 'chapter', 'section'].includes(source.kind) ? {
      textPosition: { start: cursor.offset, end: Math.min(source.text.length, cursor.offset + length), text: source.text.slice(cursor.offset, cursor.offset + length), top: 0 },
    } : {}) };
  }
  async reflow(layout: PaginationLayout, anchor = this.anchor()) {
    if (!anchor) return null;
    const previous = this.current;
    const screen = await this.prepare(anchor, layout);
    this.show(screen);
    for (const old of this.screens.slice()) if (old !== screen) this.release(old);
    return { screen, previousId: previous?.id };
  }
  dispose() {
    this.disposed = true; this.current = null;
    for (const screen of this.screens.slice()) this.release(screen);
    this.pending?.element.remove(); this.compiled.clear();
  }
}
