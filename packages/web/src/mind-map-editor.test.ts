// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { ref } from 'vue';
import { boardFixture } from './fixtures/board-workspace';
import { defaultBoardView } from './board-workspace';
import { applyMindMapOperations, moveSelectionOperations, selectionRoots, topicInput, useMindMapEditor } from './mind-map-editor';
import type { BoardDocument } from './generated/BoardDocument';
import type { BoardEditOperation } from './generated/BoardEditOperation';
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function document() {
  const source = boardFixture().document;
  return applyMindMapOperations(source, [
    { kind: 'create_mind_map', map: { map_id: 'map', root_item_id: 'root', layout: 'balanced', anchor: { x: 0, y: 0 }, branch_sides: {} }, root: topicInput('切片', 'root') },
    { kind: 'insert_topic', parent_item_id: 'root', index: 0, topic: topicInput('只读', 'read') },
    { kind: 'insert_topic', parent_item_id: 'root', index: 1, topic: topicInput('可写', 'write') },
    { kind: 'insert_topic', parent_item_id: 'read', index: 0, topic: { ...topicInput('访问元素', 'element'), source_bindings: source.items[0].source_bindings } },
  ]);
}
describe('mind map semantic editor', () => {
  it('keeps a multi-selected sibling group together at the precise dropped index', () => {
    const original = applyMindMapOperations(document(), [ { kind: 'insert_topic', parent_item_id: 'root', index: 2, topic: topicInput('C', 'c') }, { kind: 'insert_topic', parent_item_id: 'root', index: 3, topic: topicInput('D', 'd') } ]);
    const operations = moveSelectionOperations(original, ['read', 'write', 'element'], 'read', 'root', 2);
    const moved = applyMindMapOperations(original, operations), root = moved.items.find(item => item.item_id === 'root')!;
    expect(root.target.kind === 'topic' && root.target.children).toEqual(['c', 'read', 'write', 'd']);
  });
  it('normalizes parent/child multi-selection, copies source-bearing branches with fresh ids, and rejects a cycle', () => {
    const original = document();
    expect(selectionRoots(original.items, ['read', 'element', 'write'])).toEqual(['read', 'write']);
    const copied = applyMindMapOperations(original, [{ kind: 'copy_subtree', item_id: 'read', parent_item_id: 'write', index: 0, id_prefix: 'copy' }]);
    expect(copied.items.find(item => item.item_id === 'copy-element')?.source_bindings).toEqual(original.items.find(item => item.item_id === 'element')?.source_bindings);
    expect(copied.deliveries).toEqual(original.deliveries);
    expect(() => applyMindMapOperations(original, [{ kind: 'move_subtree', item_id: 'read', parent_item_id: 'element', index: 0 }])).toThrow('自己的分支');
    expect(original.items.some(item => item.item_id === 'copy-read')).toBe(false);
  });
  it('serializes optimistic edits without dropping an edit queued during an in-flight save', async () => {
    let stored = document(); const board = ref<BoardDocument | null>(clone(stored)), view = ref(defaultBoardView());
    let release: (() => void) | undefined; const gate = new Promise<void>(resolve => { release = resolve; });
    const requests: number[] = [];
    const api = { boardEdit: vi.fn(async (input: { expected_revision: number; operations: BoardEditOperation[] }) => {
      requests.push(input.expected_revision); if (requests.length === 1) await gate;
      expect(input.expected_revision).toBe(stored.revision);
      stored = applyMindMapOperations(stored, input.operations); stored.revision++; return clone(stored);
    }), boardUndo: vi.fn(), boardRedo: vi.fn() };
    const editor = useMindMapEditor(board, view, api, async () => true); editor.install(stored);
    editor.apply([{ kind: 'update_topic', item_id: 'read', title: '只读借用' }], 'rename');
    await Promise.resolve(); editor.apply([{ kind: 'insert_topic', parent_item_id: 'write', index: 0, topic: topicInput('修改元素', 'new') }], 'insert');
    expect(board.value!.items.find(item => item.item_id === 'read')?.title).toBe('只读借用');
    expect(board.value!.items.some(item => item.item_id === 'new')).toBe(true);
    release!(); expect(await editor.flush()).toBe(true);
    expect(requests).toEqual([stored.revision - 2, stored.revision - 1]);
    expect(board.value!.items.some(item => item.item_id === 'new')).toBe(true); expect(editor.queue.value).toHaveLength(0);
  });
  it('defers an AI read containing our committed insert until the local save acknowledgement arrives', async () => {
    const original = document(), board = ref<BoardDocument | null>(clone(original)); let release!: (document: BoardDocument) => void;
    const response = new Promise<BoardDocument>(resolve => { release = resolve; });
    const api = { boardEdit: vi.fn().mockReturnValue(response), boardUndo: vi.fn(), boardRedo: vi.fn() };
    const editor = useMindMapEditor(board, ref(defaultBoardView()), api, async () => true); editor.install(original);
    const operations: BoardEditOperation[] = [{ kind: 'insert_topic', parent_item_id: 'read', index: 1, topic: topicInput('手工添加', 'local') }];
    editor.apply(operations, 'insert'); await Promise.resolve();
    const saved = { ...applyMindMapOperations(original, operations), revision: original.revision + 1 };
    const remote = { ...applyMindMapOperations(saved, [{ kind: 'update_topic', item_id: 'write', title: 'AI 已补例子' }]), revision: original.revision + 2 };
    editor.receive(remote); expect(board.value!.items.filter(item => item.item_id === 'local')).toHaveLength(1);
    release(saved); expect(await editor.flush()).toBe(true);
    expect(board.value!.revision).toBe(remote.revision); expect(board.value!.items.filter(item => item.item_id === 'local')).toHaveLength(1);
    expect(board.value!.items.find(item => item.item_id === 'write')?.title).toBe('AI 已补例子'); expect(editor.error.value).toBe(''); expect(editor.queue.value).toHaveLength(0);
  });
  it('retains failed local edits and retries the same operation and revision', async () => {
    const original = document(), board = ref<BoardDocument | null>(clone(original));
    const api = { boardEdit: vi.fn().mockRejectedValueOnce(new Error('断网')).mockImplementation(async input => ({ ...applyMindMapOperations(original, input.operations), revision: original.revision + 1 })), boardUndo: vi.fn(), boardRedo: vi.fn() };
    const editor = useMindMapEditor(board, ref(defaultBoardView()), api, async () => true); editor.install(original);
    editor.apply([{ kind: 'update_topic', item_id: 'read', title: '手工文字' }], 'rename');
    expect(await editor.flush()).toBe(false); expect(editor.error.value).toContain('断网'); expect(board.value!.items.find(item => item.item_id === 'read')?.title).toBe('手工文字');
    expect(await editor.flush()).toBe(true); expect(api.boardEdit.mock.calls[0]).toEqual(api.boardEdit.mock.calls[1]);
  });
  it('preserves an unsaved title when AI delivers, requires resolving same-topic changes, and never resurrects deleted topics', () => {
    const original = document(), board = ref<BoardDocument | null>(clone(original));
    const api = { boardEdit: vi.fn(), boardUndo: vi.fn(), boardRedo: vi.fn() };
    const editor = useMindMapEditor(board, ref(defaultBoardView()), api, async () => true); editor.install(original); editor.startEdit('read'); editor.draftTitle.value = '我的草稿';
    const updated = applyMindMapOperations(original, [{ kind: 'update_topic', item_id: 'read', title: 'AI 新标题' }]); updated.revision++;
    editor.receive(updated); expect(editor.draftTitle.value).toBe('我的草稿'); expect(editor.commitEdit()).toBe(false); expect(editor.draftConflict.value).toBe(true); expect(api.boardEdit).not.toHaveBeenCalled();
    const deleted = applyMindMapOperations(updated, [{ kind: 'remove_subtree', item_id: 'read' }]); deleted.revision++; editor.receive(deleted);
    expect(editor.commitEdit(true)).toBe(false); expect(editor.draftTitle.value).toBe('我的草稿'); expect(board.value!.items.some(item => item.item_id === 'read')).toBe(false);
  });
  it('will not retry pending operations over changed parent ordering after a remote revision arrives', async () => {
    const original = document(), board = ref<BoardDocument | null>(clone(original));
    const api = { boardEdit: vi.fn().mockRejectedValue(new Error('断网')), boardUndo: vi.fn(), boardRedo: vi.fn() };
    const editor = useMindMapEditor(board, ref(defaultBoardView()), api, async () => true); editor.install(original);
    editor.apply([{ kind: 'insert_topic', parent_item_id: 'root', index: 1, topic: topicInput('手工主题', 'local') }], 'insert'); await editor.flush();
    const remote = applyMindMapOperations(original, [{ kind: 'insert_topic', parent_item_id: 'root', index: 1, topic: topicInput('远程主题', 'remote') }]); remote.revision++;
    editor.receive(remote); expect(await editor.flush()).toBe(false); expect(api.boardEdit).toHaveBeenCalledTimes(1); expect(editor.error.value).toContain('同一分支');
    expect(board.value!.items.some(item => item.item_id === 'local')).toBe(true); expect(board.value!.items.some(item => item.item_id === 'remote')).toBe(true);
  });
  it('replays a failed edit against a newer unrelated branch without replacing that branch', async () => {
    const original = document(), board = ref<BoardDocument | null>(clone(original)); let stored = original;
    const api = { boardEdit: vi.fn().mockRejectedValueOnce(new Error('断网')).mockImplementation(async input => { expect(input.expected_revision).toBe(stored.revision); stored = { ...applyMindMapOperations(stored, input.operations), revision: stored.revision + 1 }; return stored; }), boardUndo: vi.fn(), boardRedo: vi.fn() };
    const editor = useMindMapEditor(board, ref(defaultBoardView()), api, async () => true); editor.install(original);
    editor.apply([{ kind: 'update_topic', item_id: 'read', title: '保留人工修改' }], 'rename'); await editor.flush();
    stored = applyMindMapOperations(original, [{ kind: 'update_topic', item_id: 'write', title: '另一支的更新' }]); stored.revision++;
    editor.receive(stored); expect(await editor.flush()).toBe(true);
    expect(board.value!.items.find(item => item.item_id === 'read')?.title).toBe('保留人工修改'); expect(board.value!.items.find(item => item.item_id === 'write')?.title).toBe('另一支的更新');
  });
});
