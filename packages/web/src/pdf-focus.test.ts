// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PdfFocusReader } from './pdf-focus';
import { pdfFocusGeometry } from './pdf-rendering';
import type { PdfReadingAnchor } from './useReadingContinuity';

vi.mock('pdfjs-dist/legacy/build/pdf.mjs', () => ({ TextLayer: class {
  constructor(private options: any) {}
  async render() { const reader = this.options.textContentSource.getReader(); while (!(await reader.read()).done) {} this.options.container.textContent = 'PDF text'; }
  cancel() {}
} }));
const readers: PdfFocusReader[] = [];
afterEach(() => { readers.forEach(r => r.dispose()); readers.length = 0; document.body.replaceChildren(); });
const anchor = (pageIndex: number): PdfReadingAnchor => ({ surface: 'pdf', sourceKey: 'test', pageIndex, pageRatio: .3, probeRatio: .28,
  horizontalRatio: .5, zoom: 1, anchorLid: null });
function setup(renderPage?: (number: number) => { promise: Promise<void>; cancel: () => void }) {
  const host = document.createElement('div'); document.body.append(host);
  const cleanup = vi.fn();
  const doc = { getPage: vi.fn(async (number: number) => ({
    getViewport: ({ scale }: any) => ({ width: 612 * scale, height: 792 * scale, scale }), cleanup,
    streamTextContent: () => new ReadableStream({ start(controller) { controller.enqueue({ items: [] }); controller.close(); } }),
    render: () => renderPage?.(number) ?? ({ promise: Promise.resolve(), cancel() {} }),
  })) };
  const pages = Array.from({ length: 9 }, (_, pageIndex) => ({ pageIndex, width: 612, height: 792, rotate: 0 as const, view: [0, 0, 612, 792] as [number, number, number, number] }));
  const reader = new PdfFocusReader(host, doc as any, pages, 'test', {
    changed() {}, decorate(_page, layer) { layer.textContent = 'annotation'; }, releaseDecoration(layer) { layer.replaceChildren(); },
    externalResources: () => ({ pages: 0, backingPixels: 0 }),
  });
  readers.push(reader);
  return { reader, doc, cleanup, host, geometry: pdfFocusGeometry(1440, 900, 1) };
}
describe('PDF complete-spread preparation', () => {
  it('settles preparation when disposed after publishing the current screen', async () => {
    let painted!: FrameRequestCallback;
    const raf = vi.spyOn(window, 'requestAnimationFrame').mockImplementation(callback => { painted = callback; return 1; });
    const { reader, geometry, host } = setup();
    try {
      reader.open(anchor(0), geometry);
      const running = (reader as any).run() as Promise<void>;
      await vi.waitFor(() => expect(reader.pagination.visible()).toBeTruthy());
      reader.dispose(); painted(0);
      await expect(running).resolves.toBeUndefined();
      expect(host.children).toHaveLength(0);
    } finally { raf.mockRestore(); }
  });
  it('does not publish half a spread; canvas, text and annotation publish together', async () => {
    let ready!: () => void; const pending = new Promise<void>(resolve => { ready = resolve; });
    const { reader, host, geometry } = setup(number => ({ promise: number === 2 ? pending : Promise.resolve(), cancel() { ready(); } }));
    reader.open(anchor(0), geometry);
    await vi.waitFor(() => expect(reader.metrics.canvases).toBe(2));
    expect(reader.pagination.visible()).toBeUndefined();
    expect(host.querySelectorAll('.pdf-focus-screen[aria-hidden="true"]')).toHaveLength(1);
    ready(); await vi.waitFor(() => expect(reader.pagination.visible()?.pages).toHaveLength(2));
    for (const page of reader.pagination.visible()!.pages) { expect(page.text.textContent).toBe('PDF text'); expect(page.annotations.textContent).toBe('annotation'); }
  });
  it('moves without preparing while paused and includes candidate canvases in the budget', async () => {
    const { reader, geometry } = setup(); reader.open(anchor(2), geometry);
    await vi.waitFor(() => expect(reader.snapshot().ready).toHaveLength(3));
    reader.setPaused(true); const before = { ...reader.metrics };
    expect(reader.move(1)).toBe(true); expect(reader.pagination.visible()!.start.index).toBe(4);
    await new Promise(r => setTimeout(r, 25));
    expect(reader.metrics.preparations).toBe(before.preparations); expect(reader.metrics.canvases).toBe(before.canvases);
    expect(reader.resources().pages).toBeLessThanOrEqual(6); expect(reader.peak.backingPixels).toBeLessThanOrEqual(12_000_000);
    reader.setPaused(false); await vi.waitFor(() => expect(reader.adjacent(1)).toBeTruthy());
  });
  it('retains old pixels during reflow and preserves a right-page anchor', async () => {
    const { reader, geometry } = setup(); reader.open(anchor(3), geometry);
    await vi.waitFor(() => expect(reader.snapshot().ready).toHaveLength(3));
    const oldCanvas = reader.pagination.visible()!.pages.find(p => p.info.pageIndex === 3)!.canvas;
    const before = { ...reader.metrics }; expect(reader.captureAnchor()?.pageIndex).toBe(3);
    const next = pdfFocusGeometry(390, 844, 1);
    reader.open(anchor(3), next); expect(reader.pagination.visible()?.pages[0].canvas).toBe(oldCanvas);
    expect(reader.metrics).toEqual(before);
    await vi.waitFor(() => expect(reader.pagination.visible()?.geometry.columns).toBe(1));
    await vi.waitFor(() => expect(reader.snapshot().ready).toHaveLength(3));
    expect(reader.captureAnchor()?.pageIndex).toBe(3); expect(reader.resources().pages).toBe(3);
    expect(reader.peak.backingPixels).toBeLessThanOrEqual(12_000_000);
  });
  it('releases neighbours and unfinished work on exit, then destroys retained canvases', async () => {
    const { reader, geometry, host, cleanup } = setup(); reader.open(anchor(2), geometry);
    await vi.waitFor(() => expect(reader.snapshot().ready).toHaveLength(3));
    reader.retainCurrent(); expect(reader.resources().pages).toBe(2); expect(reader.pagination.visible()!.element.inert).toBe(true);
    const canvases = [...host.querySelectorAll('canvas')]; reader.dispose();
    expect(host.children).toHaveLength(0); expect(canvases.every(c => c.width === 0 && c.height === 0)).toBe(true); expect(cleanup).toHaveBeenCalled();
  });
  it('cancels an unready forward target when the reader reverses at the first page', async () => {
    let ready!: () => void; const pending = new Promise<void>(resolve => { ready = resolve; });
    const { reader, geometry } = setup(number => ({ promise: number === 3 ? pending : Promise.resolve(), cancel() { ready(); } }));
    reader.open(anchor(0), geometry);
    await vi.waitFor(() => expect(reader.metrics.canvases).toBe(3));
    expect(reader.move(1)).toBe(false); reader.cancelPending(); ready();
    await vi.waitFor(() => expect(reader.snapshot().ready).toHaveLength(2));
    expect(reader.pagination.visible()!.start.index).toBe(0); expect(reader.snapshot().waiting).toBe(false);
  });
  it('does not resume preparation if another pause takes ownership before the continuation', async () => {
    const { reader, geometry } = setup(); reader.setPaused(true); reader.open(anchor(0), geometry);
    reader.setPaused(false); reader.setSelectionPaused(true);
    await new Promise(r => setTimeout(r, 20)); expect(reader.metrics.preparations).toBe(0);
    reader.setSelectionPaused(false); await vi.waitFor(() => expect(reader.pagination.visible()).toBeTruthy());
  });
  it('preserves the ready neighbours when restoring an equivalent display geometry', async () => {
    const { reader, geometry } = setup(); reader.open(anchor(2), geometry);
    await vi.waitFor(() => expect(reader.snapshot().ready).toHaveLength(3));
    const before = reader.metrics.canvases;
    reader.open(anchor(2), { ...geometry }); await new Promise(r => setTimeout(r, 100));
    expect(reader.snapshot().ready).toHaveLength(3); expect(reader.adjacent(1)).toBeTruthy();
    expect(reader.metrics.canvases).toBe(before);
  });
});
