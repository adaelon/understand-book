import MarkdownIt from 'markdown-it';
import type { BoardItem } from './generated/BoardItem';
import type { BoardEditOperation } from './generated/BoardEditOperation';
import type { BoardMindMap } from './generated/BoardMindMap';
import { topicTreeIndex } from './mind-map-layout';

export interface OutlineTopic { title: string; children: OutlineTopic[] }
const markdown = new MarkdownIt({ html: false });

function plainTitle(text: string): string {
  const tokens = markdown.parseInline(text, {})[0]?.children ?? [];
  return tokens.map(token => token.type === 'text' || token.type === 'code_inline' ? token.content
    : token.type === 'softbreak' || token.type === 'hardbreak' ? '\n' : token.type === 'image' ? token.content : '').join('');
}

/** Import Markdown headings and nested lists. Paragraphs within a list remain one title. */
export function parseMindMapOutline(text: string): OutlineTopic[] {
  const roots: OutlineTopic[] = [], headings: { level: number; node: OutlineTopic }[] = [];
  const lists: { indent: number; contentIndent: number; node: OutlineTopic }[] = [];
  let fence: string | null = null, previous: OutlineTopic | null = null;
  const sourceLines = text.replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n').split('\n');
  for (let sourceIndex = 0; sourceIndex < sourceLines.length; sourceIndex++) {
    let line = sourceLines[sourceIndex];
    if (line.trim() && /^ {0,3}(?:=+|-+)\s*$/.test(sourceLines[sourceIndex + 1] ?? '') && !/^\s*[-+*]\s/.test(line)) {
      line = `${sourceLines[++sourceIndex].trim().startsWith('=') ? '#' : '##'} ${line.trim()}`;
    }
    const fenced = line.trimStart().match(/^(`{3,}|~{3,})/);
    if (fenced) { if (!fence) fence = fenced[1][0]; else if (fence === fenced[1][0]) fence = null; continue; }
    if (fence) continue;
    if (!line.trim()) {
      if (lists.length && line.length >= lists.at(-1)!.contentIndent) lists.at(-1)!.node.title += '\n';
      continue;
    }
    const heading = line.match(/^ {0,3}(#{1,6})[ \t]+(.+?)(?:[ \t]+#+[ \t]*)?$/);
    if (heading) {
      const level = heading[1].length, node = { title: plainTitle(heading[2]), children: [] as OutlineTopic[] };
      while (headings.length && headings.at(-1)!.level >= level) headings.pop();
      (headings.at(-1)?.node.children ?? roots).push(node); headings.push({ level, node }); lists.length = 0; previous = node; continue;
    }
    const list = line.match(/^(\s*)(?:[-+*]|\d+[.)])[ \t]+(.*)$/);
    if (list) {
      const indent = list[1].replace(/\t/g, '    ').length, title = list[2].replace(/^\[[ xX]\][ \t]+/, '');
      const node = { title: plainTitle(title), children: [] as OutlineTopic[] };
      while (lists.length && lists.at(-1)!.indent >= indent) lists.pop();
      (lists.at(-1)?.node.children ?? headings.at(-1)?.node.children ?? roots).push(node);
      lists.push({ indent, contentIndent: line.length - list[2].length, node }); previous = node; continue;
    }
    if (/^ {0,3}(?:---+|___+|\*\*\*+)\s*$/.test(line)) continue;
    const indentation = line.match(/^\s*/)?.[0].replace(/\t/g, '    ').length ?? 0;
    if (lists.length && indentation >= lists.at(-1)!.contentIndent) {
      lists.at(-1)!.node.title += `\n${plainTitle(line.trim())}`; continue;
    }
    // A plain first line is useful for pasting a title followed by an outline.
    const node = { title: plainTitle(line.trim()), children: [] as OutlineTopic[] };
    if (!roots.length) { roots.push(node); headings.push({ level: 0, node }); }
    else if (previous) { (headings.at(-1)?.node.children ?? roots).push(node); }
    previous = node; lists.length = 0;
  }
  return roots;
}

const escapeTitle = (text: string) => text.replace(/&/g, '&amp;').replace(/\\/g, '\\\\').replace(/([`*_[\]<>#!])/g, '\\$1');

export function serializeMindMapOutline(items: readonly BoardItem[], rootItemId: string): string {
  const index = topicTreeIndex(items), output: string[] = [], seen = new Set<string>();
  const pending = [{ id: rootItemId, depth: 0 }];
  while (pending.length) {
    const { id, depth } = pending.pop()!, item = index.byId.get(id);
    if (!item || seen.has(id)) continue;
    seen.add(id);
    const lines = item.title.replace(/\r\n?/g, '\n').split('\n');
    // A list allows arbitrary depth; indented continuation preserves multiline labels.
    const indent = '  '.repeat(depth);
    output.push(`${indent}- ${escapeTitle(lines[0])}`);
    for (const line of lines.slice(1)) output.push(`${indent}  ${escapeTitle(line)}`);
    for (const child of [...(index.childrenById.get(id) ?? [])].reverse()) pending.push({ id: child, depth: depth + 1 });
  }
  return output.length ? `${output.join('\n')}\n` : '';
}

export interface MindMapImportOptions {
  title?: string;
  mapId?: string;
  rootItemId?: string;
  anchor?: { x: number; y: number };
  layout?: BoardMindMap['layout'];
  createId?: () => string;
}
export interface MindMapImport { map: BoardMindMap; rootItemId: string; operations: BoardEditOperation[]; topicCount: number }

export function buildMindMapImport(text: string, options: MindMapImportOptions = {}): MindMapImport {
  const forest = parseMindMapOutline(text);
  if (!forest.length) throw new Error('没有找到可导入的标题或列表。');
  const createId = options.createId ?? (() => crypto.randomUUID());
  const root = forest.length === 1 ? forest[0] : { title: options.title || '导入的思维导图', children: forest };
  const rootItemId = options.rootItemId ?? createId();
  const map: BoardMindMap = { map_id: options.mapId ?? createId(), root_item_id: rootItemId, layout: options.layout ?? 'balanced', anchor: options.anchor ?? { x: 120, y: 120 }, branch_sides: {} };
  const topic = (id: string, title: string) => ({ item_id: id, title, style: { color: null, bold: false }, based_on: [], source_bindings: [] });
  const operations: BoardEditOperation[] = [{ kind: 'create_mind_map', map, root: topic(rootItemId, root.title) }];
  const pending = root.children.map((node, index) => ({ node, index, parent: rootItemId })).reverse();
  while (pending.length) {
    const { node, index, parent } = pending.pop()!, id = createId();
    operations.push({ kind: 'insert_topic', parent_item_id: parent, index, topic: topic(id, node.title) });
    for (let i = node.children.length - 1; i >= 0; i--) pending.push({ node: node.children[i], index: i, parent: id });
  }
  return { map, rootItemId, operations, topicCount: operations.length };
}
