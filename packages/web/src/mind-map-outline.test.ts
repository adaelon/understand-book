import { describe, expect, it } from 'vitest';
import { makeRustMindMapFixture } from './fixtures/mind-map';
import { buildMindMapImport, parseMindMapOutline, serializeMindMapOutline } from './mind-map-outline';

describe('Markdown mind-map outlines', () => {
  it('combines headings and nested lists without treating code blocks as topics', () => {
    const result = parseMindMapOutline('# 数组与切片\n## 借用\n- **只读** `&[u8]`\n  读取元素\n  - s[0] 是 20\n- 可写\n## 生命周期\n```rust\n- not a topic\n```');
    expect(result).toEqual([{ title: '数组与切片', children: [
      { title: '借用', children: [{ title: '只读 &[u8]\n读取元素', children: [{ title: 's[0] 是 20', children: [] }] }, { title: '可写', children: [] }] },
      { title: '生命周期', children: [] },
    ] }]);
  });

  it('supports underlined headings and ordered lists with a plain-text title', () => {
    expect(parseMindMapOutline('Rust\n====\n借用\n----\n1. 只读\n   1. 读取\n2. 可写')[0]).toMatchObject({ title: 'Rust', children: [{ title: '借用', children: [{ title: '只读', children: [{ title: '读取' }] }, { title: '可写' }] }] });
    expect(parseMindMapOutline('我的导图\n- 子主题')[0]).toEqual({ title: '我的导图', children: [{ title: '子主题', children: [] }] });
  });

  it('round-trips literal syntax, code, entities, Chinese and multiline labels through arbitrary-depth lists', () => {
    const { items } = makeRustMindMapFixture(3);
    items[0].title = '# Rust **不是粗体** &mut [u8] &lt;T&gt;';
    items[1].title = '第一行\n\n第三行 `let s` \\ value'; items[2].title = '[文字](不是链接) <标签>';
    const parsed = parseMindMapOutline(serializeMindMapOutline(items, 'topic-0'));
    expect(parsed[0].title).toBe(items[0].title);
    expect(parsed[0].children.map(node => node.title)).toEqual(items.slice(1).map(item => item.title));
  });

  it('creates one atomic preorder import with stable allocated IDs and valid sibling indices', () => {
    let next = 0;
    const result = buildMindMapImport('- 只读\n  - 读取元素\n- 可写', { title: 'Rust', createId: () => `id-${++next}`, anchor: { x: 800, y: 100 } });
    expect(result.topicCount).toBe(4);
    expect(result.operations.map(operation => operation.kind)).toEqual(['create_mind_map', 'insert_topic', 'insert_topic', 'insert_topic']);
    expect(result.operations[1]).toMatchObject({ parent_item_id: result.rootItemId, index: 0, topic: { item_id: 'id-3', title: '只读' } });
    expect(result.operations[2]).toMatchObject({ parent_item_id: 'id-3', index: 0 });
    expect(result.operations[3]).toMatchObject({ parent_item_id: result.rootItemId, index: 1 });
    expect(result.map.anchor).toEqual({ x: 800, y: 100 });
    expect(() => buildMindMapImport(' \n\n')).toThrow('没有找到');
  });
});
