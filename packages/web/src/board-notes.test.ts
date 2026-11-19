// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import BoardWorkspace from './components/BoardWorkspace.vue';
import BoardCanvas from './components/BoardCanvas.vue';
import BoardReadingPane from './components/BoardReadingPane.vue';
import { boardFixture } from './fixtures/board-workspace';
import type { BoardItem } from './generated/BoardItem';

const mocks = vi.hoisted(() => ({ api: { boardConversation: vi.fn(), boardRead: vi.fn(), boardUpdate: vi.fn(), boardImport: vi.fn(), boardCatalog: vi.fn(), boardViewSave: vi.fn(), workspaceNoteCreate: vi.fn() } }));
vi.mock('./api', () => ({ api: mocks.api }));
vi.mock('./reader-context', async importOriginal => ({ ...await importOriginal<typeof import('./reader-context')>(), bindSceneApi: (api: unknown) => api, readerKey: () => 'notes-fixture' }));
vi.mock('./components/AgentPresentation.vue', () => ({ default: { template: '<div />' } }));
const note = (id: string, title = '观察与反馈', missing = false): BoardItem => ({
  item_id: `item-${id}`, title, readable_content: '练习后的具体反馈帮助确定下一次尝试。',
  target: { kind: 'note', note_id: id, missing }, position: { x: 0, y: 0 }, size: { width: 280, height: 190 },
  created_at: '2026-10-27T08:00:00Z', source_bindings: [], explanation_blocks: [],
});
let stored = boardFixture();
const wrappers: ReturnType<typeof mount>[] = [];
beforeEach(() => {
  vi.clearAllMocks(); stored = boardFixture(); stored.document.items = [note('note-a')];
  stored.view.selected_item_id = 'item-note-a'; stored.view.reading_item_id = 'item-note-a';
  mocks.api.boardRead.mockImplementation(async () => structuredClone(stored));
  mocks.api.boardConversation.mockResolvedValue({ board_id: stored.document.board_id, sessions: [] });
  mocks.api.boardViewSave.mockResolvedValue({ saved: true });
  mocks.api.boardCatalog.mockResolvedValue({ items: [] });
  mocks.api.workspaceNoteCreate.mockImplementation(async request => ({ note_id: 'created-note', ...request, revision: 1, source_bindings: [] }));
  mocks.api.boardImport.mockImplementation(async request => {
    const added = note(request.source.note_id, '新的想法');
    if (!stored.document.items.some(item => item.item_id === added.item_id)) stored.document.items.push(added);
    stored.document.revision++; return structuredClone(stored.document);
  });
  mocks.api.boardUpdate.mockImplementation(async request => {
    stored.document.items = stored.document.items.filter(item => !request.remove_item_ids?.includes(item.item_id));
    stored.document.revision++; return structuredClone(stored.document);
  });
});
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); });
async function setup() {
  const wrapper = mount(BoardWorkspace, { props: { boardId: stored.document.board_id, notesRevision: 0 }, global: { stubs: { BoardNoteRelations: true, AgentPresentation: true } } });
  wrappers.push(wrapper); await flushPromises(); return wrapper;
}
async function newNoteForm(wrapper: Awaited<ReturnType<typeof setup>>) {
  await wrapper.get('.board-add-button').trigger('click');
  await wrapper.findAll('[role="tab"]').find(tab => tab.text() === '空间笔记')!.trigger('click');
  await flushPromises();
  await wrapper.get('.board-note-create-trigger').trigger('click');
  await wrapper.get('.board-note-create-form input').setValue('新的想法');
  await wrapper.get('.board-note-create-form textarea').setValue('练习与解释如何连接起来。');
}

describe('shared workspace notes on boards', () => {
  it('creates one canonical note then imports its identity into the current board', async () => {
    const wrapper = await setup(); await newNoteForm(wrapper);
    await wrapper.get('.board-note-create-form').trigger('submit'); await flushPromises();
    expect(mocks.api.workspaceNoteCreate).toHaveBeenCalledWith({ title: '新的想法', content: '练习与解释如何连接起来。' });
    expect(mocks.api.boardImport).toHaveBeenCalledWith(expect.objectContaining({ source: { kind: 'note', note_id: 'created-note' } }));
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    expect(wrapper.emitted('notes-changed')).toHaveLength(1);
    expect(wrapper.getComponent(BoardCanvas).props('view').selected_item_id).toBe('item-created-note');
    expect(wrapper.text()).toContain('与空间笔记同步');
  });
  it('keeps the created identity when board import fails, so retry does not create a duplicate', async () => {
    mocks.api.boardImport.mockRejectedValueOnce(new Error('连接暂时中断'));
    const wrapper = await setup(); await newNoteForm(wrapper);
    await wrapper.get('.board-note-create-form').trigger('submit'); await flushPromises();
    expect(wrapper.get('.board-note-create-form').text()).toContain('笔记已保存在学习空间');
    expect(wrapper.get('.board-note-create-form input').attributes('disabled')).toBeDefined();
    await wrapper.get('.board-note-create-form').trigger('submit'); await flushPromises();
    expect(mocks.api.workspaceNoteCreate).toHaveBeenCalledTimes(1);
    expect(mocks.api.boardImport).toHaveBeenCalledTimes(2);
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
  });
  it('opens the canonical note editor and refreshes changed content without discarding unsaved layout', async () => {
    const wrapper = await setup();
    await wrapper.get('.board-note-edit').trigger('click'); await flushPromises();
    expect(wrapper.emitted('open-note')?.[0]).toEqual(['note-a']);
    wrapper.getComponent(BoardCanvas).vm.$emit('move', 'item-note-a', { x: 210, y: 140 });
    stored.document.items[0].title = '修订后的观察'; stored.document.items[0].readable_content = '这是在统一笔记中更新的正文。';
    await wrapper.setProps({ notesRevision: 1 }); await flushPromises();
    expect(wrapper.get('.board-prose').text()).toContain('这是在统一笔记中更新的正文');
    expect(wrapper.getComponent(BoardCanvas).props('items')[0].position).toEqual({ x: 210, y: 140 });
    expect(wrapper.getComponent(BoardCanvas).props('view').selected_item_id).toBe('item-note-a');
  });
  it('selects the shared relation when its projected line is activated', async () => {
    stored.document.items.push({ ...note('note-b', '分解练习'), position: { x: 350, y: 0 } });
    stored.document.links = [{ link_id: 'note-link', from: { kind: 'item', item_id: 'item-note-a' }, to: { kind: 'item', item_id: 'item-note-b' }, label: '支持', reason: '具体反馈能帮助决定如何拆分下一次练习。', note_relation_id: 'relation-a', based_on: [] }];
    const wrapper = await setup();
    await wrapper.get('[data-board-link="note-link"]').trigger('click'); await flushPromises();
    const pane = wrapper.findAllComponents(BoardReadingPane).find(pane => pane.props('item')?.item_id === 'item-note-a')!;
    expect(pane.props('selectedRelationId')).toBe('relation-a');
    expect(wrapper.get('[data-board-link="note-link"] title').text()).toContain('具体反馈');
    pane.vm.$emit('notes-changed'); await flushPromises();
    expect(mocks.api.boardRead).toHaveBeenCalledTimes(2);
    expect(wrapper.emitted('notes-changed')).toHaveLength(1);
  });
  it('shows deleted notes explicitly and removes only the board card', async () => {
    stored.document.items = [note('note-a', '原来的笔记', true)];
    const wrapper = await setup();
    expect(wrapper.get('.board-note-missing').text()).toContain('这条笔记已删除');
    expect(wrapper.find('.board-note-edit').exists()).toBe(false);
    expect(wrapper.find('board-note-relations-stub').exists()).toBe(false);
    await wrapper.get('.board-item-actions button').trigger('click'); await flushPromises();
    expect(mocks.api.boardUpdate).toHaveBeenCalledWith(expect.objectContaining({ remove_item_ids: ['item-note-a'] }));
    expect(wrapper.find('.board-card').exists()).toBe(false);
  });
});
