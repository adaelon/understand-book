// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import type { Manifest } from './api';
import { FocusSource } from './reader-focus-source';
import { FocusReader } from './reader-focus';
import { ReaderPagination, type PaginationLayout } from './reader-pagination';
import { renderPaginationSource } from './reader-pagination-render';

const texts = ['正文一 **bold**。', '正文二 $x^2$。', '最后一段。'];
const text = texts.join('\n\n');
const tree: Manifest['tree'] = texts.map((t, i) => ({ lid: `1.${i}`, kind: 'paragraph', children: [], span: { start: text.indexOf(t), end: text.indexOf(t) + t.length } }));
const read = () => vi.fn(async (lid: string, end = lid) => ({ lid, text: text.slice(tree.find(n => n.lid === lid)!.span.start, tree.find(n => n.lid === end)!.span.end) }));
const layout: PaginationLayout = { width: 700, height: 800, fontSize: 19, fontFamily: 'serif', lineHeight: 1.85 };

describe('focus acquisition', () => {
  it('keeps an in-flight neighbor preparation when turning to a ready screen', () => {
    const host = document.createElement('div'); document.body.append(host);
    const reader = new FocusReader(host, new FocusSource(tree, read(), () => null));
    const controller = new AbortController();
    const current = { start: { index: 0, offset: 0 }, end: { index: 1, offset: 0 }, layout };
    const next = { start: current.end, end: { index: 2, offset: 0 }, layout };
    vi.spyOn(reader.pagination, 'visible').mockReturnValue(current as any);
    vi.spyOn(reader.pagination, 'resident').mockReturnValue([current, next] as any);
    vi.spyOn(reader.pagination, 'show').mockImplementation(() => {});
    (reader as any).active = controller; (reader as any).running = true;
    try {
      reader.setPaused(true);
      expect(reader.move(1)).toBe(true);
      expect(controller.signal.aborted).toBe(false);
      // A new foreground target still supersedes this preparation.
      reader.open({ index: 2, offset: 0 }, layout);
      expect(controller.signal.aborted).toBe(true);
    } finally { vi.restoreAllMocks(); reader.dispose(); host.remove(); }
  });
  it('starts a foreground reflow without waiting for the old geometry to paint', async () => {
    const host = document.createElement('div'); document.body.append(host);
    const request = read(), reader = new FocusReader(host, new FocusSource(tree, request, () => null));
    const raf = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(1);
    try {
      reader.open({ index: 0, offset: 0 }, layout);
      await new Promise(resolve => setTimeout(resolve, 20));
      expect(request).toHaveBeenCalled();
    } finally { reader.dispose(); raf.mockRestore(); host.remove(); }
  });
  it('resumes a suspended preparation only after the completed frame can paint', async () => {
    const host = document.createElement('div'); document.body.append(host);
    const reader = new FocusReader(host, new FocusSource(tree, read(), () => null));
    let frame!: FrameRequestCallback;
    const raf = vi.spyOn(window, 'requestAnimationFrame').mockImplementation(callback => { frame = callback; return 1; });
    try {
      reader.setPaused(true);
      let resumed = false;
      const pending = (reader as any).checkpoint(new AbortController().signal).then(() => { resumed = true; });
      reader.setPaused(false);
      await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
      expect(resumed).toBe(false);
      frame(0); expect(resumed).toBe(false);
      await pending; expect(resumed).toBe(true);
    } finally { reader.dispose(); raf.mockRestore(); host.remove(); }
  });
  it('keeps suspended preparation paused when another input takes the scheduled resume', async () => {
    const host = document.createElement('div'); document.body.append(host);
    const reader = new FocusReader(host, new FocusSource(tree, read(), () => null));
    let frame!: FrameRequestCallback;
    const raf = vi.spyOn(window, 'requestAnimationFrame').mockImplementation(callback => { frame = callback; return 1; });
    try {
      reader.setPaused(true);
      let resumed = false;
      const pending = (reader as any).checkpoint(new AbortController().signal).then(() => { resumed = true; });
      reader.setPaused(false); reader.setSelectionPaused(true);
      frame(0); await new Promise(resolve => setTimeout(resolve, 10));
      expect(resumed).toBe(false);
      reader.setSelectionPaused(false); frame(0);
      await pending; expect(resumed).toBe(true);
    } finally { reader.dispose(); raf.mockRestore(); host.remove(); }
  });
  it('merges adjacent reads, transfers consumed text and releases the unread tail', async () => {
    const request = read(), source = new FocusSource(tree, request, () => null), reserves: number[] = [];
    expect(await source.take(0, undefined, n => reserves.push(n))).toMatchObject({ text: texts[0] });
    expect(request.mock.calls).toEqual([['1.0', '1.2', undefined]]);
    expect(source.bytes()).toBe((texts[1].length + texts[2].length) * 2);
    expect(reserves).toEqual([text.length * 4]);
    await source.take(1); expect(request).toHaveBeenCalledTimes(1);
    source.clear(); expect(source.bytes()).toBe(0);
  });
  it('rejects late cancelled data without retaining it', async () => {
    let finish!: (value: { lid: string; text: string }) => void;
    const source = new FocusSource(tree, () => new Promise(r => { finish = r; }), () => null);
    const controller = new AbortController();
    const task = source.take(0, controller.signal);
    expect(source.bytes()).toBeGreaterThan(0);
    controller.abort(); finish({ lid: '1.0', text });
    await expect(task).rejects.toMatchObject({ name: 'AbortError' });
    expect(source.bytes()).toBe(0);
  });
  it('rebuilds an exact predecessor at a mid-leaf canonical cursor', async () => {
    const host = document.createElement('div'); document.body.append(host);
    const engine = new ReaderPagination({ host, read: async i => i < tree.length ? { lid: tree[i].lid, kind: 'paragraph', text: texts[i] } : null, render: renderPaginationSource });
    const end = { index: 1, offset: 3, lid: '1.1' };
    const screen = await engine.prepare({ index: 0, offset: 0 }, layout, { end });
    expect(screen.end).toEqual(end); expect(screen.pages[0].fragments.at(-1)?.end).toBe(3);
    expect(screen.eof).toBe(false);
    engine.dispose(); host.remove();
  });
  it('pauses preparation, cancels stale geometry and releases the complete ownership chain', async () => {
    const host = document.createElement('div'); document.body.append(host);
    const request = read(), source = new FocusSource(tree, request, () => null), reader = new FocusReader(host, source);
    reader.setPaused(true); reader.open({ index: 0, offset: 0 }, layout);
    await new Promise(r => setTimeout(r, 20)); expect(request).not.toHaveBeenCalled();
    reader.open({ index: 1, offset: 0 }, { ...layout, fontSize: 22 }); reader.setPaused(false);
    await vi.waitFor(() => expect(reader.state.phase).toBe('ready'));
    expect(reader.pagination.anchor()?.index).toBe(1);
    await vi.waitFor(() => expect(reader.state.preparing).toBe(false));
    const original = reader.pagination.visible()!;
    reader.open(original.start, { ...original.layout, fontSize: 19 });
    await vi.waitFor(() => expect(reader.pagination.visible()?.layout.fontSize).toBe(19));
    await vi.waitFor(() => expect(reader.state.preparing).toBe(false));
    const reflows = reader.pagination.metrics.preparations;
    reader.open(original.start, original.layout);
    expect(reader.pagination.visible()).toBe(original);
    expect(reader.pagination.metrics.preparations).toBe(reflows);
    expect(reader.pagination.resources().screens).toBeLessThanOrEqual(4);
    const retained = reader.pagination.visible()!, preparations = reader.pagination.metrics.preparations;
    reader.retainCurrent();
    expect(reader.pagination.resident()).toEqual([retained]); expect(retained.element.inert).toBe(true); expect(source.bytes()).toBe(0);
    reader.setVisible(true); reader.open(retained.start, retained.layout);
    expect(reader.pagination.visible()).toBe(retained); expect(retained.element.inert).toBe(false);
    expect(reader.pagination.metrics.preparations).toBe(preparations);
    reader.dispose(); await new Promise(r => setTimeout(r, 20));
    expect(source.bytes()).toBe(0); expect(host.children.length).toBe(0); host.remove();
  });
});
