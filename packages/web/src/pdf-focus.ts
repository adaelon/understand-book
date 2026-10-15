import type { PDFDocumentProxy, PDFPageProxy, RenderTask } from 'pdfjs-dist';
import { TextLayer } from 'pdfjs-dist/legacy/build/pdf.mjs';
import type { PdfSourceMap } from './api';
import { pdfPageVisualSize } from './pdf-annotation-projection';
import { PDF_MAX_BACKING_PIXELS, pdfFocusRasterSize, pdfFocusWindow, pdfSpreadStart, type PdfFocusGeometry } from './pdf-rendering';
import type { PdfReadingAnchor } from './useReadingContinuity';
import type { TurnScreen } from './reader-focus-turn';

type PageInfo = PdfSourceMap['pages'][number];
type Page = { info: PageInfo; element: HTMLElement; canvas: HTMLCanvasElement; text: HTMLElement; annotations: HTMLElement;
  pdf?: PDFPageProxy; textTask?: TextLayer; task?: RenderTask; width: number; height: number; pixels: number };
export type PdfSpread = TurnScreen & { id: number; key: string; geometry: PdfFocusGeometry; pages: Page[]; ready: boolean };
const nextTask = () => new Promise<void>(resolve => setTimeout(resolve, 0));

/** Owns every focus canvas, including unfinished work and the old visible geometry.
 * Preparation is serial; input only selects already completed spreads. */
export class PdfFocusReader {
  readonly pagination = { visible: () => this.current };
  readonly metrics = { preparations: 0, canvases: 0, textLayers: 0, textChunks: 0, decorations: 0, misses: 0, turns: 0 };
  readonly peak = { pages: 0, backingPixels: 0, domNodes: 0, preparationTasks: 0 };
  private screens: PdfSpread[] = [];
  private current?: PdfSpread;
  private geometry?: PdfFocusGeometry;
  private target = 0;
  private anchor?: PdfReadingAnchor;
  private anchorPage = 0;
  private direction = 1;
  private sequence = 0;
  private serial = 0;
  private running = false;
  private paused = false;
  private selectionPaused = false;
  private visible = true;
  private disposed = false;
  private scheduled?: ReturnType<typeof setTimeout>;
  private scheduledFrame = 0;
  private resume?: () => void;
  private annotationRevision = 0;
  private appliedRevision = 0;
  private shield = document.createElement('div');
  private readonly rasterDpr = window.devicePixelRatio || 1;
  error: string | null = null;
  constructor(private host: HTMLElement, private doc: PDFDocumentProxy, private pages: PageInfo[], private sourceKey: string,
    private hooks: {
      changed(): void;
      decorate(page: PageInfo, layer: HTMLElement): void;
      releaseDecoration(layer: HTMLElement): void;
      externalResources(): { pages: number; backingPixels: number };
    }) {
      host.classList.add('pdf-focus-host');
      this.shield.className = 'pdf-focus-interaction-shield'; this.shield.setAttribute('aria-hidden', 'true'); host.append(this.shield);
    }

  resources() {
    const external = this.hooks.externalResources();
    return { pages: this.screens.reduce((n, s) => n + s.pages.length, external.pages),
      backingPixels: this.screens.reduce((n, s) => n + s.pages.reduce((m, p) => m + p.canvas.width * p.canvas.height, 0), external.backingPixels),
      domNodes: this.screens.reduce((n, s) => n + s.element.getElementsByTagName('*').length + 1, 0), preparationTasks: Number(this.running) };
  }
  private sample() { const r = this.resources(); for (const k of Object.keys(this.peak) as (keyof typeof this.peak)[]) this.peak[k] = Math.max(this.peak[k], r[k]); }
  snapshot() { return { current: this.current && { id: this.current.id, start: this.current.start, eof: this.current.eof,
    pages: this.current.pages.map(p => p.info.pageIndex), geometry: this.current.geometry },
    ready: this.screens.filter(s => s.ready).map(s => ({ start: s.start.index, columns: s.geometry.columns })),
    metrics: { ...this.metrics }, resources: this.resources(), peak: { ...this.peak }, error: this.error,
    waiting: !!this.geometry && (!this.current || this.current.key !== this.key(this.target, this.geometry)) };
  }
  elements() { return new Map(this.current?.pages.map(p => [p.info.pageIndex, p.element]) ?? []); }
  private key(start: number, g: PdfFocusGeometry) { return `${start}:${g.width}:${g.height}:${g.columns}:${g.zoom}`; }
  private cached(start: number) { return this.screens.find(s => s.ready && this.geometry && s.key === this.key(start, this.geometry)); }
  adjacent(direction: number) { return this.current && this.cached(this.current.start.index + Math.sign(direction) * this.current.geometry.columns); }
  cancelPending() {
    if (!this.current || !this.geometry) return;
    this.target = pdfSpreadStart(this.current.start.index, this.geometry.columns); this.anchor = undefined; this.sequence++;
    this.error = null; this.hooks.changed(); this.schedule();
  }
  move(direction: number) {
    if (!this.current || !this.geometry) return false;
    this.direction = Math.sign(direction);
    const target = this.current.start.index + this.direction * this.geometry.columns;
    if (target < 0 || target >= this.pages.length) { this.cancelPending(); return false; }
    this.target = target; this.anchorPage = target; this.anchor = undefined; this.sequence++;
    const ready = this.cached(target);
    if (ready) { this.metrics.turns++; this.show(ready); }
    else { this.metrics.misses++; this.hooks.changed(); }
    this.schedule(); return !!ready;
  }
  open(anchor: PdfReadingAnchor, geometry: PdfFocusGeometry) {
    this.visible = true; this.geometry = geometry; this.anchor = anchor; this.anchorPage = anchor.pageIndex;
    this.target = pdfSpreadStart(Math.min(this.pages.length - 1, anchor.pageIndex), geometry.columns);
    this.sequence++; this.error = null;
    const ready = this.cached(this.target) ?? this.regroup(geometry);
    if (ready) { this.show(ready); this.restoreAnchor(anchor); this.anchor = undefined; }
    this.schedule(); this.hooks.changed();
  }
  setPaused(paused: boolean) { this.paused = paused; if (paused) this.stopDrawing(); this.wake(); }
  setSelectionPaused(paused: boolean) { this.selectionPaused = paused; if (paused) this.stopDrawing(); this.wake(); }
  setVisible(visible: boolean) { this.visible = visible; if (!visible) this.stopDrawing(); this.wake(); }
  private stopDrawing() {
    // onContinue can already have queued a PDF.js RAF. Cancelling an unfinished
    // candidate prevents that queued draw from running inside a new interaction.
    for (const s of this.screens) if (!s.ready) for (const p of s.pages) p.task?.cancel();
  }
  private blocked() { return this.paused || this.selectionPaused || !this.visible; }
  private wake() { if (!this.blocked()) this.schedule(); }
  private async gate() {
    if (this.disposed) throw new Error('PDF reader disposed');
    while (this.blocked() && !this.disposed) await new Promise<void>(resolve => { this.resume = resolve; });
    if (this.disposed) throw new Error('PDF reader disposed');
  }
  refreshDecorations() { this.annotationRevision++; this.schedule(); }
  private decorate(screen: PdfSpread) {
    for (const page of screen.pages) { this.metrics.decorations++; this.hooks.decorate(page.info, page.annotations); }
  }
  private show(screen: PdfSpread) {
    this.current = screen;
    for (const s of this.screens) {
      s.element.hidden = s !== screen; s.setInteractive!(s === screen);
      if (s !== screen) s.element.setAttribute('aria-hidden', 'true'); else s.element.removeAttribute('aria-hidden');
    }
    this.host.style.touchAction = screen.geometry.zoom > 1 ? 'pan-x pan-y pinch-zoom' : 'pan-y pinch-zoom';
    this.sample(); this.hooks.changed();
  }
  private schedule() {
    if (this.disposed || this.scheduled || this.scheduledFrame || this.blocked() || !this.geometry) return;
    // Let the confirmed page reach the compositor before allocating/drawing the
    // next canvases. This also resumes a paused PDF.js continuation after paint.
    this.scheduledFrame = requestAnimationFrame(() => {
      this.scheduledFrame = 0;
      this.scheduled = setTimeout(() => {
        this.scheduled = undefined;
        if (this.blocked() || this.disposed) return;
        this.resume?.(); this.resume = undefined; void this.run();
      }, 0);
    });
  }
  private evict(screen: PdfSpread) {
    this.screens = this.screens.filter(s => s !== screen);
    for (const p of screen.pages) {
      p.task?.cancel(); p.textTask?.cancel(); this.hooks.releaseDecoration(p.annotations);
      p.canvas.width = 0; p.canvas.height = 0; p.text.replaceChildren();
      // PDF.js caches page proxies/operator lists. Release them when no retained
      // geometry uses that page; a cancelled render finishes cleanup in prepare().
      if (!this.screens.some(s => s.pages.some(other => other.pdf === p.pdf))) p.pdf?.cleanup();
    }
    screen.element.remove();
  }
  retainCurrent() {
    this.visible = false; this.sequence++;
    for (const s of [...this.screens]) if (s !== this.current) this.evict(s);
    if (this.current) { this.current.element.inert = true; this.current.element.setAttribute('aria-hidden', 'true'); }
  }
  private reserve(pages: number, pixels: number) {
    const maxPages = Math.max(this.geometry!.columns, this.current?.geometry.columns ?? 1) * 3;
    // Obsolete geometry and the farthest screen yield before a new canvas exists.
    for (const s of [...this.screens].filter(s => s !== this.current).sort((a, b) =>
      Number(b.key !== this.key(b.start.index, this.geometry!)) - Number(a.key !== this.key(a.start.index, this.geometry!)) || Math.abs(b.start.index - this.target) - Math.abs(a.start.index - this.target))) {
      const r = this.resources();
      if (r.pages + pages <= maxPages && r.backingPixels + pixels <= PDF_MAX_BACKING_PIXELS) break;
      this.evict(s);
    }
    const r = this.resources();
    return r.pages + pages <= maxPages && r.backingPixels + pixels <= PDF_MAX_BACKING_PIXELS;
  }
  private sizePage(page: Page, g: PdfFocusGeometry) {
    const visual = pdfPageVisualSize(page.info), scale = Math.min(g.pageWidth / visual.width, g.pageHeight / visual.height) * g.zoom;
    page.width = visual.width * scale; page.height = visual.height * scale;
    Object.assign(page.element.style, { width: `${page.width}px`, height: `${page.height}px`, flex: '0 0 auto' });
    Object.assign(page.text.style, { width: `${visual.width}px`, height: `${visual.height}px`, transform: `scale(${scale})` });
    page.canvas.dataset.rasterScale = String(page.canvas.width / page.width);
  }
  private createScreen(start: number, g: PdfFocusGeometry): PdfSpread {
    const element = document.createElement('div'); element.className = 'pdf-focus-screen'; element.hidden = true;
    element.setAttribute('aria-hidden', 'true'); element.style.pointerEvents = 'none';
    Object.assign(element.style, { display: 'none', width: `${g.width}px`, height: `${g.height}px`, overflow: g.zoom > 1 ? 'auto' : 'hidden',
      touchAction: g.zoom > 1 ? 'pan-x pan-y pinch-zoom' : 'pan-y pinch-zoom' });
    const row = document.createElement('div'); row.className = 'pdf-focus-row'; element.append(row);
    const screen: PdfSpread = { id: ++this.serial, key: this.key(start, g), geometry: g, element, start: { index: start, offset: 0 },
      eof: start + g.columns >= this.pages.length, ready: false, pages: [],
      setInteractive: active => {
        element.inert = false;
        // Only canonical current text participates in native selection. Keep its
        // prepared geometry through motion; pointer ownership blocks dragging
        // until settlement. Changing inherited inert here would restyle every
        // transparent glyph twice per turn. Preview annotations stay inert.
        for (const p of screen.pages) { p.text.style.contentVisibility = screen === this.current ? 'visible' : 'hidden'; p.annotations.inert = !active; }
      } };
    this.screens.push(screen); this.host.append(element); return screen;
  }
  private regroup(g: PdfFocusGeometry) {
    const available = new Map(this.screens.filter(s => s.ready).flatMap(s => s.pages.map(p => [p.info.pageIndex, p] as const)));
    if (!this.pages.slice(this.target, this.target + g.columns).every(p => available.has(p.pageIndex))) return;
    const groups = pdfFocusWindow(this.target, this.pages.length, g.columns, this.direction)
      .map(start => ({ start, pages: this.pages.slice(start, start + g.columns).map(p => available.get(p.pageIndex)) }))
      .filter(group => group.pages.every((p): p is Page => !!p));
    const retained = new Set(groups.flatMap(group => group.pages));
    // Transfer ownership within this task: the old complete screen remains painted
    // until its same canvas/text/annotation nodes have been regrouped and shown.
    for (const s of [...this.screens]) { s.pages = s.pages.filter(p => !retained.has(p)); this.evict(s); }
    for (const group of groups) {
      const screen = this.createScreen(group.start, g);
      for (const page of group.pages as Page[]) { this.sizePage(page, g); screen.pages.push(page); screen.element.firstElementChild!.append(page.element); }
      screen.ready = true; screen.element.dataset.ready = 'true'; screen.element.style.display = '';
    }
    return this.cached(this.target);
  }
  private async prepare(start: number, g: PdfFocusGeometry): Promise<PdfSpread | undefined> {
    const infos = this.pages.slice(start, start + g.columns);
    const plans = infos.map(info => {
      const visual = pdfPageVisualSize(info), scale = Math.min(g.pageWidth / visual.width, g.pageHeight / visual.height) * g.zoom;
      return { info, width: visual.width * scale, height: visual.height * scale, raster: pdfFocusRasterSize(visual, this.rasterDpr) };
    });
    const pixels = plans.reduce((n, p) => n + p.raster.width * p.raster.height, 0);
    if (!this.reserve(infos.length, pixels)) return;
    const screen = this.createScreen(start, g), element = screen.element, row = element.firstElementChild!;
    this.metrics.preparations++;
    for (const p of plans) {
      const shell = document.createElement('section'); shell.className = 'pdf-page-shell'; shell.dataset.pageIndex = String(p.info.pageIndex);
      Object.assign(shell.style, { width: `${p.width}px`, height: `${p.height}px`, flex: '0 0 auto' });
      const canvas = document.createElement('canvas'); canvas.className = 'pdf-page-canvas';
      canvas.width = p.raster.width; canvas.height = p.raster.height;
      canvas.dataset.backingPixels = String(canvas.width * canvas.height);
      const text = document.createElement('div'); text.className = 'pdf-text-layer';
      text.style.pointerEvents = 'auto';
      const annotations = document.createElement('div'); annotations.className = 'pdf-user-annotation-layer';
      shell.append(canvas, text, annotations); row.append(shell);
      const page = { ...p, element: shell, canvas, text, annotations, pixels: canvas.width * canvas.height };
      this.sizePage(page, g); screen.pages.push(page);
    }
    this.sample();
    try {
      for (const p of screen.pages) {
        await this.gate(); if (!this.screens.includes(screen)) return;
        p.pdf = await this.doc.getPage(p.info.pageIndex + 1);
        await this.gate(); if (!this.screens.includes(screen)) return;
        const viewport = p.pdf.getViewport({ scale: 1 }), raster = pdfFocusRasterSize(pdfPageVisualSize(p.info), this.rasterDpr);
        this.metrics.canvases++;
        p.task = p.pdf.render({ canvas: p.canvas, viewport, transform: [raster.scale, 0, 0, raster.scale, 0, 0] });
        p.task.onContinue = (continuation: () => void) => { void this.gate().then(() => { if (this.screens.includes(screen)) continuation(); }).catch(() => {}); };
        await p.task.promise; p.task = undefined;
        await nextTask(); await this.gate(); if (!this.screens.includes(screen)) return;
        this.metrics.textLayers++;
        p.text.style.setProperty('--scale-factor', String(viewport.scale));
        p.text.style.setProperty('--user-unit', '1');
        p.text.style.setProperty('--total-scale-factor', String(viewport.scale));
        const source = p.pdf.streamTextContent({ includeMarkedContent: true, disableNormalization: true }).getReader();
        const textContentSource = new ReadableStream({
          pull: async controller => {
            await this.gate(); const chunk = await source.read(); await this.gate();
            if (chunk.done || !this.screens.includes(screen)) { controller.close(); return; }
            this.metrics.textChunks++; controller.enqueue(chunk.value);
          },
          cancel: reason => source.cancel(reason),
        }, { highWaterMark: 0 });
        const div = document.createElement('div'); div.className = 'textLayer';
        p.textTask = new TextLayer({ textContentSource, container: div, viewport });
        await p.textTask.render(); await nextTask(); await this.gate();
        if (!this.screens.includes(screen)) return;
        p.text.replaceChildren(div);
        p.text.style.contentVisibility = 'hidden';
        await nextTask();
      }
      await this.gate(); if (!this.screens.includes(screen)) return;
      // In-progress canvases stay outside browser paint. Once drawing is done,
      // resolve each selection layer separately, without combining its layout
      // with PDF.js drawing in the same rendering task.
      element.style.display = '';
      for (const p of screen.pages) {
        await nextTask(); await this.gate(); if (!this.screens.includes(screen)) return;
        p.text.style.contentVisibility = 'visible';
        (p.text.firstElementChild?.lastElementChild ?? p.text).getBoundingClientRect();
        p.text.style.contentVisibility = 'hidden';
      }
      this.decorate(screen); screen.ready = true; element.dataset.ready = 'true'; this.sample(); return screen;
    } catch (e) {
      if (!this.disposed && this.screens.includes(screen) && !(e instanceof Error && e.name === 'RenderingCancelledException')) this.error = String(e);
      this.evict(screen); return;
    } finally {
      if (!this.screens.includes(screen)) for (const p of screen.pages) {
        if (!this.screens.some(s => s.pages.some(other => other.pdf === p.pdf))) p.pdf?.cleanup();
      }
    }
  }
  private async run() {
    if (this.running || this.blocked() || !this.geometry || this.disposed) return;
    this.running = true; this.sample();
    const sequence = this.sequence;
    try {
      const g = this.geometry;
      if (this.appliedRevision !== this.annotationRevision) {
        for (const s of this.screens.filter(s => s.ready)) this.decorate(s);
        this.appliedRevision = this.annotationRevision;
      }
      for (const start of pdfFocusWindow(this.target, this.pages.length, g.columns, this.direction)) {
        await this.gate(); if (sequence !== this.sequence) break;
        const screen = this.cached(start) ?? await this.prepare(start, g);
        if (sequence !== this.sequence || this.disposed) break;
        if (!screen) break;
        if (start === this.target && this.current !== screen) {
          this.show(screen); if (this.anchor) this.restoreAnchor(this.anchor); this.anchor = undefined;
          // Publish the complete screen before spending another task on neighbours.
          await new Promise<void>(resolve => requestAnimationFrame(() => setTimeout(resolve, 0)));
        }
      }
      if (this.geometry === g) {
        const keep = new Set(pdfFocusWindow(this.target, this.pages.length, g.columns, this.direction));
        for (const s of [...this.screens]) if (s !== this.current && (s.key !== this.key(s.start.index, g) || !keep.has(s.start.index))) this.evict(s);
      }
    } catch (error) {
      // Closing the reader can release a paused gate or the post-publish frame.
      // The cancelled preparation is finished; there is no live reader to notify.
      if (!this.disposed) throw error;
    } finally {
      this.running = false; if (!this.disposed) this.hooks.changed();
      if (sequence !== this.sequence) this.schedule();
    }
  }
  captureAnchor(): PdfReadingAnchor | null {
    const screen = this.current; if (!screen) return null;
    const root = screen.element, g = screen.geometry;
    const probe = Math.min(Math.max(g.height * .28, 96), Math.max(0, g.height - 24));
    const p = screen.pages.find(p => p.info.pageIndex === this.anchorPage) ?? screen.pages[0];
    const scrollTop = g.zoom > 1 ? root.scrollTop : 0;
    const scrollWidth = Math.max(g.width, screen.pages.reduce((n, p) => n + p.width, 48 + (screen.pages.length - 1) * 32));
    return { surface: 'pdf', sourceKey: this.sourceKey, pageIndex: p.info.pageIndex,
      pageRatio: Math.max(0, Math.min(1, (probe + scrollTop - 24) / p.height)), probeRatio: probe / Math.max(1, g.height),
      horizontalRatio: ((g.zoom > 1 ? root.scrollLeft : 0) + g.width / 2) / scrollWidth, anchorLid: null, zoom: g.zoom };
  }
  private restoreAnchor(anchor: PdfReadingAnchor) {
    const screen = this.current, page = screen?.pages.find(p => p.info.pageIndex === anchor.pageIndex);
    if (!screen || !page) return;
    const root = screen.element, rect = root.getBoundingClientRect(), pr = page.element.getBoundingClientRect();
    root.scrollTop += pr.top + pr.height * anchor.pageRatio - rect.top - rect.height * anchor.probeRatio;
    if (anchor.horizontalRatio !== null) root.scrollLeft = anchor.horizontalRatio * root.scrollWidth - root.clientWidth / 2;
  }
  dispose() {
    this.disposed = true; this.sequence++; clearTimeout(this.scheduled); cancelAnimationFrame(this.scheduledFrame); this.resume?.(); this.resume = undefined;
    for (const s of [...this.screens]) this.evict(s);
    this.shield.remove();
    this.current = undefined;
  }
}
