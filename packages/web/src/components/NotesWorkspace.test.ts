// @vitest-environment happy-dom
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import NotesWorkspace from './NotesWorkspace.vue';
import type { WorkspaceNote } from '../generated/WorkspaceNote';
import type { WorkspaceNoteRelation } from '../generated/WorkspaceNoteRelation';
import { network } from '../reader-context';

const mocks = vi.hoisted(() => ({
  workspaceNotes: vi.fn(), workspaceNote: vi.fn(), workspaceNoteCreate: vi.fn(), workspaceNoteUpdate: vi.fn(),
  workspaceNoteDelete: vi.fn(), workspaceNoteRelationSave: vi.fn(), workspaceNoteRelationDelete: vi.fn(),
}));
vi.mock('../api', async importOriginal => ({ ...await importOriginal<typeof import('../api')>(), api: mocks }));
const wrappers: VueWrapper[] = [];
const first: WorkspaceNote = { note_id: 'note-1', learning_workspace_id: 'learning-1', title: '反馈需要指向具体的表现', content: '知道哪里发生了变化，才能决定下一次怎么练习。', revision: 2, created_at: '2026-10-26T09:00:00Z', updated_at: '2026-10-27T09:00:00Z', author: 'user', updated_by: 'user', origin: null, source_bindings: [] };
const second: WorkspaceNote = { ...first, note_id: 'note-2', title: '把一次尝试变成可比较的记录', content: '录下短片段，并留下当时的问题。', revision: 1, updated_at: '2026-10-26T09:00:00Z' };
const link: WorkspaceNoteRelation = { relation_id: 'relation-1', from_note_id: first.note_id, to_note_id: second.note_id, label: '需要', reason: '比较前后的尝试，才能确认反馈是否有用。', revision: 1, author: 'user', created_at: '2026-10-27T10:00:00Z', updated_at: '2026-10-27T10:00:00Z' };
beforeEach(() => {
  vi.resetAllMocks();
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  mocks.workspaceNotes.mockResolvedValue({ notes: [{ ...first }, { ...second }], relations: [] });
  mocks.workspaceNoteUpdate.mockImplementation(async payload => ({ ...first, ...payload, revision: payload.expected_revision + 1 }));
  mocks.workspaceNoteCreate.mockImplementation(async payload => ({ ...first, ...payload, note_id: 'created-note', revision: 1 }));
  mocks.workspaceNote.mockResolvedValue(first);
  mocks.workspaceNoteRelationSave.mockImplementation(async payload => ({ ...link, ...payload }));
  mocks.workspaceNoteDelete.mockResolvedValue({ deleted: true });
  mocks.workspaceNoteRelationDelete.mockResolvedValue({ deleted: true });
});
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); });
async function start(props: Record<string, unknown> = {}) {
  const wrapper = mount(NotesWorkspace, { props: { learningWorkspaceId: 'learning-1', ...props } });
  wrappers.push(wrapper); await flushPromises(); return wrapper;
}
function button(wrapper: VueWrapper, text: string) { return wrapper.findAll('button').find(item => item.text() === text)!; }
const editor = (wrapper: VueWrapper) => wrapper.get<HTMLTextAreaElement>('.notes-content-input');
const leave = (wrapper: VueWrapper) => (wrapper.vm as unknown as { beforeLeave(): Promise<boolean> }).beforeLeave();

it('keeps unsaved text across note navigation and finds it through full-text search', async () => {
  const wrapper = await start();
  expect(mocks.workspaceNotes).toHaveBeenCalledWith('learning-1');
  await editor(wrapper).setValue('新的观察：换弓时有一个停顿。');
  await wrapper.get('[data-note-id="note-2"]').trigger('click');
  expect(editor(wrapper).element.value).toBe(second.content);
  await wrapper.get('[data-note-id="note-1"]').trigger('click');
  expect(editor(wrapper).element.value).toContain('换弓');
  await wrapper.get('input[type="search"]').setValue('换弓');
  expect(wrapper.findAll('[data-note-id]')).toHaveLength(1);
  expect(wrapper.get('[data-note-id]').attributes('data-note-id')).toBe('note-1');
  expect(mocks.workspaceNoteUpdate).not.toHaveBeenCalled();
});

it('saves all retained drafts before leaving and emits changes with the explicit workspace scope', async () => {
  const wrapper = await start();
  await editor(wrapper).setValue('修改后的第一条');
  await wrapper.get('[data-note-id="note-2"]').trigger('click');
  await editor(wrapper).setValue('修改后的第二条');
  expect(await leave(wrapper)).toBe(true);
  expect(mocks.workspaceNoteUpdate).toHaveBeenNthCalledWith(1, { note_id: 'note-1', expected_revision: 2, title: first.title, content: '修改后的第一条' }, 'learning-1');
  expect(mocks.workspaceNoteUpdate).toHaveBeenNthCalledWith(2, { note_id: 'note-2', expected_revision: 1, title: second.title, content: '修改后的第二条' }, 'learning-1');
  expect(wrapper.emitted('changed')).toHaveLength(2);
});

it('keeps failed drafts and blocks navigation, then lets the user discard to the latest saved revision', async () => {
  mocks.workspaceNoteUpdate.mockRejectedValue(new Error('笔记已由其他窗口修改'));
  mocks.workspaceNote.mockResolvedValue({ ...first, content: '另一窗口保存的版本', revision: 3 });
  const wrapper = await start();
  await editor(wrapper).setValue('这次尚未保存的思考');
  await wrapper.get('[data-note-id="note-2"]').trigger('click');
  expect(await leave(wrapper)).toBe(false); await flushPromises();
  expect(editor(wrapper).element.value).toBe('这次尚未保存的思考');
  expect(wrapper.get('[role="alert"]').text()).toContain('草稿已保留');
  expect(wrapper.emitted('close')).toBeUndefined();
  await button(wrapper, '放弃修改').trigger('click');
  await wrapper.get('.notes-confirmation .notes-destructive').trigger('click'); await flushPromises();
  expect(editor(wrapper).element.value).toBe('另一窗口保存的版本');
  expect(mocks.workspaceNote).toHaveBeenCalledWith('note-1', 'learning-1');
  expect(await leave(wrapper)).toBe(true);
});

it('creates one canonical note before asking the Agent and ignores composition shortcuts', async () => {
  mocks.workspaceNotes.mockResolvedValue({ notes: [], relations: [] });
  const wrapper = await start();
  await button(wrapper, '写下第一条笔记').trigger('click');
  await wrapper.get('.notes-title-input').setValue('一次练习的发现');
  await editor(wrapper).setValue('## 观察\n\n节奏在换弓处发生变化。');
  await editor(wrapper).trigger('compositionstart');
  await editor(wrapper).trigger('keydown', { ctrlKey: true, key: 'Enter' });
  expect(mocks.workspaceNoteCreate).not.toHaveBeenCalled();
  await editor(wrapper).trigger('compositionend');
  await wrapper.findAll('.notes-action')[1].trigger('click'); await flushPromises();
  expect(mocks.workspaceNoteCreate).toHaveBeenCalledTimes(1);
  expect(mocks.workspaceNoteCreate).toHaveBeenCalledWith({ title: '一次练习的发现', content: '## 观察\n\n节奏在换弓处发生变化。' }, 'learning-1');
  expect(wrapper.emitted('ask')?.[0]).toEqual([{ noteId: 'created-note', title: '一次练习的发现', content: '## 观察\n\n节奏在换弓处发生变化。' }]);
  await button(wrapper, '预览').trigger('click');
  expect(wrapper.get('.notes-prose h2').text()).toBe('观察');
});

it('requires a meaningful relation, preserves its draft while navigating, and follows incoming links', async () => {
  const wrapper = await start();
  await wrapper.get('[aria-label="添加笔记关联"]').trigger('click');
  await wrapper.get('.notes-relation-form select').setValue('note-2');
  await wrapper.get('.notes-relation-form input').setValue('需要');
  expect(await leave(wrapper)).toBe(false); await flushPromises();
  expect(wrapper.get('.notes-relation-form [role="alert"]').text()).toContain('关联名称和原因');
  expect(mocks.workspaceNoteRelationSave).not.toHaveBeenCalled();
  await wrapper.get('[data-note-id="note-2"]').trigger('click');
  await wrapper.get('[data-note-id="note-1"]').trigger('click');
  expect(wrapper.get<HTMLInputElement>('.notes-relation-form input').element.value).toBe('需要');
  await wrapper.get('.notes-relation-form textarea').setValue(link.reason);
  await wrapper.get('.notes-relation-form').trigger('submit'); await flushPromises();
  expect(mocks.workspaceNoteRelationSave).toHaveBeenCalledWith({ from_note_id: 'note-1', to_note_id: 'note-2', label: '需要', reason: link.reason }, 'learning-1');
  await wrapper.get('.notes-related-title').trigger('click');
  expect(wrapper.get<HTMLInputElement>('.notes-title-input').element.value).toBe(second.title);
  expect(wrapper.get('.notes-relation-group h3').text()).toBe('指向这条笔记');
  await wrapper.get('.notes-related-title').trigger('click');
  expect(wrapper.get<HTMLInputElement>('.notes-title-input').element.value).toBe(first.title);
});

it('requires confirmation to delete a note and removes its relationships from the notebook', async () => {
  mocks.workspaceNotes.mockResolvedValue({ notes: [first, second], relations: [link] });
  const wrapper = await start();
  await wrapper.get('.notes-delete-note').trigger('click');
  expect(mocks.workspaceNoteDelete).not.toHaveBeenCalled();
  expect(wrapper.get('[role="dialog"]').attributes('aria-modal')).toBe('true');
  await wrapper.get('.notes-confirmation .cancel').trigger('click');
  expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
  await wrapper.get('.notes-delete-note').trigger('click');
  await wrapper.get('.notes-confirmation .notes-destructive').trigger('click'); await flushPromises();
  expect(mocks.workspaceNoteDelete).toHaveBeenCalledWith({ note_id: 'note-1', expected_revision: 2 }, 'learning-1');
  expect(wrapper.find('[data-note-id="note-1"]').exists()).toBe(false);
  expect(wrapper.find('.notes-relation-card').exists()).toBe(false);
  expect(wrapper.emitted('changed')).toHaveLength(1);
});

it('shows original author separately from recent edits and sends canonical note identity when opening a source', async () => {
  mocks.workspaceNotes.mockResolvedValue({ notes: [{ ...first, author: 'unknown', updated_by: 'assistant', origin: { kind: 'reading_note', book_id: 'book-1', publication_id: null, session_id: null, turn_id: null, mem_id: 'mem-1' }, source_bindings: [{ source_ref_id: 'source-1', book_id: 'book-1', evidence_text_digest: '', label_snapshot: '练习记录，第 12 页', preview_snapshot: '保留下来的原文片段' }] }], relations: [] });
  const wrapper = await start();
  expect(wrapper.get('.notes-provenance').text()).toContain('最初内容原有笔记');
  expect(wrapper.get('.notes-provenance').text()).toContain('最近编辑Agent 写作');
  await wrapper.get('.notes-source-link').trigger('click');
  expect(wrapper.emitted('source')?.[0][0]).toEqual({ note_id: 'note-1', source_ref_id: 'source-1' });
  expect(wrapper.emitted('source')?.[0][1]).toBe(wrapper.get('.notes-source-link').element);
});

it('opens archived notebooks in read mode and disables mutations', async () => {
  const wrapper = await start({ readonly: true });
  expect(wrapper.text()).toContain('已归档，笔记仅供阅读');
  expect(wrapper.find('.notes-prose').exists()).toBe(true);
  expect(wrapper.find('.notes-content-input').exists()).toBe(false);
  expect(button(wrapper, '新建笔记').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.notes-delete-note').attributes('disabled')).toBeDefined();
  expect(wrapper.get('[aria-label="添加笔记关联"]').attributes('disabled')).toBeDefined();
  expect(await leave(wrapper)).toBe(true);
});

it('recovers a failed initial load and restores the requested note', async () => {
  mocks.workspaceNotes.mockRejectedValueOnce(new Error('暂时离线'));
  const wrapper = await start({ initialNoteId: 'note-2' });
  expect(wrapper.get('[role="alert"]').text()).toContain('暂时离线');
  await button(wrapper, '重新载入').trigger('click'); await flushPromises();
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect(editor(wrapper).element.value).toBe(second.content);
});

it('keeps a draft reachable when another editor deletes its note and recovers it as a new note', async () => {
  const wrapper = await start();
  await editor(wrapper).setValue('另一处删除时，我仍在整理的内容');
  mocks.workspaceNotes.mockResolvedValue({ notes: [second], relations: [] });
  await (wrapper.vm as unknown as { refresh(): Promise<void> }).refresh(); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('已被删除，草稿已保留');
  expect(wrapper.find('[data-note-id="note-1"]').exists()).toBe(true);
  await button(wrapper, '另存为新笔记').trigger('click'); await flushPromises();
  expect(mocks.workspaceNoteCreate).toHaveBeenCalledWith({ title: first.title, content: '另一处删除时，我仍在整理的内容' }, 'learning-1');
  expect(wrapper.find('[data-note-id="note-1"]').exists()).toBe(false);
  expect(wrapper.find('[data-note-id="created-note"]').exists()).toBe(true);
  expect(editor(wrapper).element.value).toBe('另一处删除时，我仍在整理的内容');
  expect(await leave(wrapper)).toBe(true);
});
