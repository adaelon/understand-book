// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import BoardNoteRelations from './BoardNoteRelations.vue';
import type { BoardItem } from '../generated/BoardItem';

const mocks = vi.hoisted(() => ({
  workspaceNotes: vi.fn(),
  workspaceNoteRelationSave: vi.fn(),
  workspaceNoteRelationDelete: vi.fn(),
  boardUpdate: vi.fn(),
}));
vi.mock('../api', () => ({ api: mocks }));
vi.mock('../network-context', () => ({ bindSceneApi: (api: unknown) => api, readerKey: {} }));

function noteItem(noteId: string, itemId = `item-${noteId}`, missing = false): BoardItem {
  return {
    item_id: itemId, title: `标题 ${noteId}`, readable_content: '笔记正文',
    explanation_blocks: [], source_bindings: [], created_at: '2026-10-27T00:00:00Z',
    position: { x: 0, y: 0 }, size: { width: 300, height: 240 },
    target: { kind: 'note', note_id: noteId, missing },
  } as BoardItem;
}
function relation(from = 'note-a', to = 'note-b') {
  return { relation_id: 'relation-one', from_note_id: from, to_note_id: to, label: '支持', reason: '具体例子支持这个观点。', revision: 4 };
}
function notebook(relations: ReturnType<typeof relation>[] = []) {
  return { notes: ['note-a', 'note-b', 'note-c'].map(note_id => ({ note_id, title: `标题 ${note_id}` })), relations };
}
type Panel = ReturnType<typeof mount<typeof BoardNoteRelations>>;
const panels: Panel[] = [];
async function setup(props: Partial<InstanceType<typeof BoardNoteRelations>['$props']> = {}) {
  const panel = mount(BoardNoteRelations, { props: { noteId: 'note-a', items: [noteItem('note-a'), noteItem('note-b')], ...props } });
  panels.push(panel);
  await flushPromises();
  return panel;
}
function button(panel: Panel, text: string) {
  const result = panel.findAll('button').find(candidate => candidate.text() === text);
  expect(result, `button ${text}`).toBeTruthy();
  return result!;
}
async function click(panel: Panel, text: string) {
  await button(panel, text).trigger('click');
  await flushPromises();
}

beforeEach(() => {
  vi.resetAllMocks();
  mocks.workspaceNotes.mockResolvedValue(notebook());
  mocks.workspaceNoteRelationSave.mockImplementation(async request => ({ relation_id: 'relation-new', revision: 1, ...request }));
  mocks.workspaceNoteRelationDelete.mockResolvedValue(undefined);
});
afterEach(() => { for (const panel of panels.splice(0)) panel.unmount(); });

describe('board note relations', () => {
  it('requires both a name and reason and saves canonical note ids rather than board item ids', async () => {
    const panel = await setup();
    await click(panel, '建立关系');
    expect(button(panel, '保存关系').attributes('disabled')).toBeDefined();
    await panel.get('input').setValue('  支持  ');
    await panel.get('textarea').setValue('   ');
    expect(button(panel, '保存关系').attributes('disabled')).toBeDefined();
    await panel.get('form').trigger('submit');
    expect(mocks.workspaceNoteRelationSave).not.toHaveBeenCalled();
    await panel.get('textarea').setValue('  这个观察提供了具体的例子。  ');
    expect(button(panel, '保存关系').attributes('disabled')).toBeUndefined();
    await panel.get('form').trigger('submit');
    await flushPromises();
    expect(mocks.workspaceNoteRelationSave).toHaveBeenCalledExactlyOnceWith({
      from_note_id: 'note-a', to_note_id: 'note-b', label: '支持', reason: '这个观察提供了具体的例子。',
    });
    expect(panel.find('form').exists()).toBe(false);
    expect(panel.get('.note-relation').text()).toContain('这个观察提供了具体的例子。');
    expect(panel.emitted('changed')).toHaveLength(1);
  });

  it('edits an existing relation with its revision and replaces its displayed content', async () => {
    mocks.workspaceNotes.mockResolvedValue(notebook([relation()]));
    const panel = await setup();
    await click(panel, '编辑关系');
    expect(panel.find('select').exists()).toBe(false);
    await panel.get('input').setValue('进一步解释');
    await panel.get('textarea').setValue('补充了这个观点成立的条件。');
    await panel.get('form').trigger('submit');
    await flushPromises();
    expect(mocks.workspaceNoteRelationSave).toHaveBeenCalledExactlyOnceWith({
      relation_id: 'relation-one', expected_revision: 4,
      from_note_id: 'note-a', to_note_id: 'note-b', label: '进一步解释', reason: '补充了这个观点成立的条件。',
    });
    expect(panel.findAll('.note-relation')).toHaveLength(1);
    expect(panel.get('.note-relation').text()).toContain('补充了这个观点成立的条件。');
    expect(panel.emitted('changed')).toHaveLength(1);
  });

  it('keeps the direction of an incoming relation when opening it from a board edge', async () => {
    mocks.workspaceNotes.mockResolvedValue(notebook([relation('note-b', 'note-a')]));
    const panel = await setup({ selectedRelationId: 'relation-one' });
    expect(panel.get('.note-relation-editing-title').text()).toBe('标题 note-b → 标题 note-a');
    const endpoints = panel.findAll('.note-relation-route button');
    await endpoints[0].trigger('click');
    await endpoints[1].trigger('click');
    expect(panel.emitted('open-note')).toEqual([['note-b'], ['note-a']]);
    await panel.get('textarea').setValue('这一方向保留原来的解释关系。');
    await panel.get('form').trigger('submit');
    await flushPromises();
    expect(mocks.workspaceNoteRelationSave).toHaveBeenCalledExactlyOnceWith({
      relation_id: 'relation-one', expected_revision: 4,
      from_note_id: 'note-b', to_note_id: 'note-a', label: '支持', reason: '这一方向保留原来的解释关系。',
    });
  });

  it('removes the canonical relation using its revision without updating board layout', async () => {
    mocks.workspaceNotes.mockResolvedValue(notebook([relation()]));
    const panel = await setup();
    await click(panel, '移除关系');
    expect(mocks.workspaceNoteRelationDelete).toHaveBeenCalledExactlyOnceWith({ relation_id: 'relation-one', expected_revision: 4 });
    expect(mocks.boardUpdate).not.toHaveBeenCalled();
    expect(panel.find('.note-relation').exists()).toBe(false);
    expect(panel.emitted('changed')).toHaveLength(1);
  });

  it('preserves the edited draft when saving fails and allows retrying it', async () => {
    mocks.workspaceNotes.mockResolvedValue(notebook([relation()]));
    mocks.workspaceNoteRelationSave.mockRejectedValueOnce(new Error('暂时无法保存'));
    const panel = await setup();
    await click(panel, '编辑关系');
    await panel.get('input').setValue('反例');
    await panel.get('textarea').setValue('这个情况说明原观点有适用条件。');
    await panel.get('form').trigger('submit');
    await flushPromises();
    expect(panel.get('[role="alert"]').text()).toContain('暂时无法保存');
    expect((panel.get('input').element as HTMLInputElement).value).toBe('反例');
    expect((panel.get('textarea').element as HTMLTextAreaElement).value).toBe('这个情况说明原观点有适用条件。');
    expect(panel.get('.note-relation strong').text()).toBe('支持');
    expect(panel.emitted('changed')).toBeUndefined();
    expect(button(panel, '保存关系').attributes('disabled')).toBeUndefined();
    await panel.get('form').trigger('submit');
    await flushPromises();
    expect(mocks.workspaceNoteRelationSave).toHaveBeenCalledTimes(2);
    expect(mocks.workspaceNoteRelationSave.mock.calls[1][0]).toEqual(mocks.workspaceNoteRelationSave.mock.calls[0][0]);
    expect(panel.find('[role="alert"]').exists()).toBe(false);
    expect(panel.find('form').exists()).toBe(false);
    expect(panel.emitted('changed')).toHaveLength(1);
  });

  it('offers each available target once and excludes self, missing notes and other board items', async () => {
    const answer: BoardItem = { ...noteItem('unused', 'answer-one'), target: { kind: 'answer', session_id: 'session-one', turn_id: 'turn-one', question: '问题', answer_parts: [] } };
    const panel = await setup({ items: [
      noteItem('note-a'), noteItem('note-a', 'self-again'), noteItem('note-b'),
      noteItem('note-b', 'duplicate-b'), noteItem('note-c', 'missing-c', true), answer,
    ] });
    await click(panel, '建立关系');
    expect(panel.findAll('select option').map(option => ({ value: option.attributes('value'), text: option.text() })))
      .toEqual([{ value: 'note-b', text: '标题 note-b' }]);
    await click(panel, '取消');
    await panel.setProps({ items: [noteItem('note-a'), noteItem('note-c', 'missing-c', true), answer] });
    expect(button(panel, '建立关系').attributes('disabled')).toBeDefined();
    expect(panel.text()).toContain('再向画板添加一条笔记，就可以建立关系。');
  });
});
