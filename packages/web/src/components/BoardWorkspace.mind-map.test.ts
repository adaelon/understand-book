// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, h, nextTick } from 'vue';
import BoardWorkspace from './BoardWorkspace.vue';
import BoardCanvas from './BoardCanvas.vue';
import { boardFixture } from '../fixtures/board-workspace';
import { defaultBoardView, type BoardQuestionRequest } from '../board-workspace';
import { applyMindMapOperations, topicInput } from '../mind-map-editor';
import type { BoardDocument } from '../generated/BoardDocument';
import type { BoardEditOperation } from '../generated/BoardEditOperation';

const mocks = vi.hoisted(() => ({ api: { boardConversation: vi.fn(), boardList: vi.fn(), boardRead: vi.fn(), boardUpdate: vi.fn(),
  boardViewSave: vi.fn(), boardSearch: vi.fn(), boardEdit: vi.fn(), boardUndo: vi.fn(), boardRedo: vi.fn() } }));
vi.mock('../api', () => ({ api: mocks.api }));
vi.mock('../network-context', () => ({ bindSceneApi: (api: unknown) => api, readerKey: () => 'reader-fixture' }));
vi.mock('./AgentPresentation.vue', () => ({ default: defineComponent({ setup(_, { expose }) { expose({ flushBoardState: async () => {} }); return () => h('div'); } }) }));

let stored = boardFixture();
const wrappers: ReturnType<typeof mount>[] = [];
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value));
beforeEach(() => {
  vi.clearAllMocks(); stored = boardFixture(); stored.document.items = []; stored.document.mind_maps = [];
  stored.document.edit_history = { transactions: [], undo_stack: [], redo_stack: [] };
  stored.view = defaultBoardView();
  mocks.api.boardConversation.mockResolvedValue({ board_id: stored.document.board_id, sessions: [] });
  mocks.api.boardSearch.mockResolvedValue({ items: [] });
  mocks.api.boardList.mockImplementation(async () => ({ items: [{ ...clone(stored.document), item_count: stored.document.items.length }] }));
  mocks.api.boardRead.mockImplementation(async () => clone(stored));
  mocks.api.boardViewSave.mockImplementation(async (_id, view) => { stored.view = clone(view); return { saved: true }; });
  mocks.api.boardUpdate.mockImplementation(async request => {
    expect(request.expected_revision).toBe(stored.document.revision);
    stored.document = { ...stored.document, revision: stored.document.revision + 1, ...(request.title ? { title: request.title } : {}) };
    return clone(stored.document);
  });
  mocks.api.boardEdit.mockImplementation(async (request: { expected_revision: number; operations: BoardEditOperation[]; action: string }) => {
    expect(request.expected_revision).toBe(stored.document.revision);
    const before = stored.document.revision, next = applyMindMapOperations(stored.document, request.operations);
    next.revision = before + 1;
    next.edit_history = { transactions: [...(stored.document.edit_history?.transactions ?? []), { revision: next.revision, before_revision: before, actor: 'human', action: request.action, undo_of: null, redo_of: null }],
      undo_stack: [...(stored.document.edit_history?.undo_stack ?? []), next.revision], redo_stack: [] };
    stored.document = next; return clone(next);
  });
  mocks.api.boardUndo.mockImplementation(async (_boardId: string, revision: number) => {
    expect(revision).toBe(stored.document.revision); return clone({ ...stored.document, revision: revision + 1 });
  });
});
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); });
async function setup() {
  const wrapper = mount(BoardWorkspace, { props: { boardId: stored.document.board_id }, attachTo: document.body });
  wrappers.push(wrapper); await flushPromises(); return wrapper;
}
function seedMap() {
  stored.document = applyMindMapOperations(stored.document, [
    { kind: 'create_mind_map', root: topicInput('Rust 切片', 'root'), map: { map_id: 'rust-map', root_item_id: 'root', layout: 'balanced', anchor: { x: 100, y: 150 }, branch_sides: {} } },
    { kind: 'insert_topic', parent_item_id: 'root', index: 0, topic: topicInput('只读切片', 'read') },
    { kind: 'insert_topic', parent_item_id: 'root', index: 1, topic: topicInput('可写切片', 'write') },
    { kind: 'insert_topic', parent_item_id: 'read', index: 0, topic: topicInput('读取 20 与 30', 'example') },
  ]);
  stored.view.selected_item_id = 'root'; stored.view.selected_item_ids = ['root'];
}
function button(wrapper: ReturnType<typeof setup> extends Promise<infer T> ? T : never, label: string) {
  const found = wrapper.findAll('button').find(candidate => candidate.text() === label);
  if (!found) throw new Error(`Missing button ${label}`); return found;
}
async function select(wrapper: Awaited<ReturnType<typeof setup>>, id: string) {
  wrapper.getComponent(BoardCanvas).vm.$emit('topic-select', id, false); await nextTick();
}
async function rename(wrapper: Awaited<ReturnType<typeof setup>>, title: string) {
  const input = wrapper.get('.board-canvas [data-topic-editor]');
  await input.setValue(title); await input.trigger('keydown', { key: 'Enter' }); await flushPromises();
}
function children(document: BoardDocument, id: string) {
  const item = document.items.find(item => item.item_id === id); return item?.target.kind === 'topic' ? item.target.children : [];
}

describe('mind map editing through the saved board workspace', () => {
  it('saves the current human title before regenerating a precise pending candidate without replaying its stale rename request', async () => {
    seedMap(); stored.document.revision = 9;
    const oldRequest = '把标题改成 AI 建议标题，并补充一个例子。';
    const candidate = { session_id: 'candidate-session', turn_id: 'candidate-turn', candidate_id: 'candidate-rust', candidate_revision: 3 };
    mocks.api.boardConversation.mockResolvedValue({ board_id: stored.document.board_id, sessions: [{ session_id: 'history-session', title: 'Rust 切片讨论', turns: [{
      turn_id: 'history-turn', user_turn_ordinal: 1, user: oldRequest, status: 'completed', outcome: null,
      question_source_label: null, question_quote: null, effect_labels: [],
      pending_board_edit: { ...candidate, current_board_revision: 7, item_ids: ['read'], message: '这个主题已由你更新，候选尚未应用。', summary: '修改主题标题并补充一个例子', requires_confirmation: false },
    }] }] });
    const wrapper = await setup(); await select(wrapper, 'read');
    await wrapper.get('.board-rail-tabs button:last-child').trigger('click');
    await wrapper.get('[data-topic-id="read"]').trigger('dblclick');
    const humanTitle = '人工标题：只能读取已有元素';
    await wrapper.get('.board-canvas [data-topic-editor]').setValue(humanTitle);
    expect(stored.document.items.find(item => item.item_id === 'read')?.title).toBe('只读切片');
    expect(mocks.api.boardEdit).not.toHaveBeenCalled();

    const persist = mocks.api.boardEdit.getMockImplementation()!;
    let releaseSave!: () => void;
    const saveGate = new Promise<void>(resolve => { releaseSave = resolve; });
    mocks.api.boardEdit.mockImplementationOnce(async request => { await saveGate; return persist(request); });
    const regenerate = button(wrapper, '按当前内容重新生成');
    expect(regenerate.isVisible()).toBe(true);
    await regenerate.trigger('click'); await flushPromises();
    expect(mocks.api.boardEdit).toHaveBeenCalledOnce();
    expect(mocks.api.boardEdit.mock.calls[0][0]).toMatchObject({ expected_revision: 9, operations: [{ kind: 'update_topic', item_id: 'read', title: humanTitle }] });
    expect(wrapper.emitted('follow-up')).toBeUndefined();

    releaseSave(); await flushPromises();
    expect(stored.document.items.find(item => item.item_id === 'read')?.title).toBe(humanTitle);
    expect(stored.document.revision).toBe(10);
    expect(wrapper.emitted('follow-up')).toHaveLength(1);
    const request = wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest;
    expect(request.context).toEqual({ board_id: stored.document.board_id, board_revision: 10, item_id: 'read', regenerate_from: candidate });
    expect(request.message).toBe('基于当前内容完成这次未应用的修改，保留冲突期间的人工编辑。');
    expect(request.message).not.toContain(oldRequest);
    expect(request.message).not.toContain('AI 建议标题');
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });

  it('creates and renames a map, adds child and sibling by keyboard, and focuses the committed topic', async () => {
    const wrapper = await setup();
    await button(wrapper, '添加思维导图').trigger('click'); await flushPromises();
    await rename(wrapper, 'Rust 切片');
    const root = stored.document.mind_maps![0].root_item_id;
    expect(document.activeElement?.getAttribute('data-topic-id')).toBe(root);
    await wrapper.get(`[data-topic-id="${root}"]`).trigger('keydown', { key: 'Tab' }); await flushPromises();
    await rename(wrapper, '只读切片 &[T]');
    const first = children(stored.document, root)[0];
    await wrapper.get(`[data-topic-id="${first}"]`).trigger('keydown', { key: 'Enter' }); await flushPromises();
    await rename(wrapper, '可写切片 &mut [T]');
    const ordered = children(stored.document, root).map(id => stored.document.items.find(item => item.item_id === id)?.title);
    expect(ordered).toEqual(['只读切片 &[T]', '可写切片 &mut [T]']);
    expect(stored.document.items).toHaveLength(3);
    expect(mocks.api.boardEdit.mock.calls.flatMap(call => call[0].operations.map((operation: BoardEditOperation) => operation.kind)))
      .toEqual(['create_mind_map', 'update_topic', 'insert_topic', 'update_topic', 'insert_topic', 'update_topic']);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });

  it('keeps a title draft and selection when switching between graph and outline', async () => {
    seedMap(); const wrapper = await setup(); await select(wrapper, 'read');
    await wrapper.get('[data-topic-id="read"]').trigger('dblclick');
    await wrapper.get('.board-canvas [data-topic-editor]').setValue('只读切片：文字还没保存');
    await button(wrapper, '大纲').trigger('click'); await nextTick();
    const outlineInput = wrapper.get('.mind-map-outline textarea');
    expect((outlineInput.element as HTMLTextAreaElement).value).toBe('只读切片：文字还没保存');
    await outlineInput.setValue('只读切片：可以读取，不能改写');
    await button(wrapper, '画布').trigger('click'); await nextTick();
    const graphInput = wrapper.get('.board-canvas [data-topic-editor]');
    expect((graphInput.element as HTMLTextAreaElement).value).toBe('只读切片：可以读取，不能改写');
    expect(document.activeElement).toBe(graphInput.element);
    expect(wrapper.get('[data-topic-id="read"]').attributes('aria-selected')).toBe('true');
    expect(stored.document.items.find(item => item.item_id === 'read')?.title).toBe('只读切片');
    await graphInput.trigger('keydown', { key: 'Enter' }); await flushPromises();
    expect(stored.document.items.find(item => item.item_id === 'read')?.title).toBe('只读切片：可以读取，不能改写');
  });

  it('persists collapsed topics and outline view, then unfolds ancestors when locating a search result', async () => {
    seedMap(); const wrapper = await setup(); await select(wrapper, 'read');
    await button(wrapper, '折叠').trigger('click'); await button(wrapper, '大纲').trigger('click');
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(stored.view.folded_topic_ids).toEqual(['read']); expect(stored.view.mind_map_outline).toBe(true);
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1);
    const reopened = await setup();
    expect(reopened.find('[data-outline-item="example"]').exists()).toBe(false);
    await reopened.get('[aria-label="查找大纲主题"]').setValue('读取 20');
    await reopened.get('[data-outline-item="example"] .outline-locate').trigger('click'); await flushPromises();
    expect(reopened.get('[data-outline-item="example"]').attributes('aria-selected')).toBe('true');
    expect(reopened.getComponent(BoardCanvas).props('foldedIds')).toEqual([]);
  });

  it.each([36, -2000])('locates an outline search result only after the graph is visible (camera x %s)', async cameraX => {
    seedMap(); stored.view.x = cameraX;
    const wrapper = await setup(), canvas = wrapper.getComponent(BoardCanvas);
    const surface = canvas.element as HTMLElement;
    Object.defineProperties(surface, {
      clientWidth: { configurable: true, get: () => surface.style.display === 'none' ? 0 : 1100 },
      clientHeight: { configurable: true, get: () => surface.style.display === 'none' ? 0 : 700 },
    });
    const initialView = { ...canvas.props('view') };
    await button(wrapper, '大纲').trigger('click');
    await wrapper.get('[aria-label="查找大纲主题"]').setValue('读取 20');
    await wrapper.get('[data-outline-item="example"] .outline-locate').trigger('click'); await flushPromises();
    expect(surface.clientWidth).toBe(0);
    expect(canvas.props('view')).toMatchObject({ x: initialView.x, y: initialView.y, zoom: initialView.zoom });
    await button(wrapper, '画布').trigger('click'); await nextTick();
    const nextView = canvas.props('view'), node = wrapper.get('[data-topic-id="example"]').element as HTMLElement;
    const left = parseFloat(node.style.left) * nextView.zoom + nextView.x;
    expect(surface.clientWidth).toBe(1100);
    expect(left).toBeGreaterThanOrEqual(24);
    expect(left + parseFloat(node.style.width) * nextView.zoom).toBeLessThanOrEqual(1076);
    expect(nextView.zoom).toBe(initialView.zoom);
    if (cameraX === 36) expect(nextView.x).toBe(initialView.x);
    expect(document.activeElement?.getAttribute('data-topic-id')).toBe('example');
  });

  it('moves a whole branch with searchable mobile controls and excludes its own descendants', async () => {
    seedMap(); const wrapper = await setup(); await select(wrapper, 'read');
    await button(wrapper, '移动到…').trigger('click');
    const options = wrapper.findAll('[role="option"]').map(option => option.text());
    expect(options.some(option => option.includes('读取 20'))).toBe(false);
    expect(options.some(option => option.startsWith('只读切片'))).toBe(false);
    await wrapper.get('[aria-label="搜索目标主题"]').setValue('可写');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(1);
    await wrapper.get('[role="option"]').trigger('click');
    await wrapper.get('form[aria-label="移动分支"]').trigger('submit'); await flushPromises();
    expect(children(stored.document, 'root')).toEqual(['write']);
    expect(children(stored.document, 'write')).toEqual(['read']);
    expect(children(stored.document, 'read')).toEqual(['example']);
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
  });

  it('creates, edits and removes an explicit relation without changing the topic tree', async () => {
    seedMap(); const wrapper = await setup(); await select(wrapper, 'read');
    await button(wrapper, '建立关联…').trigger('click');
    await wrapper.get('[aria-label="搜索目标主题"]').setValue('可写');
    await wrapper.get('[role="option"]').trigger('click');
    await wrapper.get('[aria-label="关系说明"]').setValue('访问权限不同');
    await wrapper.get('form[aria-label="建立主题关联"]').trigger('submit'); await flushPromises();
    expect(stored.document.links).toHaveLength(1);
    const id = stored.document.links[0].link_id;
    await wrapper.get(`[data-board-link="${id}"]`).trigger('click'); await flushPromises();
    await wrapper.get('[aria-label="编辑关联说明"]').setValue('只读不允许写入');
    await wrapper.get('[aria-label="编辑关联说明"]').trigger('blur'); await flushPromises();
    expect(stored.document.links[0]).toMatchObject({ link_id: id, label: '只读不允许写入' });
    await button(wrapper, '删除关联').trigger('click'); await flushPromises();
    expect(stored.document.links).toHaveLength(0);
    expect(children(stored.document, 'root')).toEqual(['read', 'write']);
  });

  it('leaves native text undo to the title editor and dispatches a single board undo outside it', async () => {
    seedMap(); const wrapper = await setup(); await select(wrapper, 'read');
    await wrapper.get('[data-topic-id="read"]').trigger('dblclick');
    const input = wrapper.get('.board-canvas [data-topic-editor]');
    await input.trigger('keydown', { key: 'z', ctrlKey: true });
    expect(mocks.api.boardUndo).not.toHaveBeenCalled();
    await input.trigger('keydown', { key: 'Escape' });
    await wrapper.get('[data-topic-id="read"]').trigger('keydown', { key: 'z', ctrlKey: true }); await flushPromises();
    expect(mocks.api.boardUndo).toHaveBeenCalledTimes(1);
  });

  it('stops active generation before undoing the committed board transaction', async () => {
    seedMap(); const wrapper = await setup();
    const run = { request_id: 'request-rust', board_id: stored.document.board_id, item_id: 'read', question: '补一个例子', status_text: '正在补充', accepted: true, pending: true, can_retry: false, can_stop: true };
    await wrapper.setProps({ run });
    await wrapper.trigger('keydown', { key: 'z', ctrlKey: true }); await flushPromises();
    expect(wrapper.emitted('cancel-follow-up')).toHaveLength(1);
    expect(mocks.api.boardUndo).not.toHaveBeenCalled();
    await wrapper.setProps({ run: { ...run, pending: false, can_stop: false } }); await flushPromises();
    expect(mocks.api.boardUndo).toHaveBeenCalledTimes(1);
    await wrapper.setProps({ run: { ...run, pending: false, can_stop: false, status_text: '已停止' } }); await flushPromises();
    expect(mocks.api.boardUndo).toHaveBeenCalledTimes(1);
  });

  it('keeps the draft visible when AI deletes its topic and saves the recovered text under a new identity', async () => {
    seedMap(); const wrapper = await setup(); await select(wrapper, 'read');
    await wrapper.get('[data-topic-id="read"]').trigger('dblclick');
    await wrapper.get('.board-canvas [data-topic-editor]').setValue('我的补充：只读借用不能改写原数组');
    const before = stored.document.revision;
    stored.document = applyMindMapOperations(stored.document, [{ kind: 'remove_subtree', item_id: 'read' }]);
    stored.document.revision = before + 1;
    await wrapper.setProps({ run: { request_id: 'request-delete', board_id: stored.document.board_id, item_id: 'read', question: '简化这一支', status_text: '已完成',
      accepted: true, pending: false, can_retry: false, can_stop: false,
      delivery: { board_id: stored.document.board_id, before_revision: before, after_revision: stored.document.revision, item_ids: [], block_ids: [], origin_session_id: 'rust-chat', origin_turn_id: 'delete-turn' } } });
    await flushPromises();
    const orphan = wrapper.get('[aria-label="已删除主题的草稿"]');
    expect((orphan.element as HTMLTextAreaElement).value).toBe('我的补充：只读借用不能改写原数组');
    expect(wrapper.find('[data-topic-id="read"]').exists()).toBe(false);
    await button(wrapper, '另存为新主题').trigger('click'); await flushPromises();
    const recovered = stored.document.items.find(item => item.title === '我的补充：只读借用不能改写原数组');
    expect(recovered).toBeDefined(); expect(recovered?.item_id).not.toBe('read');
    expect(stored.document.items.some(item => item.item_id === 'read' || item.item_id === 'example')).toBe(false);
    expect(wrapper.find('[aria-label="已删除主题的草稿"]').exists()).toBe(false);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });

  it('unfolds route targets and relation endpoints without making a content edit', async () => {
    seedMap(); stored.view.folded_topic_ids = ['root', 'read'];
    stored.document.links = [{ link_id: 'compare-access', from: { kind: 'item', item_id: 'example' }, to: { kind: 'item', item_id: 'write' }, label: '对比读写权限', based_on: [] }];
    stored.document.routes = [{ route_id: 'rust-route', title: '从只读到可写', steps: [{ step_id: 'compare', explanation: null,
      reveal_targets: [{ kind: 'item', item_id: 'example' }], focus_targets: [{ kind: 'link', link_id: 'compare-access' }], transition: 'instant', duration_ms: 500 }] }];
    const revision = stored.document.revision, wrapper = await setup();
    expect(wrapper.find('[data-topic-id="example"]').exists()).toBe(false);
    await wrapper.get('[aria-label="选择讲解路径"]').setValue('rust-route'); await flushPromises();
    expect(wrapper.getComponent(BoardCanvas).props('foldedIds')).toEqual([]);
    expect(wrapper.find('[data-topic-id="example"]').exists()).toBe(true);
    expect(stored.document.revision).toBe(revision);
    expect(mocks.api.boardEdit).not.toHaveBeenCalled();
    expect(mocks.api.boardUpdate).not.toHaveBeenCalled();
  });
});
