// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { alignMarkdownSource, bindMarkdownSourceMap, createMarkdownDomSourceMap, projectMarkdownDom } from './markdown-source-map';
import { renderInlineMarkdown } from './md';
import { sourceEdits } from './markdown-cooperative-alignment';
import { runReaderWork } from './reader-cooperative';
import { ReaderPagination, type PaginationLayout, type PaginationSource } from './reader-pagination';
import { renderPaginationSource } from './reader-pagination-render';

describe('pagination source alignment', () => {
  it.each([
    '中文 **strong** and [link](https://example.com/long/path?q=abc) 后文。',
    'This is $x^2$ and **bold** &amp; more.',
    '尾随空白  ',
    '相同字重复重复，**重复** 后文。',
  ])('retains selection semantics and complete canonical boundaries: %s', async source => {
    const root = document.createElement('div'); root.innerHTML = renderInlineMarkdown(source);
    const projection = projectMarkdownDom(root);
    const spans = await alignMarkdownSource(source, projection.text);
    const map = bindMarkdownSourceMap(source, projection, spans);
    const existing = createMarkdownDomSourceMap(source, root);
    expect(map.sourceRangesForSemanticRange(0, projection.text.length)).toEqual(existing.sourceRangesForSemanticRange(0, projection.text.length));
    const boundaries = Array.from({ length: projection.text.length + 1 }, (_, i) => map.sourceBoundary(i));
    expect(boundaries[0]).toBe(0); expect(boundaries.at(-1)).toBe(source.length);
    expect(boundaries.every((n, i) => !i || n >= boundaries[i - 1])).toBe(true);
    expect(boundaries.slice(1).map((n, i) => source.slice(boundaries[i], n)).join('')).toBe(source);
  });

  it('has cooperative checkpoints on divergent input and reconstructs both strings', async () => {
    const source = 'a'.repeat(1800) + 'b'.repeat(900), semantic = 'b'.repeat(900) + 'a'.repeat(1800);
    const work = sourceEdits(source, semantic); let checkpoints = 0;
    const wrapped = (function* () { for (;;) { const n = work.next(); if (n.done) return n.value; checkpoints++; yield; } })();
    let yields = 0;
    const edits = await runReaderWork(wrapped, async () => { yields++; });
    let a = 0, b = 0;
    for (const edit of edits) {
      if (edit.kind === 'equal') { expect(source[a]).toBe(semantic[b]); a++; b++; }
      else if (edit.kind === 'delete') a++; else b++;
    }
    expect(a).toBe(source.length); expect(b).toBe(semantic.length);
    expect(checkpoints).toBeGreaterThan(100); expect(yields).toBeGreaterThan(0);
  });
});

describe('local screen lifecycle', () => {
  const layout: PaginationLayout = { width: 700, height: 800, fontSize: 19, fontFamily: 'serif', lineHeight: 1.8 };
  const source: PaginationSource = { lid: '1.1', text: '可选择的 **原文**。', kind: 'paragraph' };
  it('joins imported inline formula leaves without merging canonical source ranges', async () => {
    const sources = [
      { lid: '1.1', kind: 'paragraph', text: 'Let ' },
      { lid: '1.2', kind: 'formula', text: '$x$' },
      { lid: '1.3', kind: 'paragraph', text: ' be a number.' },
      { lid: '1.4', kind: 'paragraph', text: 'A separate paragraph.' },
    ];
    const host = document.createElement('section'); document.body.append(host);
    const engine = new ReaderPagination({ host, read: async i => sources[i] ?? null, render: renderPaginationSource });
    const screen = await engine.prepare({ index: 0, offset: 0 }, layout);
    const fragments = screen.pages.flatMap(p => p.fragments);
    expect(fragments.slice(0, 3).map(f => f.element.parentElement)).toEqual(Array(3).fill(fragments[0].element.parentElement));
    expect(fragments[0].element.parentElement?.classList.contains('focus-flow')).toBe(true);
    expect(fragments[3].element.parentElement).not.toBe(fragments[2].element.parentElement);
    expect(fragments.map(f => ({ lid: f.lid, start: f.start, end: f.end }))).toEqual(sources.map(s => ({ lid: s.lid, start: 0, end: s.text.length })));
    const range = document.createRange(); range.selectNodeContents(fragments[2].element);
    expect(engine.sourceRanges(fragments[2], range)).toEqual([{ start: 0, end: sources[2].text.length }]);
    engine.dispose(); host.remove();
  });
  it('caps resident + preparing screens, releases old DOM and shows without reads', async () => {
    const host = document.createElement('section'); document.body.append(host);
    const read = vi.fn(async index => index === 0 ? source : null);
    const engine = new ReaderPagination({ host, read, render: renderPaginationSource });
    const first = await engine.prepare({ index: 0, offset: 0 }, layout); engine.show(first);
    for (let i = 0; i < 7; i++) {
      const next = await engine.prepare({ index: 0, offset: 0 }, layout);
      const reads = read.mock.calls.length, metrics = { ...engine.metrics };
      engine.show(next);
      expect(read.mock.calls.length).toBe(reads);
      expect(engine.metrics.preparations).toBe(metrics.preparations);
      expect(engine.resources().screens).toBeLessThanOrEqual(4);
    }
    expect(first.pages).toEqual([]); expect(first.element.childNodes.length).toBe(0);
    expect(engine.peak.screens).toBeLessThanOrEqual(4);
    expect(() => engine.show(first)).toThrow('not resident');
    expect(() => engine.release(engine.visible()!)).toThrow('visible');
    engine.dispose(); expect(host.children.length).toBe(0); expect(engine.resources().sourceBytes).toBe(0); host.remove();
  });
  it('updates cached resource counts when prepared annotations change and releases them with the screen', async () => {
    const host = document.createElement('section'); document.body.append(host);
    let marked = false;
    const engine = new ReaderPagination({ host, read: async i => i === 0 ? { lid: '1.1', kind: 'paragraph', text: '选中字句。' } : null,
      render: renderPaginationSource, decorations: () => ({ marks: marked ? [{ start: 1, end: 3, className: 'hl-mark' }] : [], markers: [] }) });
    const screen = await engine.prepare({ index: 0, offset: 0 }, layout); engine.show(screen);
    const plain = engine.resources(); marked = true; engine.decorate(screen);
    expect(screen.element.querySelector('.hl-mark')?.textContent).toBe('中字');
    expect(engine.resources().domNodes).toBeGreaterThan(plain.domNodes);
    expect(engine.resources().parsedBytes).toBeGreaterThan(plain.parsedBytes);
    marked = false; engine.decorate(screen);
    expect(engine.resources().domNodes).toBe(plain.domNodes);
    engine.dispose(); expect(engine.resources().domNodes).toBe(0); host.remove();
  });
  it('preserves the readable screen on a failed preparation', async () => {
    const host = document.createElement('section'); document.body.append(host);
    let fail = false;
    const engine = new ReaderPagination({ host, read: async index => index === 0 ? source : null, render: renderPaginationSource, ready: async () => { if (fail) throw new Error('decode failed'); } });
    const first = await engine.prepare({ index: 0, offset: 0 }, layout); engine.show(first); fail = true;
    await expect(engine.reflow({ ...layout, fontSize: 22 })).rejects.toThrow('decode failed');
    expect(engine.visible()).toBe(first); expect(first.element.hidden).toBe(false); expect(engine.resources().preparationTasks).toBe(0);
    engine.dispose(); host.remove();
  });
  it('serializes preparation and discards a read that finishes after disposal', async () => {
    const host = document.createElement('section'); document.body.append(host);
    let resolve!: (value: PaginationSource) => void;
    const engine = new ReaderPagination({ host, read: () => new Promise(r => { resolve = r; }), render: renderPaginationSource });
    const pending = engine.prepare({ index: 0, offset: 0 }, layout);
    await expect(engine.prepare({ index: 0, offset: 0 }, layout)).rejects.toThrow('unavailable');
    engine.dispose(); resolve(source); await expect(pending).rejects.toThrow('disposed');
    expect(host.children.length).toBe(0); expect(engine.resources().sourceBytes).toBe(0); host.remove();
  });
  it('maps a partial code-object selection exactly using the prepared alignment', async () => {
    const host = document.createElement('section'); document.body.append(host);
    const engine = new ReaderPagination({ host, read: async index => index === 0 ? { ...source, kind: 'code' } : null, render: renderPaginationSource });
    const screen = await engine.prepare({ index: 0, offset: 2 }, layout);
    expect(screen.start.offset).toBe(0);
    const fragment = screen.pages[0].fragments[0], range = document.createRange(); range.setStart(fragment.element.firstChild!, 2); range.setEnd(fragment.element.firstChild!, 5);
    expect(engine.sourceRanges(fragment, range)).toEqual([{ start: 2, end: 5 }]);
    engine.dispose(); host.remove();
  });
  it('freezes canonical prose quotes across continuations without aligning again, rejecting stale DOM', async () => {
    const host = document.createElement('section'); document.body.append(host);
    const engine = new ReaderPagination({ host, read: async i => i === 0 ? source : null, render: renderPaginationSource });
    const screen = await engine.prepare({ index: 0, offset: 5 }, layout); engine.show(screen);
    const fragment = screen.pages[0].fragments[0], range = document.createRange(); range.selectNodeContents(fragment.element);
    const alignments = engine.metrics.alignments;
    expect(engine.selection(range)).toEqual({ ranges: [{ lid: '1.1', start: 7, end: 9 }, { lid: '1.1', start: 11, end: 12 }], resolvedText: '原文。' });
    expect(engine.metrics.alignments).toBe(alignments);
    const next = await engine.prepare({ index: 0, offset: 0 }, layout); engine.show(next);
    expect(engine.selection(range)).toBeNull(); engine.dispose(); host.remove();
  });
});
