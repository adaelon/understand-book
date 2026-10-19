import { describe, expect, it, vi } from 'vitest';
import type { Manifest } from './api';
import { readWindowText, ReaderWindowTextCache } from './reader-window-text';

const source = '第一段😀\n\n第二段\n\n第三段';
const paragraphs = ['第一段😀', '第二段', '第三段'];
const leaves: Manifest['tree'] = paragraphs.map((text, index) => ({
  lid: `1.${index + 1}`, children: [], kind: 'paragraph',
  span: { start: source.indexOf(text), end: source.indexOf(text) + text.length },
}));
const tree: Manifest['tree'] = [{ lid: '1', children: leaves.map(node => node.lid), kind: 'chapter', span: { start: 0, end: source.length } }, ...leaves];
const reader = () => vi.fn(async (lid: string, end = lid) => ({ lid, text: source.slice(
  tree.find(node => node.lid === lid)!.span.start, tree.find(node => node.lid === end)!.span.end,
) }));

describe('reader window text', () => {
  it('reuses recent text and reads only missing leaves, with separate publication ownership', async () => {
    const read = reader(), cache = new ReaderWindowTextCache(tree, read);
    cache.remember([{ lid: '1.1', text: paragraphs[0] }]);
    expect(await cache.read(['1.1', '1.2'])).toEqual(paragraphs.slice(0, 2).map((text, i) => ({ lid: `1.${i + 1}`, text })));
    expect(read.mock.calls).toEqual([['1.2']]);
    await cache.read(['1.2', '1.1']);
    expect(read).toHaveBeenCalledOnce();
    expect(new ReaderWindowTextCache(tree, read).peek(['1.1'])).toBeNull();
  });
  it('evicts least recently used leaves and excludes oversized text without truncating a read', async () => {
    const read = reader(), cache = new ReaderWindowTextCache(tree, read, 2, 40);
    await cache.read(['1.1', '1.2']); cache.peek(['1.1']); await cache.read(['1.3']);
    expect(cache.peek(['1.2'])).toBeNull(); expect(cache.peek(['1.1', '1.3'])).not.toBeNull();
    const small = new ReaderWindowTextCache(tree, read, 2, 2);
    expect((await small.read(['1.1']))[0].text).toBe(paragraphs[0]);
    expect(small.peek(['1.1'])).toBeNull();
  });
  it('reads contiguous leaves once and restores exact UTF-16 paragraphs without separator text', async () => {
    const read = reader();
    expect(await readWindowText(leaves.map(node => node.lid), tree, read)).toEqual(
      paragraphs.map((text, index) => ({ lid: `1.${index + 1}`, text })),
    );
    expect(read.mock.calls).toEqual([['1.1', '1.3']]);
  });

  it('keeps nonadjacent, reversed and parent requests separate and preserves caller order', async () => {
    const read = reader();
    const lids = ['1.3', '1.1', '1', '1.2'];
    const result = await readWindowText(lids, tree, read);
    expect(result.map(value => value.lid)).toEqual(lids);
    expect(result.map(value => value.text)).toEqual([paragraphs[2], paragraphs[0], source, paragraphs[1]]);
    expect(read.mock.calls).toEqual(lids.map(lid => [lid]));
  });

  it('does not read across a skipped leaf and propagates a failed range', async () => {
    const read = reader();
    expect(await readWindowText(['1.1', '1.3'], tree, read)).toEqual([
      { lid: '1.1', text: paragraphs[0] }, { lid: '1.3', text: paragraphs[2] },
    ]);
    expect(read.mock.calls).toEqual([['1.1'], ['1.3']]);
    await expect(readWindowText(['1.1', '1.2'], tree, async () => { throw new Error('network failed'); })).rejects.toThrow('network failed');
  });
});
