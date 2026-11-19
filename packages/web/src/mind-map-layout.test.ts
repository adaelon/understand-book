import { describe, expect, it } from 'vitest';
import { makeRustMindMapFixture } from './fixtures/mind-map';
import { layoutMindMaps, topicAncestors, topicDescendants, wrapTopicText } from './mind-map-layout';
import { connectBounds, connectionLabelOffsetY } from './board-canvas-geometry';

const measure = (text: string, size: number) => Array.from(text).reduce((width, char) => width + (/^[\x00-\xff]$/.test(char) ? size / 2 : size), 0);
const overlap = (a: { x: number; y: number; width: number; height: number }, b: typeof a) => Math.min(a.x + a.width, b.x + b.width) > Math.max(a.x, b.x) && Math.min(a.y + a.height, b.y + b.height) > Math.max(a.y, b.y);

describe('semantic mind-map layout', () => {
  it('places vertical relation labels in the sibling gap without overlapping a topic', () => {
    const fixture = makeRustMindMapFixture(4), layout = layoutMindMaps(fixture.items, fixture.mindMaps, { measureText: measure });
    const from = layout.byId.get('topic-1')!, to = layout.byId.get('topic-3')!;
    const geometry = connectBounds(from, to), baseline = geometry.label.y + connectionLabelOffsetY(from, to);
    expect(baseline - 14).toBeGreaterThanOrEqual(from.y + from.height);
    expect(baseline + 4).toBeLessThanOrEqual(to.y);
  });

  it('connects stacked topics vertically even when the second title is much wider', () => {
    const from = { x: 0, y: 0, width: 112, height: 52 }, to = { x: 0, y: 80, width: 296, height: 52 };
    expect(connectionLabelOffsetY(from, to)).toBe(4);
    expect(connectBounds(from, to).path).toMatch(/^M 56 52 C /);
  });
  it('keeps root anchor and persisted branch sides stable, without overwriting source geometry', () => {
    const fixture = makeRustMindMapFixture(32), before = JSON.stringify(fixture);
    const layout = layoutMindMaps(fixture.items, fixture.mindMaps, { measureText: measure });
    expect(layout.byId.get('topic-0')).toMatchObject({ x: 300, y: 160 });
    expect(layout.byId.get('topic-2')!.x).toBeLessThan(300);
    const root = fixture.items[0];
    if (root.target.kind === 'topic') root.target.children.reverse();
    const reordered = layoutMindMaps(fixture.items, fixture.mindMaps, { measureText: measure });
    expect(reordered.byId.get('topic-2')!.x).toBeLessThan(300);
    if (root.target.kind === 'topic') root.target.children.reverse();
    expect(JSON.stringify(fixture)).toBe(before);
    expect(layout.edges).toHaveLength(31);
    expect(layout.items[1].position).not.toEqual(fixture.items[1].position);
  });

  it('sizes Chinese and multiline topics to measured text and lays out 500 nodes without overlap', () => {
    const fixture = makeRustMindMapFixture(500);
    fixture.items[12].title = '可写切片 &mut [u8]\n对 s[0] 赋值后，原数组中的对应元素改变。';
    const layout = layoutMindMaps(fixture.items, fixture.mindMaps, { measureText: measure });
    expect(layout.nodes).toHaveLength(500);
    for (let i = 0; i < layout.nodes.length; i++) {
      const node = layout.nodes[i];
      for (const line of node.lines) expect(line.x + measure(line.text, node.fontSize)).toBeLessThanOrEqual(node.width - 19);
      for (let j = i + 1; j < layout.nodes.length; j++) if (overlap(node, layout.nodes[j])) throw new Error(`${node.item_id} overlaps ${layout.nodes[j].item_id}`);
    }
    expect(layout.byId.get('topic-12')!.lines.length).toBeGreaterThan(1);
  });

  it('folds descendants without losing counts, then restores identical geometry', () => {
    const fixture = makeRustMindMapFixture(80), original = layoutMindMaps(fixture.items, fixture.mindMaps, { measureText: measure });
    const descendants = topicDescendants(fixture.items, 'topic-1', false);
    const folded = layoutMindMaps(fixture.items, fixture.mindMaps, { measureText: measure, collapsedIds: ['topic-1'] });
    expect(folded.byId.get('topic-1')!.hiddenCount).toBe(descendants.length);
    expect(descendants.every(id => !folded.byId.has(id))).toBe(true);
    expect(topicAncestors(fixture.items, 'topic-31')).toEqual(['topic-6', 'topic-1', 'topic-0']);
    expect(layoutMindMaps(fixture.items, fixture.mindMaps, { measureText: measure }).nodes).toEqual(original.nodes);
  });

  it('rightward tree ignores side assignment and leaves free cards in place', () => {
    const fixture = makeRustMindMapFixture(20); fixture.mindMaps[0].layout = 'right';
    const card = { ...fixture.items[0], item_id: 'free', position: { x: -500, y: 700 }, target: { kind: 'note' as const, note_id: 'note', missing: false } };
    const layout = layoutMindMaps([...fixture.items, card], fixture.mindMaps, { measureText: measure });
    expect(layout.nodes.every(node => node.x >= 300)).toBe(true);
    expect(layout.items.find(item => item.item_id === 'free')).toBe(card);
  });

  it('wraps CJK by measured glyphs, keeps ordinary identifiers whole, and retains explicit blank lines', () => {
    const lines = wrapTopicText('读取 slice_name 后\n\n修改 very_long_identifier_name', 120, 16, 500, measure);
    expect(lines).toContain(''); expect(lines.some(line => line.includes('slice_name'))).toBe(true);
    expect(lines.every(line => measure(line, 16) <= 120)).toBe(true);
  });
});
