import { describe, expect, it, vi } from 'vitest';
import type { Manifest } from './api';
import { readWindowText } from './reader-window-text';

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
