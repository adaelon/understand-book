import { ReaderPagination, compareCursor, type PaginationLayout, type PaginationOptions, type PreparedScreen, type SourceCursor } from './reader-pagination';
import { FocusSource } from './reader-focus-source';
import { renderPaginationSource } from './reader-pagination-render';
import { yieldReaderTask } from './reader-cooperative';

export function abortable<T>(task: Promise<T>, signal: AbortSignal): Promise<T> {
  signal.throwIfAborted();
  return new Promise((resolve, reject) => {
    const abort = () => reject(signal.reason);
    signal.addEventListener('abort', abort, { once: true });
    task.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort));
  });
}
type Request = { cursor: SourceCursor; before?: boolean; layout: PaginationLayout; replace?: boolean };
export interface FocusState { phase: 'idle' | 'loading' | 'ready' | 'waiting' | 'error'; error: string | null; preparing: boolean }
const same = (a: SourceCursor, b: SourceCursor) => compareCursor(a, b) === 0;
const geometryKey = (layout: PaginationLayout) => JSON.stringify(layout);

/** One foreground request and one serial preparation loop. The bounded boundary
 * journal is metadata only; text/HTML/DOM ownership stays in source/pagination. */
export class FocusReader {
  readonly pagination: ReaderPagination;
  readonly state: FocusState = { phase: 'idle', error: null, preparing: false };
  readonly metrics = { inputs: 0, misses: 0, cancellations: 0, imageFailures: 0, turns: 0, maxWaitMs: 0 };
  private boundaries: { start: SourceCursor; end: SourceCursor }[] = [];
  private request: Request | null = null;
  private failed: Request | null = null;
  private direction = 1;
  private requestedLayout?: PaginationLayout;
  private active?: AbortController;
  private running = false;
  private timer?: ReturnType<typeof setTimeout>;
  private scheduledFrame = 0;
  private paused = false;
  private selectionPaused = false;
  private decorationDirty = false;
  private visible = true;
  private disposed = false;
  private resume?: () => void;
  private revision = 0;
  private waitStarted = 0;
  private mathFonts?: Promise<FontFace[]>;
  private readyFonts = new WeakMap<import('./reader-pagination').PaginationSource, Set<string>>();
  private alternate: PreparedScreen | null = null;
  constructor(host: HTMLElement, readonly source: FocusSource, private changed: () => void = () => {}, decorations?: PaginationOptions['decorations']) {
    this.pagination = new ReaderPagination({ host,
      decorations,
      read: (index, signal) => source.take(index, signal, bytes => this.pagination.reserveSource(bytes)),
      render: renderPaginationSource,
      externalResources: () => ({ sourceBytes: source.bytes(), parsedBytes: source.parsedBytes() + this.boundaries.length * 128 }),
      ready: async (root, text, layout) => {
        const signal = this.active!.signal;
        if (document.fonts && !this.readyFonts.get(text)?.has(layout.fontFamily)) {
          await abortable(Promise.all([400, 600].map(w => document.fonts.load(`${w} ${layout.fontSize}px ${layout.fontFamily}`, text.text))), signal);
          const families = this.readyFonts.get(text) ?? new Set<string>();
          families.add(layout.fontFamily); this.readyFonts.set(text, families);
        }
        // KaTeX's fixed font set must be ready before the first math page.
        // A later Size2 face used only by a background candidate otherwise
        // completes during a turn and invalidates already displayed math layout.
        if (document.fonts && root.querySelector('.katex')) {
          this.mathFonts ??= Promise.all(Array.from(document.fonts).filter(face => /KaTeX_/.test(face.family)).map(face => face.load()));
          await abortable(this.mathFonts, signal);
        }
        await Promise.all(Array.from(root.querySelectorAll('img')).map(async img => {
          try { await abortable(img.decode(), signal); }
          catch (error) {
            signal.throwIfAborted();
            this.metrics.imageFailures++;
            const placeholder = document.createElement('span');
            placeholder.className = 'focus-image-unavailable';
            placeholder.textContent = img.alt ? `图片暂不可用：${img.alt}` : '图片暂不可用';
            img.removeAttribute('src'); img.replaceWith(placeholder);
          }
        }));
      },
    });
  }
  private notify() { if (!this.disposed) this.changed(); }
  private cancel() { this.revision++; if (this.active) { this.metrics.cancellations++; this.active.abort(); } }
  open(cursor: SourceCursor, layout: PaginationLayout) {
    const current = this.pagination.visible();
    const reusable = this.pagination.resident().find(screen => same(screen.start, cursor) && geometryKey(screen.layout) === geometryKey(layout));
    if (reusable) {
      this.cancel(); this.request = null; this.failed = null; this.requestedLayout = { ...layout };
      this.pagination.show(reusable);
      this.alternate = current && current !== reusable && same(current.start, cursor) ? current : this.alternate;
      for (const screen of this.pagination.resident()) if (screen !== reusable && screen !== this.alternate && geometryKey(screen.layout) !== geometryKey(layout)) this.pagination.release(screen);
      this.state.phase = 'ready'; this.state.error = null; this.notify(); this.schedule(); return;
    }
    this.cancel();
    this.requestedLayout = { ...layout };
    this.request = { cursor: { ...cursor }, layout: { ...layout }, replace: true };
    this.failed = null; this.waitStarted = performance.now();
    this.state.phase = this.pagination.visible() ? 'waiting' : 'loading'; this.state.error = null;
    this.notify(); this.schedule();
  }
  private neighbor(screen: PreparedScreen, direction: number) {
    return this.pagination.resident().find(s => geometryKey(s.layout) === geometryKey(screen.layout) && (direction > 0 ? same(s.start, screen.end) : same(s.end, screen.start)));
  }
  adjacent(direction: number) {
    const current = this.pagination.visible();
    return current ? this.neighbor(current, direction) : undefined;
  }
  /** True means the target was already ready at input time. Publication itself
   * only switches resident DOM; all replenishment begins in a later task. */
  move(direction: number): boolean {
    const current = this.pagination.visible();
    if (!current || this.disposed) return false;
    this.direction = direction > 0 ? 1 : -1;
    if ((direction > 0 && current.eof) || (direction < 0 && same(current.start, { index: 0, offset: 0 }))) return true;
    this.metrics.inputs++;
    if (this.alternate && this.pagination.resident().includes(this.alternate)) this.pagination.release(this.alternate);
    this.alternate = null;
    const layout = this.requestedLayout ?? current.layout;
    const reflow = geometryKey(layout) !== geometryKey(current.layout);
    const target = this.neighbor(current, direction);
    // A ready turn pauses the serial loop; its in-flight neighbor can still
    // serve the new window. Restarting it on every turn can starve a complex
    // page. Foreground replacement and unavailable targets still supersede it.
    if (!target || reflow || this.request) this.cancel();
    this.failed = null; this.state.error = null;
    if (target) {
      // A new reading target supersedes the old target, not the requested font
      // or viewport. Keep a usable old-geometry neighbor while reflow finishes.
      this.request = reflow ? { cursor: { ...target.start }, layout, replace: true } : null;
      this.waitStarted = performance.now();
      this.pagination.show(target); this.metrics.turns++; this.state.phase = reflow ? 'waiting' : 'ready';
      this.notify(); this.schedule(); return true;
    }
    this.metrics.misses++; this.waitStarted = performance.now();
    this.request = { cursor: { ...(direction > 0 ? current.end : current.start) }, before: direction < 0, layout, replace: reflow };
    this.state.phase = 'waiting'; this.notify(); this.schedule(); return false;
  }
  retry() {
    this.cancel(); this.request = this.failed; this.failed = null;
    this.state.error = null;
    this.state.phase = this.request ? (this.pagination.visible() ? 'waiting' : 'loading') : (this.pagination.visible() ? 'ready' : 'idle');
    this.waitStarted = performance.now(); this.notify(); this.schedule();
  }
  setPaused(paused: boolean) {
    this.paused = paused;
    this.resumeWork();
  }
  setSelectionPaused(paused: boolean) { this.selectionPaused = paused; this.resumeWork(); }
  private resumeWork() { if (!this.paused && !this.selectionPaused) this.schedule(); }
  refreshDecorations() { this.decorationDirty = true; this.schedule(); }
  setVisible(visible: boolean) {
    this.visible = visible;
    if (!visible) this.cancel(); else this.schedule();
  }
  retainCurrent() {
    this.setVisible(false); this.request = null; this.failed = null; this.boundaries = []; this.alternate = null;
    for (const screen of this.pagination.resident()) if (screen !== this.pagination.visible()) this.pagination.release(screen);
    const current = this.pagination.visible();
    if (current) { current.element.inert = true; current.element.setAttribute('aria-hidden', 'true'); }
    this.source.clear();
  }
  private async checkpoint(signal: AbortSignal) {
    signal.throwIfAborted();
    if (this.paused || this.selectionPaused) await abortable(new Promise<void>(resolve => { this.resume = resolve; }), signal);
    signal.throwIfAborted();
  }
  private schedule() {
    if (this.disposed || !this.visible || this.running && !this.resume || this.timer !== undefined || this.scheduledFrame || this.paused || this.selectionPaused) return;
    const enqueue = () => {
      this.timer = setTimeout(() => {
        this.timer = undefined;
        if (this.disposed || !this.visible || this.paused || this.selectionPaused) return;
        this.resume?.(); this.resume = undefined; void this.run();
      }, 0);
    };
    // Foreground reflow should replace stale geometry as soon as possible.
    // Replenishment and suspended alignment wait for the completed reader
    // frame and its observers, then recheck ownership in the queued task.
    if (this.request && !this.resume) enqueue();
    else this.scheduledFrame = requestAnimationFrame(() => {
      this.scheduledFrame = 0; enqueue();
    });
  }
  private remember(screen: PreparedScreen) {
    this.boundaries = this.boundaries.filter(b => !same(b.start, screen.start));
    this.boundaries.push({ start: { ...screen.start }, end: { ...screen.end } });
    if (this.boundaries.length > 128) this.boundaries.shift();
  }
  private async prepare(request: Request, signal: AbortSignal): Promise<PreparedScreen> {
    const control = { signal, checkpoint: () => this.checkpoint(signal) };
    if (!request.before) {
      const screen = await this.pagination.prepare(request.cursor, request.layout, control);
      this.remember(screen); return screen;
    }
    const end = request.cursor;
    const known = this.boundaries.find(b => same(b.end, end));
    let cursor = known?.start ?? { index: Math.max(0, end.index - (end.offset ? 0 : 1)), offset: 0 };
    // Cold jumps/reflow have no earlier screen boundary. Replay only the local
    // preceding leaf, stopping exactly at the current canonical source cursor.
    for (;;) {
      const screen = await this.pagination.prepare(cursor, request.layout, { ...control, end });
      this.remember(screen);
      if (same(screen.end, end)) return screen;
      if (compareCursor(screen.end, cursor) <= 0) throw new Error('Previous page made no source progress');
      cursor = { ...screen.end }; this.pagination.release(screen);
    }
  }
  private nextPreparation(): Request | null {
    const current = this.pagination.visible();
    if (!current) return null;
    const next = this.neighbor(current, 1), previous = this.neighbor(current, -1);
    if (this.alternate && !this.pagination.resident().includes(this.alternate)) this.alternate = null;
    this.pagination.protect([current, ...[next, previous, this.alternate].filter((s): s is PreparedScreen => !!s)]);
    const forward = !current.eof && !next ? { cursor: current.end, layout: current.layout } : null;
    const back = !same(current.start, { index: 0, offset: 0 }) && !previous ? { cursor: current.start, before: true, layout: current.layout } : null;
    if (this.direction > 0 ? forward : back) return this.direction > 0 ? forward : back;
    if (this.direction > 0 ? back : forward) return this.direction > 0 ? back : forward;
    // While adjusting display settings, use the fourth slot for the previous
    // geometry at this exact anchor. A reading turn releases it for lookahead.
    if (this.alternate) return null;
    const adjacent = this.direction > 0 ? next : previous;
    if (!adjacent || (this.direction > 0 ? adjacent.eof : same(adjacent.start, { index: 0, offset: 0 })) || this.neighbor(adjacent, this.direction)) return null;
    return { cursor: this.direction > 0 ? adjacent.end : adjacent.start, before: this.direction < 0, layout: adjacent.layout };
  }
  private async run() {
    if (this.running || this.disposed || !this.visible || this.paused || this.selectionPaused) return;
    this.running = true;
    const revision = this.revision, controller = new AbortController(); this.active = controller;
    try {
      for (let batch = 0; batch < 4; batch++) {
        await this.checkpoint(controller.signal);
        if (this.decorationDirty) {
          for (const screen of this.pagination.resident()) {
            await this.checkpoint(controller.signal);
            this.pagination.decorate(screen);
            await yieldReaderTask();
          }
          this.decorationDirty = false;
        }
        const foreground = this.request;
        const request = foreground ?? this.nextPreparation();
        if (!request) break;
        this.state.preparing = true; this.notify();
        if (request.replace) {
          this.boundaries = [];
        }
        const screen = await this.prepare(request, controller.signal);
        controller.signal.throwIfAborted();
        if (foreground) {
          if (!screen.pages.length) { this.pagination.release(screen); this.request = null; this.state.phase = 'ready'; break; }
          const previous = this.pagination.visible();
          this.pagination.show(screen);
          if (!request.replace) this.metrics.turns++;
          else {
            this.alternate = previous && same(previous.start, screen.start) && geometryKey(previous.layout) !== geometryKey(screen.layout) ? previous : null;
            for (const old of this.pagination.resident()) if (old !== screen && old !== this.alternate) this.pagination.release(old);
          }
          this.metrics.maxWaitMs = Math.max(this.metrics.maxWaitMs, performance.now() - this.waitStarted);
          this.request = null; this.state.phase = 'ready'; this.state.error = null; this.notify();
          // Let the newly readable foreground reach the rendering opportunity
          // before neighbor preparation consumes another main-thread slice.
          await abortable(new Promise<void>(resolve => requestAnimationFrame(() => resolve())), controller.signal);
        }
        await yieldReaderTask();
      }
    } catch (error) {
      if (!controller.signal.aborted && !this.disposed) {
        this.failed = this.request;
        this.state.error = error instanceof Error ? error.message : String(error);
        if (this.request) this.state.phase = 'error';
      }
    } finally {
      this.active = undefined; this.running = false; this.state.preparing = false; this.notify();
      if (revision !== this.revision) this.schedule();
    }
  }
  snapshot() {
    const screen = this.pagination.visible();
    return { ...this.state, metrics: { ...this.metrics }, resources: this.pagination.resources(), peak: { ...this.pagination.peak },
      work: { ...this.pagination.metrics }, sourceBytes: this.source.bytes(), boundaryCount: this.boundaries.length,
      current: screen ? { start: screen.start, end: screen.end, eof: screen.eof, id: screen.id } : null,
      resident: this.pagination.resident().map(s => ({ start: s.start, end: s.end, id: s.id })),
    };
  }
  dispose() {
    this.disposed = true; this.cancel(); clearTimeout(this.timer); cancelAnimationFrame(this.scheduledFrame); this.resume?.();
    this.request = null; this.failed = null; this.boundaries = []; this.source.clear(); this.pagination.dispose();
  }
}
