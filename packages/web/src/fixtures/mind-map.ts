import type { BoardItem } from '../generated/BoardItem';
import type { BoardMindMap } from '../generated/BoardMindMap';

// Repeatable scale input based on the Rust material used in the visitor replay.
// Repetition exercises layout capacity; it does not claim to be a generated chapter outline.
const rustTitles = ['数组与切片', '数组包含连续的元素', '切片借用数组的一段', '起点包含，终点不包含', '读取 s[0] 得到 20',
  '只读借用 &a[1..3]', '可写借用 &mut a[1..3]', '所有权仍属于原数组', '借用期间不能再次可变访问', '长度可以在运行时确定',
  '索引超出范围会报错', '写入后原数组发生改变', 'Vec 可以增长与缩小', '字符串切片使用 UTF-8 字节范围', '引用不能比原数据活得更久', '先运行例子，再比较结果'];

export function makeRustMindMapFixture(count = 500, titles: readonly string[] = rustTitles): { items: BoardItem[]; mindMaps: BoardMindMap[] } {
  const items: BoardItem[] = [];
  for (let i = 0; i < count; i++) {
    const depth = i === 0 ? 0 : Math.floor(Math.log(i * 4 + 1) / Math.log(5));
    const title = i === 0 ? 'Rust：数组、切片与借用' : `${titles[(i - 1) % titles.length]}${i > titles.length ? ` · 示例 ${i}` : ''}`;
    items.push({ item_id: `topic-${i}`, title, readable_content: '', explanation_blocks: [], created_at: '2026-10-28T00:00:00Z', source_bindings: [],
      position: { x: 0, y: 0 }, size: { width: 160, height: 48 },
      target: { kind: 'topic', children: [], style: { color: null, bold: depth === 0 }, based_on: [] } });
    if (i > 0) {
      const parent = items[Math.floor((i - 1) / 5)];
      if (parent.target.kind === 'topic') parent.target.children.push(items[i].item_id);
    }
  }
  const root = items[0];
  const children = root?.target.kind === 'topic' ? root.target.children : [];
  return { items, mindMaps: root ? [{ map_id: 'rust-map', root_item_id: root.item_id, layout: 'balanced', anchor: { x: 300, y: 160 },
    branch_sides: Object.fromEntries(children.map((id, i) => [id, i % 2 === 0 ? 'right' : 'left'])) }] : [] };
}
