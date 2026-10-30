// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { defineComponent, h } from 'vue';
import { mount, flushPromises } from '@vue/test-utils';
import BoardWorkspace from './components/BoardWorkspace.vue';
import BoardCanvas from './components/BoardCanvas.vue';
import { boardFixture, boardFixtureCatalog, boardFixtureItems, boardFixtureReceipt } from './fixtures/board-workspace';
import { boardDate, fitBoard, zoomBoardAt, projectBoardRoute, type BoardQuestionRequest, type BoardRunView } from './board-workspace';
import BoardReadingPane from './components/BoardReadingPane.vue';
import { requestWorkspaceRecovery } from './network-client';

const mocks = vi.hoisted(() => ({ api: { boardConversation: vi.fn(), boardList: vi.fn(), boardCreate: vi.fn(), boardRead: vi.fn(), boardUpdate: vi.fn(), boardImport: vi.fn(), boardDelete: vi.fn(), boardCatalog: vi.fn(), boardViewSave: vi.fn(), boardSearch: vi.fn() }, flushScene: vi.fn(), mounts: 0 }));
vi.mock('./api', () => ({ api: mocks.api }));
vi.mock('./network-client', () => ({ requestWorkspaceRecovery: vi.fn() }));
vi.mock('./network-context', () => ({ bindSceneApi: (api: unknown) => api, readerKey: () => 'reader-fixture' }));
vi.mock('./components/AgentPresentation.vue', () => ({ default: defineComponent({ setup(_, { expose }) { mocks.mounts++; expose({ flushBoardState: mocks.flushScene, locateRegion: () => true }); return () => h('div', { class: 'retained-presentation-fixture' }, '可操作的留存演示'); } }) }));
let stored = boardFixture();
let wrappers: ReturnType<typeof mount>[] = [];
let restoreFullscreen = () => {};
function mockBrowserFullscreen() {
  let element: Element | null = null;
  const request = vi.fn(async () => { element = document.documentElement; document.dispatchEvent(new Event('fullscreenchange')); });
  const exit = vi.fn(async () => { element = null; document.dispatchEvent(new Event('fullscreenchange')); });
  const properties: [object, string, PropertyDescriptor][] = [
    [document, 'fullscreenElement', { get: () => element }],
    [document.documentElement, 'requestFullscreen', { value: request }],
    [document, 'exitFullscreen', { value: exit }],
  ];
  const originals = properties.map(([target, key]) => Object.getOwnPropertyDescriptor(target, key));
  for (const [target, key, descriptor] of properties) Object.defineProperty(target, key, { ...descriptor, configurable: true });
  restoreFullscreen = () => { properties.forEach(([target, key], index) => { if (originals[index]) Object.defineProperty(target, key, originals[index]!); else Reflect.deleteProperty(target, key); }); };
  return { request, exit, escape: () => { element = null; document.dispatchEvent(new Event('fullscreenchange')); } };
}
beforeEach(() => {
  vi.clearAllMocks(); mocks.mounts = 0; stored = boardFixture();
  mocks.flushScene.mockResolvedValue(undefined);
  mocks.api.boardConversation.mockResolvedValue({ board_id: 'board-bayes', sessions: [] });
  mocks.api.boardSearch.mockResolvedValue({ items: [] });
  mocks.api.boardList.mockImplementation(async () => ({ items: [{ ...stored.document, item_count: stored.document.items.length }] }));
  mocks.api.boardRead.mockImplementation(async () => structuredClone(stored));
  mocks.api.boardViewSave.mockImplementation(async (_id, view) => { stored.view = JSON.parse(JSON.stringify(view)); return { saved: true }; });
  mocks.api.boardUpdate.mockImplementation(async request => {
    expect(request.expected_revision).toBe(stored.document.revision);
    stored.document.revision++;
    if (request.title) stored.document.title = request.title;
    for (const position of request.positions ?? []) stored.document.items.find(item => item.item_id === position.item_id)!.position = { ...position.position };
    stored.document.items = stored.document.items.filter(item => !request.remove_item_ids?.includes(item.item_id));
    return structuredClone(stored.document);
  });
  mocks.api.boardCatalog.mockImplementation(async kind => ({ items: structuredClone(boardFixtureCatalog.filter(item => item.kind === kind)) }));
  mocks.api.boardImport.mockImplementation(async request => {
    expect(request.expected_revision).toBe(stored.document.revision);
    stored.document.revision++;
    stored.document.items.push(structuredClone(boardFixtureItems.find(item => item.target.kind === request.source.kind)!));
    return structuredClone(stored.document);
  });
});
afterEach(() => { for (const wrapper of wrappers) wrapper.unmount(); wrappers = []; vi.useRealTimers(); restoreFullscreen(); restoreFullscreen = () => {}; });
async function setup(attached = false) { const wrapper = mount(BoardWorkspace, { props: { boardId: stored.document.board_id }, ...(attached ? { attachTo: document.body } : {}) }); wrappers.push(wrapper); await flushPromises(); return wrapper; }
describe('knowledge board saved workspace', () => {
  it('opens true browser fullscreen, reveals reading and questions on demand, and restores the prior layout without remounting content', async () => {
    const fullscreen = mockBrowserFullscreen();
    const wrapper = await setup(true);
    const canvas = wrapper.getComponent(BoardCanvas).element;
    const pane = wrapper.get('.board-reading-pane').element;
    const selected = wrapper.getComponent(BoardCanvas).props('view').selected_item_id;
    await wrapper.findAll('.board-rail-tabs [role="tab"]').find(tab => tab.text() === '讨论')!.trigger('click');
    await wrapper.get('textarea').setValue('没有发送的理解');
    await wrapper.get('[aria-label="全屏画板"]').trigger('click'); await flushPromises();
    expect(fullscreen.request).toHaveBeenCalledOnce();
    expect(wrapper.classes()).toContain('board-canvas-fullscreen');
    expect((wrapper.get('.board-topbar').element as HTMLElement).style.display).toBe('none');
    expect((wrapper.get('.board-side-rail').element as HTMLElement).style.display).toBe('none');
    expect(wrapper.get('.board-conversation').isVisible()).toBe(false);
    await wrapper.findAll('.board-fullscreen-tools button').find(button => button.text() === '讲解')!.trigger('click');
    expect((wrapper.get('.board-side-rail').element as HTMLElement).style.display).not.toBe('none');
    await wrapper.findAll('.board-fullscreen-tools button').find(button => button.text() === '提问')!.trigger('click');
    expect(wrapper.get('.board-side-rail .board-conversation').isVisible()).toBe(true);
    expect(wrapper.get('textarea').element).toHaveProperty('value', '没有发送的理解');
    await wrapper.get('.board-fullscreen-tools [aria-label="退出全屏"]').trigger('click'); await flushPromises();
    expect(fullscreen.exit).toHaveBeenCalledOnce();
    expect(wrapper.classes()).not.toContain('board-canvas-fullscreen');
    expect((wrapper.get('.board-topbar').element as HTMLElement).style.display).not.toBe('none');
    expect(wrapper.get('[role="tab"][aria-selected="true"]').text()).toBe('讨论');
    expect(wrapper.getComponent(BoardCanvas).element).toBe(canvas);
    expect(wrapper.get('.board-reading-pane').element).toBe(pane);
    expect(wrapper.getComponent(BoardCanvas).props('view').selected_item_id).toBe(selected);
    expect(wrapper.get('textarea').element).toHaveProperty('value', '没有发送的理解');
  });

  it('keeps the composer and its draft with the reading/discussion rail when it is collapsed and reopened', async () => {
    const wrapper = await setup(true);
    const input = wrapper.get('.board-side-rail .board-question-row textarea');
    await input.setValue('请画出这个过程的分支和回路');
    await wrapper.get('[aria-label="收起讲解与讨论"]').trigger('click');
    expect(input.isVisible()).toBe(false);
    await wrapper.get('[aria-label="展开讲解与讨论"]').trigger('click');
    expect(input.isVisible()).toBe(true);
    await wrapper.findAll('.board-rail-tabs [role="tab"]').find(tab => tab.text() === '讨论')!.trigger('click');
    expect(wrapper.findAll('.board-question-row textarea')).toHaveLength(1);
    expect(input.element).toHaveProperty('value', '请画出这个过程的分支和回路');
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    const request = wrapper.emitted('follow-up')?.[0]?.[0] as BoardQuestionRequest;
    expect(request.message).toBe('请画出这个过程的分支和回路');
    expect(request.context.item_id).toBe('item-answer');
  });

  it('restores collapsed rails on native Escape and identifies denied fullscreen as window mode', async () => {
    const fullscreen = mockBrowserFullscreen();
    const wrapper = await setup();
    await wrapper.get('[aria-label="收起讲解与讨论"]').trigger('click');
    await wrapper.get('[aria-label="全屏画板"]').trigger('click'); await flushPromises();
    fullscreen.escape(); await flushPromises();
    expect(wrapper.classes()).not.toContain('board-canvas-fullscreen');
    expect((wrapper.get('.board-side-rail').element as HTMLElement).style.display).toBe('none');
    expect(wrapper.get('[aria-label="展开讲解与讨论"]').isVisible()).toBe(true);
    fullscreen.request.mockRejectedValueOnce(new Error('fullscreen blocked'));
    await wrapper.get('[aria-label="全屏画板"]').trigger('click'); await flushPromises();
    expect(wrapper.get('.board-fullscreen-notice').text()).toContain('浏览器未允许全屏');
    expect(wrapper.classes()).toContain('board-canvas-fullscreen');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); await flushPromises();
    expect(wrapper.classes()).not.toContain('board-canvas-fullscreen');
    expect((wrapper.get('.board-side-rail').element as HTMLElement).style.display).toBe('none');
  });
  it('automatically retries a failed board read on recovery without writing or losing the selected board', async () => {
    mocks.api.boardRead.mockRejectedValueOnce(new TypeError('temporary connection failure'));
    const wrapper = await setup();
    expect(wrapper.text()).toContain('画板没有打开');
    expect(requestWorkspaceRecovery).toHaveBeenCalled();
    window.dispatchEvent(new Event('workspace-recovered')); await flushPromises();
    expect(wrapper.get('[aria-label="画板名称"]').element).toHaveProperty('value', stored.document.title);
    expect(mocks.api.boardRead).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).not.toContain('temporary connection failure');
    expect(mocks.api.boardUpdate).not.toHaveBeenCalled();
    expect(mocks.api.boardCreate).not.toHaveBeenCalled();
  });

  it('retries failed discussion reads while retaining the mounted board and unsent question', async () => {
    mocks.api.boardConversation.mockRejectedValueOnce(new TypeError('temporary discussion failure'));
    const wrapper = await setup();
    const canvas = wrapper.getComponent(BoardCanvas).element;
    const textarea = wrapper.find('textarea');
    await textarea.setValue('尚未发送的白板问题');
    window.dispatchEvent(new Event('workspace-recovered')); await flushPromises();
    expect(mocks.api.boardConversation).toHaveBeenCalledTimes(2);
    expect(wrapper.getComponent(BoardCanvas).element).toBe(canvas);
    expect(wrapper.find('textarea').element).toHaveProperty('value', '尚未发送的白板问题');
    expect(mocks.api.boardRead).toHaveBeenCalledTimes(1);
    expect(wrapper.emitted('follow-up')).toBeUndefined();
  });

  it('searches the full prebuilt catalog and sends category and pagination with the exact artifact locator', async () => {
    const artifact = { kind: 'artifact', artifact: 'discourse', entry_id: '14.9.3', source_revision: 'published-source' } as const;
    mocks.api.boardCatalog.mockImplementation(async (kind, after, options) => ({
      items: options?.category === 'discourse' ? [{ kind: 'prebuild', category: 'discourse', title: '远处的篇章内容', summary: '正文中的系统开销', source: artifact }] : [],
      next: options?.category === 'discourse' && after == null ? 60 : null, total: 61,
    }));
    const wrapper = await setup();
    await wrapper.get('.board-add-button').trigger('click'); await flushPromises();
    await wrapper.get('[aria-label="预构建内容类型"]').setValue('discourse'); await flushPromises();
    expect(mocks.api.boardCatalog).toHaveBeenLastCalledWith('prebuild', undefined, { category: 'discourse', query: '' });
    await wrapper.get('.board-catalog-more').trigger('click'); await flushPromises();
    expect(mocks.api.boardCatalog).toHaveBeenLastCalledWith('prebuild', 60, { category: 'discourse', query: '' });
    vi.useFakeTimers();
    try {
      await wrapper.get('.board-catalog-search').setValue('系统开销');
      await vi.advanceTimersByTimeAsync(250); await flushPromises();
      expect(mocks.api.boardCatalog).toHaveBeenLastCalledWith('prebuild', undefined, { category: 'discourse', query: '系统开销' });
      expect(wrapper.get('.board-catalog-item').text()).toContain('远处的篇章内容');
      mocks.api.boardImport.mockImplementation(async request => {
        expect(request.source).toEqual(artifact);
        return structuredClone(stored.document);
      });
      await wrapper.get('.board-catalog-item').trigger('click'); await flushPromises();
      expect(mocks.api.boardImport).toHaveBeenCalledOnce();
    } finally { vi.useRealTimers(); }
  });
  it('reopens all four retained content kinds, the complete answer and exact source address after layout and title saves', async () => {
    const wrapper = await setup();
    expect(wrapper.findAll('.board-card')).toHaveLength(4);
    expect(wrapper.get('.board-reading-content').text()).toContain('基准发生率改变，同样的检测结果也会得到不同的后验判断。');
    await wrapper.get('[aria-label="画板名称"]').setValue('我的概率画板');
    wrapper.getComponent(BoardCanvas).vm.$emit('move', 'item-answer', { x: 480, y: 90 });
    wrapper.getComponent(BoardCanvas).vm.$emit('view', { ...stored.view, zoom: .75, x: 15, selected_item_id: 'item-answer' });
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(wrapper.emitted('close')).toHaveLength(1);
    expect(stored.document.title).toBe('我的概率画板'); expect(stored.document.items[1].position).toEqual({ x: 480, y: 90 }); expect(stored.view.zoom).toBe(.75);
    wrapper.unmount(); wrappers = [];
    const reopened = await setup();
    expect(reopened.findAll('.board-card')).toHaveLength(4);
    expect((reopened.get('[aria-label="画板名称"]').element as HTMLInputElement).value).toBe('我的概率画板');
    await reopened.get('.board-reading-sources button').trigger('click'); await flushPromises();
    expect(reopened.emitted('source')?.[0][0]).toEqual({ board_id: 'board-bayes', item_id: 'item-answer', source_ref_id: 'source-bayes' });
  });
  it('keeps failed changes visible and prevents closing until a successful retry', async () => {
    const wrapper = await setup(); const original = mocks.api.boardUpdate.getMockImplementation()!;
    mocks.api.boardUpdate.mockRejectedValueOnce(new Error('磁盘写入失败'));
    wrapper.getComponent(BoardCanvas).vm.$emit('move', 'item-answer', { x: 650, y: 60 });
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(wrapper.emitted('close')).toBeUndefined(); expect(wrapper.get('[role="alert"]').text()).toContain('磁盘写入失败');
    expect(wrapper.get('[data-item-id="item-answer"]').attributes('style')).toContain('left: 650px');
    mocks.api.boardUpdate.mockImplementation(original);
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(wrapper.emitted('close')).toHaveLength(1); expect(stored.document.items[1].position.x).toBe(650);
  });
  it('imports actual catalog locators for all four types and preserves the original quote independently of interpretation', async () => {
    stored.document.items = []; stored.view.selected_item_id = null;
    const wrapper = await setup();
    for (const kind of ['prebuild', 'answer', 'presentation', 'understanding'] as const) {
      await wrapper.get('.board-add-button').trigger('click'); await flushPromises();
      const tab = wrapper.findAll('[role="tab"]').find(item => item.text() === ({ prebuild: '材料知识', answer: '问答讲解', presentation: '互动演示', understanding: '我的理解' }[kind]))!;
      await tab.trigger('click'); await flushPromises();
      await wrapper.get('.board-catalog-item').trigger('click'); await flushPromises();
    }
    expect(stored.document.items.map(item => item.target.kind)).toEqual(['prebuild', 'answer', 'presentation', 'understanding']);
    expect(wrapper.get('.board-understanding-quote').text()).toContain('阳性的人里面，也会有健康的人');
    expect(wrapper.get('.board-understanding-interpretation').text()).toContain('下一步可以尝试独立算出');
    expect(mocks.api.boardImport.mock.calls.map(call => call[0].source.kind)).toEqual(['prebuild', 'answer', 'presentation', 'understanding']);
  });
  it('keeps an opened presentation mounted while switching cards and flushes its scene before source navigation', async () => {
    const wrapper = await setup();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-presentation'); await flushPromises();
    expect(mocks.mounts).toBe(1);
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-understanding'); await flushPromises();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-presentation'); await flushPromises();
    expect(mocks.mounts).toBe(1);
    mocks.flushScene.mockRejectedValueOnce(new Error('演示保存失败'));
    const active = wrapper.findAll('.board-reading-pane').find(pane => pane.text().includes('互动演示 · 保留内容与操作现场'))!;
    await active.get('.board-reading-sources button').trigger('click'); await flushPromises();
    expect(wrapper.emitted('source')).toBeUndefined(); expect(wrapper.get('[role="alert"]').text()).toContain('演示保存失败');
    await active.get('.board-reading-sources button').trigger('click'); await flushPromises();
    expect(wrapper.emitted('source')).toHaveLength(1);
  });
  it('does not strand the saving queue after an invalid empty title', async () => {
    const wrapper = await setup();
    await wrapper.get('[aria-label="画板名称"]').setValue('');
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(wrapper.emitted('close')).toBeUndefined();
    await wrapper.get('[aria-label="画板名称"]').setValue('重新命名后可以保存');
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(stored.document.title).toBe('重新命名后可以保存'); expect(wrapper.emitted('close')).toHaveLength(1);
  });
  it('lets the user explicitly discard stale local changes and resume from another window’s saved revision', async () => {
    const wrapper = await setup(); const original = mocks.api.boardUpdate.getMockImplementation()!;
    stored.document.title = '另一个窗口保存的标题'; stored.document.revision = 4;
    mocks.api.boardUpdate.mockRejectedValueOnce(new Error('BOARD_REVISION_CONFLICT'));
    await wrapper.get('[aria-label="画板名称"]').setValue('本地尚未保存的修改');
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(wrapper.emitted('close')).toBeUndefined();
    await wrapper.findAll('button').find(button => button.text() === '放弃未保存修改并重新载入')!.trigger('click'); await flushPromises();
    expect((wrapper.get('[aria-label="画板名称"]').element as HTMLInputElement).value).toBe('另一个窗口保存的标题');
    mocks.api.boardUpdate.mockImplementation(original);
    await wrapper.get('[aria-label="画板名称"]').setValue('基于新版本继续整理');
    await wrapper.get('[aria-label="关闭画板，回到阅读"]').trigger('click'); await flushPromises();
    expect(stored.document.revision).toBe(5); expect(wrapper.emitted('close')).toHaveLength(1);
  });
  it('opens a retained embedded demonstration from its place inside the full answer', async () => {
    const answer = stored.document.items[1];
    if (answer.target.kind !== 'answer') throw new Error('wrong fixture');
    answer.target.answer_parts = [{ kind: 'markdown', text: '先解释为什么要考虑先验。' }, { kind: 'presentation', presentation_id: 'presentation-bayes', revision: 1 }, { kind: 'markdown', text: '再用演示中的观察总结判断。' }];
    const wrapper = await setup();
    expect(wrapper.get('.board-reading-content').text()).toContain('先解释为什么要考虑先验。');
    expect(wrapper.get('.board-reading-content').text()).toContain('再用演示中的观察总结判断。');
    await wrapper.get('.board-answer-presentation').trigger('click'); await flushPromises();
    expect(mocks.mounts).toBe(1);
    expect(wrapper.get('.retained-presentation-fixture').isVisible()).toBe(true);
  });
  it('can reach older catalog entries beyond the first page', async () => {
    mocks.api.boardCatalog.mockImplementation(async (kind, after) => ({ items: after === 100 ? [{ ...boardFixtureCatalog[3], title: '更早的一次理解' }] : boardFixtureCatalog.filter(item => item.kind === kind), next: after === 100 ? null : 100 }));
    const wrapper = await setup(); await wrapper.get('.board-add-button').trigger('click'); await flushPromises();
    await wrapper.findAll('[role="tab"]').find(tab => tab.text() === '我的理解')!.trigger('click'); await flushPromises();
    await wrapper.get('.board-catalog-more').trigger('click'); await flushPromises();
    expect(mocks.api.boardCatalog).toHaveBeenLastCalledWith('understanding', 100);
    expect(wrapper.findAll('.board-catalog-item').map(item => item.text())).toContain('更早的一次理解用户能够用总体人数解释阳性结果，需要进一步区分似然与后验。');
  });
});
describe('board questions and durable answer navigation', () => {
  it('allows an empty board to start from a topic and keeps all three mobile views available', async () => {
    stored.document.items = []; stored.view.selected_item_id = null; stored.view.reading_item_id = null;
    const wrapper = await setup();
    expect(wrapper.get('textarea').attributes('disabled')).toBeUndefined();
    expect(wrapper.get('.board-mobile-switch').text()).toBe('画板讲解讨论');
    await wrapper.get('textarea').setValue('状态矩阵与上下文有什么关系？');
    expect(wrapper.get('.board-question-send').attributes('disabled')).toBeUndefined();
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    expect(wrapper.emitted('follow-up')).toHaveLength(1);
    expect((wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest).context).toEqual({ board_id: 'board-bayes', board_revision: 1 });
    expect(wrapper.get('.board-body').classes()).toContain('mobile-discussion');
  });
  it('clears a focused block when returning to whole-board context', async () => {
    stored.view.selected_block_id = 'block-item-answer';
    const wrapper = await setup();
    await wrapper.get('.board-whole-context').trigger('click');
    await wrapper.get('textarea').setValue('把整张画板整理成关系图');
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    expect((wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest).context).not.toHaveProperty('item_id');
    expect((wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest).context).not.toHaveProperty('block_id');
  });
  it('keeps original board history and the actual active draft when the selected chat changes', async () => {
    mocks.api.boardConversation.mockResolvedValue({ board_id: 'board-bayes', sessions: [{ session_id: 'original-chat', title: '第一次讨论', turns: [{
      turn_id: 'original-turn', user_turn_ordinal: 1, user: '状态矩阵保存了什么？', status: 'completed', question_source_label: null, question_quote: null, effect_labels: [],
      outcome: { answer: '它保留到当前时刻为止的历史信息。', incomplete: false },
    }] }] });
    const wrapper = await setup();
    const request = { client_request_id: 'question-current', message: 'token 又如何改变状态？', context: { board_id: 'board-bayes', board_revision: 1, item_id: 'item-answer' } };
    await wrapper.setProps({ run: run(request, { turn_id: 'current-turn', discussion: {
      sessionId: 'current-chat', turnId: 'current-turn', requestId: 'question-current', user: request.message, outcome: null, pending: true,
      draft: { message_id: 1, revision: 1, operation: 'replace', view: { parts: [{ kind: 'markdown', text: '每个新 token 都会带来一次状态更新。' }], sources: [] } },
    } }) });
    await flushPromises();
    await wrapper.setProps({ run: null });
    await wrapper.findAll('.board-rail-tabs button').find(button => button.text().startsWith('讨论'))!.trigger('click');
    expect(wrapper.findAll('.board-discussion-turn')).toHaveLength(2);
    expect(wrapper.get('.board-discussion').text()).toContain('它保留到当前时刻为止的历史信息。');
    expect(wrapper.get('.board-discussion .answer-draft').text()).toContain('每个新 token');
  });
  function run(request: BoardQuestionRequest, extra: Partial<BoardRunView> = {}): BoardRunView {
    return { request_id: request.client_request_id, board_id: request.context.board_id, item_id: request.context.item_id, question: request.message, accepted: true, pending: true, can_retry: false, can_stop: true, status_text: '正在理解这个问题', ...extra };
  }
  it('clears the previous board delivery notice when opening a new empty board, even while the old run is retained', async () => {
    const wrapper = await setup();
    const delivery = { board_id: stored.document.board_id, before_revision: 1, after_revision: 1, item_ids: ['item-answer'], block_ids: [], origin_session_id: 'chat', origin_turn_id: 'turn' };
    const completed = { request_id: 'old-request', board_id: delivery.board_id, item_id: null, question: '整理画板', accepted: true, pending: false, can_retry: false, can_stop: false, status_text: '回答已保存到画板', delivery };
    await wrapper.setProps({ run: completed }); await flushPromises();
    expect(wrapper.text()).toContain('新的讲解和图解已加入画板');
    stored.document = { ...stored.document, board_id: 'new-empty', title: '新的知识画板', items: [], deliveries: [], links: [], mappings: [], routes: [] };
    await wrapper.setProps({ boardId: 'new-empty' }); await flushPromises();
    expect(wrapper.text()).not.toContain('新的讲解和图解已加入画板');
    await wrapper.setProps({ run: { ...completed } }); await flushPromises();
    expect(wrapper.text()).not.toContain('新的讲解和图解已加入画板');
    expect(wrapper.findAll('.board-card')).toHaveLength(0);
  });
  it('freezes the original item and exact captured scene while selection and a later scene change during preparation', async () => {
    const wrapper = await setup();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-presentation'); await flushPromises();
    let completeCapture!: (value: typeof boardFixtureReceipt) => void;
    mocks.flushScene.mockImplementationOnce(() => new Promise(resolve => { completeCapture = resolve; }));
    await wrapper.get('textarea').setValue('刚才这个滑块为什么改变结论？');
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-understanding'); await flushPromises();
    const exact = { ...boardFixtureReceipt, state_revision: 8, saved_state_ref: 'scene-at-submit' };
    completeCapture(exact); await flushPromises();
    const request = wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest;
    expect(request.context.item_id).toBe('item-presentation'); expect(request.context.presentation).toEqual(exact);
    expect(request.message).toBe('刚才这个滑块为什么改变结论？');
    expect(request).not.toHaveProperty('presentation_follow_up');
    expect(wrapper.get('[data-item-id="item-understanding"] button').attributes('aria-pressed')).toBe('true');
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe(request.message);
    await wrapper.setProps({ run: run(request) });
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('');
    expect(wrapper.getComponent(BoardCanvas).props('disabled')).toBe(false);
    await wrapper.setProps({ run: null });
    await wrapper.get('textarea').setValue('切换讨论后继续提问');
    expect(wrapper.get('.board-question-send').attributes('disabled')).toBeUndefined();
  });
  it('retains a rejected draft and retries the original request after selecting another card, without clearing a newer draft', async () => {
    const wrapper = await setup();
    await wrapper.get('textarea').setValue('请继续解释基准发生率');
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    const request = wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest;
    expect(request.context.block_id).toBe('block-item-answer');
    await wrapper.setProps({ run: run(request, { accepted: false, pending: false, error: '暂时无法接纳', can_retry: true, can_stop: false }) });
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe(request.message);
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-understanding'); await flushPromises();
    await wrapper.findAll('button').find(button => button.text() === '重试这次追问')!.trigger('click');
    expect(wrapper.emitted('retry-follow-up')?.[0]).toEqual([request.client_request_id]);
    await wrapper.get('textarea').setValue('这是我新写的下一问');
    await wrapper.setProps({ run: run(request) });
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('这是我新写的下一问');
  });
  it('does not emit a question when scene capture fails, and leaves the text ready for a successful retry', async () => {
    const wrapper = await setup(); wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-presentation'); await flushPromises();
    await wrapper.get('textarea').setValue('解释当前现场'); mocks.flushScene.mockRejectedValueOnce(new Error('现场没有保存成功'));
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    expect(wrapper.emitted('follow-up')).toBeUndefined(); expect(wrapper.get('.board-question-error').text()).toContain('现场没有保存成功');
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('解释当前现场');
    mocks.flushScene.mockResolvedValueOnce(boardFixtureReceipt);
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises(); expect(wrapper.emitted('follow-up')).toHaveLength(1);
  });
  it('merges a completed durable answer without stealing selection or remounting the live presentation, then locates the answer and parent', async () => {
    const wrapper = await setup(); wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-presentation'); await flushPromises();
    mocks.flushScene.mockResolvedValue(boardFixtureReceipt);
    await wrapper.get('textarea').setValue('解释当前现场'); await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    const request = wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest;
    await wrapper.setProps({ run: run(request) });
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-understanding'); await flushPromises();
    const camera = { ...wrapper.getComponent(BoardCanvas).props('view'), x: 103, y: 83, zoom: .7, selected_item_id: 'item-understanding' };
    wrapper.getComponent(BoardCanvas).vm.$emit('view', camera); await flushPromises();
    const delivery = { board_id: stored.document.board_id, before_revision: 1, after_revision: 2, item_ids: ['reply-1'], block_ids: ['block-reply-1'], origin_session_id: 'chat-board', origin_turn_id: 'turn-board', parent_item_id: 'item-presentation' };
    stored.document.items.push({ ...structuredClone(boardFixtureItems[1]), item_id: 'reply-1', block_id: 'block-reply-1', reply_to_item_id: 'item-presentation', title: '接着解释先验的变化', position: { x: 650, y: 260 } });
    stored.document.deliveries.push(delivery); stored.document.revision = 2;
    await wrapper.setProps({ run: run(request, { pending: false, can_stop: false, delivery }) }); await flushPromises();
    expect(wrapper.findAll('.board-card')).toHaveLength(5); expect(mocks.mounts).toBe(1);
    expect(wrapper.getComponent(BoardCanvas).props('view')).toMatchObject(camera);
    expect(wrapper.find('[data-board-reply-link="reply-1"]').exists()).toBe(true);
    const scrollIntoView = vi.fn();
    const originalScroll = HTMLElement.prototype.scrollIntoView;
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    await wrapper.findAll('button').find(button => button.text() === '定位新回答')!.trigger('click'); await flushPromises();
    HTMLElement.prototype.scrollIntoView = originalScroll;
    expect(scrollIntoView).not.toHaveBeenCalled(); // Only the reading pane scrolls, not its ancestors.
    expect(wrapper.get('.board-located .board-reading-heading').attributes('tabindex')).toBe('-1');
    expect(wrapper.get('[data-item-id="reply-1"] button').attributes('aria-pressed')).toBe('true');
    expect(wrapper.find('.board-located [data-board-block-id="block-reply-1"]').exists()).toBe(true);
    await wrapper.get('.board-reply-parent').trigger('click'); await flushPromises();
    expect(wrapper.get('[data-item-id="item-presentation"] button').attributes('aria-pressed')).toBe('true'); expect(mocks.mounts).toBe(1);
  });
  it('searches full text across boards and handles a stable external understanding or explanation location', async () => {
    const wrapper = await setup();
    const other = boardFixture(); other.document.board_id = 'other-board'; other.document.title = '另一张长期画板';
    mocks.api.boardRead.mockImplementation(async id => structuredClone(id === 'other-board' ? other : stored));
    mocks.api.boardSearch.mockResolvedValue({ items: [{ board_id: 'other-board', board_revision: 1, board_title: '另一张长期画板', item_id: 'item-answer', block_id: 'block-item-answer', kind: 'answer', title: '为什么阳性不等于真的患病？', snippet: '### 全文里保留的**基准发生率**解释' }] });
    await wrapper.get('.board-search-trigger').trigger('click'); await wrapper.get('input[type="search"]').setValue('基准发生率');
    await new Promise(resolve => setTimeout(resolve, 260)); await flushPromises();
    expect(mocks.api.boardSearch).toHaveBeenCalledWith('基准发生率');
    expect(wrapper.get('.board-search-result p').text()).toBe('全文里保留的基准发生率解释');
    await wrapper.get('.board-search-result').trigger('click'); await flushPromises();
    expect(wrapper.emitted('board-open')?.at(-1)).toEqual(['other-board']);
    expect(wrapper.find('.board-located [data-board-block-id="block-item-answer"]').exists()).toBe(true);
    await wrapper.setProps({ pendingLocate: { board_id: 'other-board', item_id: 'item-understanding', query: '阳性的人里面' } }); await flushPromises();
    expect(wrapper.emitted('located')).toHaveLength(1);
    expect(wrapper.get('[data-item-id="item-understanding"] button').attributes('aria-pressed')).toBe('true');
  });
  it('keeps the saved delivery visible after a failed refresh and retries without losing the current selection', async () => {
    const wrapper = await setup();
    const delivery = { board_id: stored.document.board_id, before_revision: 1, after_revision: 2, item_ids: ['reply-1'], block_ids: ['block-reply-1'], origin_session_id: 'chat-board', origin_turn_id: 'turn-board', parent_item_id: 'item-answer' };
    stored.document.items.push({ ...structuredClone(boardFixtureItems[1]), item_id: 'reply-1', block_id: 'block-reply-1', reply_to_item_id: 'item-answer' }); stored.document.revision = 2;
    mocks.api.boardRead.mockRejectedValueOnce(new Error('读取暂时失败'));
    const request = { client_request_id: 'request-1', message: '继续解释', context: { board_id: stored.document.board_id, board_revision: 1, item_id: 'item-answer' } };
    await wrapper.setProps({ run: run(request, { pending: false, can_stop: false, delivery }) }); await flushPromises();
    expect(wrapper.get('.board-delivery-notice').text()).toContain('定位新回答');
    expect(wrapper.get('.board-question-error').text()).toContain('回答已保存，画板刷新失败');
    expect(wrapper.findAll('.board-card')).toHaveLength(4);
    await wrapper.get('.board-question-error button').trigger('click'); await flushPromises();
    expect(wrapper.findAll('.board-card')).toHaveLength(5);
    expect(wrapper.find('.board-question-error').exists()).toBe(false);
    expect(wrapper.get('[data-item-id="item-answer"] button').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('.board-delivery-notice').text()).toContain('定位新回答');
  });
});
describe('WB3 linked explanations and narration', () => {
  function mockReadingGeometry() {
    return vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      const pane = this.closest<HTMLElement>('.board-reading-pane');
      const stack = this.closest<HTMLElement>('.board-reader-stack');
      if (!pane || stack?.style.display === 'none') return new DOMRect();
      const blockOffset = { 'explain-prior': 300, 'explain-total': 900, 'explain-conclusion': 1400 }[this.dataset.boardBlockId ?? ''];
      return new DOMRect(0, 100 + (blockOffset === undefined ? 0 : blockOffset - pane.scrollTop), 420, 180);
    });
  }
  function narrated() {
    const answer = stored.document.items[1];
    answer.explanation_blocks = [
      { block_id: 'explain-prior', markdown: '先确定人群中的基准发生率。', answer_parts: [] },
      { block_id: 'explain-total', markdown: '再比较所有阳性的人数。', answer_parts: [] },
      { block_id: 'explain-conclusion', markdown: '最后解释后验概率的条件。', answer_parts: [] },
    ];
    const references = answer.explanation_blocks.map(block => ({ item_id: answer.item_id, block_id: block.block_id, content_revision: 1 }));
    stored.document.links = [{ link_id: 'prior-to-total', from: { kind: 'item', item_id: 'item-prebuild' }, to: { kind: 'item', item_id: 'item-understanding' }, label: '比较', based_on: references.slice(0, 1) }];
    stored.document.mappings = [{ mapping_id: 'prior-map', explanation: references[0], visual: { kind: 'item', item_id: 'item-prebuild' } },
      { mapping_id: 'total-map', explanation: references[1], visual: { kind: 'item', item_id: 'item-understanding' } }];
    stored.document.routes = [{ route_id: 'route-steps', title: '从先验到后验', steps: [
      { step_id: 'step-prior', explanation: references[0], reveal_targets: [{ kind: 'item', item_id: 'item-prebuild' }], focus_targets: [{ kind: 'item', item_id: 'item-prebuild' }], transition: 'draw', duration_ms: 500 },
      { step_id: 'step-total', explanation: references[1], reveal_targets: [{ kind: 'link', link_id: 'prior-to-total' }], focus_targets: [{ kind: 'item', item_id: 'item-understanding' }], transition: 'draw', duration_ms: 500 },
      { step_id: 'step-conclusion', explanation: references[2], reveal_targets: [{ kind: 'item', item_id: 'item-answer' }], focus_targets: [{ kind: 'item', item_id: 'item-answer' }], transition: 'reveal_text', duration_ms: 500 },
    ] }];
    return references;
  }
  it('derives the same revealed prefix when stepping back, jumping and reopening', () => {
    narrated(); const route = stored.document.routes[0];
    expect(projectBoardRoute(stored.document, route, 'step-total').visibleItemIds).toEqual(['item-prebuild', 'item-understanding']);
    expect(projectBoardRoute(stored.document, route, 'step-total').visibleLinkIds).toEqual(['prior-to-total']);
    expect(projectBoardRoute(stored.document, route, 'step-prior').visibleItemIds).toEqual(['item-prebuild']);
    expect(projectBoardRoute(stored.document, route, 'step-prior').visibleLinkIds).toEqual([]);
    expect(projectBoardRoute(stored.document, route, 'step-total', true).visibleItemIds).toBeUndefined();
  });
  it('reveals unscheduled narration connections when both endpoints are shown, including after a backward seek', () => {
    narrated(); const route = stored.document.routes[0];
    route.steps[1].reveal_targets = [{ kind: 'item', item_id: 'item-understanding' }];
    stored.document.links.unshift({ link_id: 'comparison-to-answer', from: { kind: 'link', link_id: 'prior-to-total' },
      to: { kind: 'item', item_id: 'item-answer' }, label: '解释这个关系', based_on: [] });
    expect(projectBoardRoute(stored.document, route, 'step-prior').visibleLinkIds).toEqual([]);
    expect(projectBoardRoute(stored.document, route, 'step-total').visibleLinkIds).toEqual(['prior-to-total']);
    expect(projectBoardRoute(stored.document, route, 'step-conclusion').visibleLinkIds?.sort()).toEqual(['comparison-to-answer', 'prior-to-total']);
    expect(projectBoardRoute(stored.document, route, 'step-total').visibleLinkIds).toEqual(['prior-to-total']);
    expect(projectBoardRoute(stored.document, route, 'step-prior').visibleLinkIds).toEqual([]);
    const reopened = structuredClone(stored.document);
    expect(projectBoardRoute(reopened, reopened.routes[0], 'step-total').visibleLinkIds).toEqual(['prior-to-total']);
  });
  it('keeps explicitly scheduled narration connections hidden until their reveal step even when both endpoints are visible', () => {
    narrated(); const route = stored.document.routes[0];
    route.steps[0].reveal_targets.push({ kind: 'item', item_id: 'item-understanding' });
    expect(projectBoardRoute(stored.document, route, 'step-prior').visibleItemIds).toEqual(['item-prebuild', 'item-understanding']);
    expect(projectBoardRoute(stored.document, route, 'step-prior').visibleLinkIds).toEqual([]);
    expect(projectBoardRoute(stored.document, route, 'step-total').visibleLinkIds).toEqual(['prior-to-total']);
  });
  it('brings the endpoints of focused narration connections and their link dependencies into view', () => {
    narrated(); const route = stored.document.routes[0];
    stored.document.links.push({ link_id: 'comparison-to-answer', from: { kind: 'link', link_id: 'prior-to-total' },
      to: { kind: 'item', item_id: 'item-answer' }, label: '解释这个关系', based_on: [] });
    route.steps[0].reveal_targets = [{ kind: 'item', item_id: 'item-understanding' }];
    route.steps[0].focus_targets = [{ kind: 'link', link_id: 'comparison-to-answer' }];
    const projection = projectBoardRoute(stored.document, route, 'step-prior');
    expect(projection.visibleItemIds?.sort()).toEqual(['item-answer', 'item-prebuild', 'item-understanding']);
    expect(projection.visibleLinkIds?.sort()).toEqual(['comparison-to-answer', 'prior-to-total']);
  });
  it('preserves narration connections through playback completion, pause, reopening, backward seek and full-board view', async () => {
    vi.useFakeTimers(); narrated();
    stored.document.routes[0].steps[1].reveal_targets = [{ kind: 'item', item_id: 'item-understanding' }];
    const wrapper = await setup();
    expect(wrapper.find('[data-board-link="prior-to-total"]').exists()).toBe(true);
    await wrapper.get('[aria-label="播放讲解"]').trigger('click'); await flushPromises();
    expect(wrapper.find('[data-board-link="prior-to-total"]').exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(550); await flushPromises();
    expect(wrapper.get('.board-playback').attributes('data-step-id')).toBe('step-total');
    expect(wrapper.find('[data-board-link="prior-to-total"]').exists()).toBe(true);
    await wrapper.get('[aria-label="暂停讲解"]').trigger('click');
    await vi.advanceTimersByTimeAsync(800); await flushPromises();
    expect(wrapper.get('.board-playback').attributes()).toMatchObject({ 'data-playing': 'false', 'data-step-id': 'step-total' });
    expect(wrapper.find('[data-board-link="prior-to-total"]').exists()).toBe(true);
    await wrapper.get('[aria-label="播放讲解"]').trigger('click'); await flushPromises();
    await vi.advanceTimersByTimeAsync(1100); await flushPromises();
    expect(wrapper.get('.board-playback').attributes()).toMatchObject({ 'data-playing': 'false', 'data-step-id': 'step-conclusion' });
    expect(wrapper.find('[data-board-link="prior-to-total"]').exists()).toBe(true);
    await (wrapper.vm as unknown as { flush(): Promise<boolean> }).flush();
    wrapper.unmount(); wrappers = [];
    const reopened = await setup();
    expect(reopened.get('.board-playback').attributes()).toMatchObject({ 'data-playing': 'false', 'data-step-id': 'step-conclusion' });
    expect(reopened.find('[data-board-link="prior-to-total"]').exists()).toBe(true);
    await reopened.get('[aria-label="上一步"]').trigger('click'); await flushPromises();
    expect(reopened.find('[data-board-link="prior-to-total"]').exists()).toBe(true);
    await reopened.get('[aria-label="上一步"]').trigger('click'); await flushPromises();
    expect(reopened.find('[data-board-link="prior-to-total"]').exists()).toBe(false);
    await reopened.get('.board-show-all').trigger('click'); await flushPromises();
    expect(reopened.find('[data-board-link="prior-to-total"]').exists()).toBe(true);
    expect(mocks.api.boardUpdate).not.toHaveBeenCalled();
  });
  it('scrolls to the saved explanation block on mount without focusing the heading', async () => {
    narrated(); stored.view.selected_block_id = 'explain-total';
    const geometry = mockReadingGeometry();
    try {
      const wrapper = await setup();
      const pane = wrapper.get('.board-reading-pane').element as HTMLElement;
      expect(pane.scrollTop).toBe(880);
      expect(document.activeElement).not.toBe(wrapper.get('.board-reading-heading').element);
    } finally { geometry.mockRestore(); }
  });
  it('locates the current route block after returning from discussion instead of measuring the hidden pane', async () => {
    narrated(); const geometry = mockReadingGeometry();
    try {
      const wrapper = await setup();
      const pane = wrapper.get('.board-reading-pane').element as HTMLElement;
      await wrapper.findAll('.board-rail-tabs button').find(button => button.text().startsWith('讨论'))!.trigger('click');
      await wrapper.get('[aria-label="选择讲解路径"]').setValue('route-steps');
      await wrapper.get('[aria-label="跳到讲解步骤"]').setValue('step-conclusion'); await flushPromises();
      expect(wrapper.get('.block-highlighted').attributes('data-board-block-id')).toBe('explain-conclusion');
      expect((wrapper.get('.board-reader-stack').element as HTMLElement).style.display).toBe('none');
      await wrapper.findAll('.board-rail-tabs button').find(button => button.text() === '讲解')!.trigger('click'); await flushPromises();
      expect((wrapper.get('.board-reader-stack').element as HTMLElement).style.display).not.toBe('none');
      expect(pane.scrollTop).toBe(1380);
      // The mobile canvas tab hides the whole rail, even while the reading tab remains selected.
      pane.scrollTop = 0;
      await wrapper.findAll('.board-mobile-switch button').find(button => button.text() === '画板')!.trigger('click');
      await wrapper.findAll('.board-mobile-switch button').find(button => button.text() === '讲解')!.trigger('click'); await flushPromises();
      expect(pane.scrollTop).toBe(1380);
    } finally { geometry.mockRestore(); }
  });
  it('preserves the reading position when switching tabs without an exact selected block', async () => {
    const wrapper = await setup();
    const pane = wrapper.get('.board-reading-pane').element as HTMLElement;
    pane.scrollTop = 440;
    await wrapper.findAll('.board-rail-tabs button').find(button => button.text().startsWith('讨论'))!.trigger('click');
    await wrapper.findAll('.board-rail-tabs button').find(button => button.text() === '讲解')!.trigger('click'); await flushPromises();
    expect(pane.scrollTop).toBe(440);
  });
  it('links a graphic to its exact explanation without replacing the full answer and freezes the selected block in a question', async () => {
    narrated(); const wrapper = await setup();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-understanding'); await flushPromises();
    expect(wrapper.getComponent(BoardCanvas).props('view')).toMatchObject({ selected_item_id: 'item-understanding', reading_item_id: 'item-answer', selected_block_id: 'explain-total' });
    expect(wrapper.get('.block-highlighted').attributes('data-board-block-id')).toBe('explain-total');
    expect(wrapper.get('.board-reading-content').text()).toContain('先确定人群中的基准发生率');
    expect(wrapper.get('.board-reading-content').text()).toContain('最后解释后验概率的条件');
    await wrapper.get('[data-board-block-id="explain-prior"] .board-block-locate').trigger('click'); await flushPromises();
    expect(wrapper.getComponent(BoardCanvas).props('highlightTargets')).toEqual([{ kind: 'item', item_id: 'item-prebuild' }]);
    await wrapper.get('textarea').setValue('这一段为什么需要条件？');
    await wrapper.get('.board-question-form').trigger('submit'); await flushPromises();
    expect((wrapper.emitted('follow-up')![0][0] as BoardQuestionRequest).context).toMatchObject({ item_id: 'item-answer', block_id: 'explain-prior' });
    wrapper.unmount(); wrappers = [];
    const reopened = await setup();
    expect(reopened.get('.block-highlighted').attributes('data-board-block-id')).toBe('explain-prior');
    expect(reopened.getComponent(BoardCanvas).props('highlightTargets')).toEqual([{ kind: 'item', item_id: 'item-prebuild' }]);
  });
  it('offers each explicitly mapped explanation instead of choosing an arbitrary answer', async () => {
    const references = narrated();
    stored.document.mappings.push({ mapping_id: 'another-explanation', explanation: references[2], visual: { kind: 'item', item_id: 'item-prebuild' } });
    const wrapper = await setup();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-prebuild'); await flushPromises();
    expect(wrapper.findAll('.board-mapping-choices button')).toHaveLength(2);
    expect(wrapper.getComponent(BoardCanvas).props('view').selected_block_id).toBeNull();
    await wrapper.findAll('.board-mapping-choices button')[1].trigger('click'); await flushPromises();
    expect(wrapper.get('.block-highlighted').attributes('data-board-block-id')).toBe('explain-conclusion');
    expect(wrapper.find('.board-mapping-choices').exists()).toBe(false);
  });
  it('locates a whole understanding record with a null block while keeping the quote and interpretation separate', async () => {
    narrated();
    stored.document.mappings = [{ mapping_id: 'understanding-comparison', explanation: { item_id: 'item-understanding', block_id: null, content_revision: 1 }, visual: { kind: 'item', item_id: 'item-prebuild' } }];
    const wrapper = await setup();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-prebuild'); await flushPromises();
    expect(wrapper.get('.board-understanding-quote').text()).toContain('阳性的人里面，也会有健康的人');
    expect(wrapper.get('.board-understanding-interpretation').text()).toContain('下一步可以尝试独立算出');
    await wrapper.get('.board-item-locate').trigger('click'); await flushPromises();
    expect(wrapper.getComponent(BoardCanvas).props('highlightTargets')).toEqual([{ kind: 'item', item_id: 'item-prebuild' }]);
    expect(wrapper.getComponent(BoardCanvas).props('view')).toMatchObject({ selected_item_id: 'item-understanding', selected_block_id: null });
  });
  it('plays locally without stealing focus, pauses for user control, and reopens the saved step completed and paused', async () => {
    vi.useFakeTimers(); narrated(); const wrapper = await setup();
    await wrapper.get('[aria-label="选择讲解路径"]').setValue('route-steps'); await flushPromises();
    const external = document.createElement('input'); document.body.appendChild(external); external.focus();
    try {
      await wrapper.get('[aria-label="播放讲解"]').trigger('click'); await flushPromises();
      await vi.advanceTimersByTimeAsync(550); await flushPromises();
      expect(wrapper.get('.board-playback').attributes('data-step-id')).toBe('step-total');
      expect(document.activeElement).toBe(external);
      const camera = { ...wrapper.getComponent(BoardCanvas).props('view'), x: 117, y: 81 };
      wrapper.getComponent(BoardCanvas).vm.$emit('user-control', 'pan');
      wrapper.getComponent(BoardCanvas).vm.$emit('view', camera);
      await vi.advanceTimersByTimeAsync(800); await flushPromises();
      expect(wrapper.get('.board-playback').attributes()).toMatchObject({ 'data-playing': 'false', 'data-following': 'false', 'data-step-id': 'step-total' });
      expect(wrapper.getComponent(BoardCanvas).props('view')).toMatchObject({ x: 117, y: 81 });
      expect(mocks.api.boardUpdate).not.toHaveBeenCalled();
      await (wrapper.vm as unknown as { flush(): Promise<boolean> }).flush();
      wrapper.unmount(); wrappers = [];
      const reopened = await setup();
      expect(reopened.get('.board-playback').attributes()).toMatchObject({ 'data-playing': 'false', 'data-following': 'false', 'data-step-id': 'step-total' });
      expect(reopened.getComponent(BoardCanvas).props('progress')).toEqual({});
      await reopened.get('.board-resume-following').trigger('click'); await flushPromises();
      expect(reopened.get('.board-playback').attributes('data-following')).toBe('true');
      await reopened.get('[aria-label="上一步"]').trigger('click'); await flushPromises();
      expect(reopened.getComponent(BoardCanvas).props('visibleItemIds')).toEqual(['item-prebuild']);
      await reopened.get('[aria-label="跳到讲解步骤"]').setValue('step-conclusion'); await flushPromises();
      expect(reopened.get('.board-playback').attributes('data-step-id')).toBe('step-conclusion');
      await reopened.get('.board-show-all').trigger('click');
      expect(reopened.getComponent(BoardCanvas).props('visibleItemIds')).toBeUndefined();
      expect(reopened.get('.board-reading-content').text()).toContain('先确定人群中的基准发生率');
    } finally { external.remove(); }
  });
  it('retains the demonstration instance when its region is linked beside the explanation', async () => {
    const refs = narrated();
    stored.document.mappings.push({ mapping_id: 'region-map', explanation: refs[1], visual: { kind: 'presentation_anchor', item_id: 'item-presentation', reference: boardFixtureReceipt.reference, anchor_id: 'result' } });
    const wrapper = await setup();
    await wrapper.get('[data-board-block-id="explain-total"] .board-block-locate').trigger('click'); await flushPromises();
    expect(mocks.mounts).toBe(1); expect(wrapper.get('.board-reader-stack').classes()).toContain('has-region');
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-understanding'); await flushPromises();
    await wrapper.get('[data-board-block-id="explain-total"] .board-block-locate').trigger('click'); await flushPromises();
    expect(mocks.mounts).toBe(1);
    const presentationPane = wrapper.findAllComponents(BoardReadingPane).find(pane => pane.props('item')?.item_id === 'item-presentation')!;
    presentationPane.vm.$emit('activate-target', { kind: 'presentation_anchor', item_id: 'item-presentation', reference: boardFixtureReceipt.reference, anchor_id: 'result' }); await flushPromises();
    expect(wrapper.get('.block-highlighted').attributes('data-board-block-id')).toBe('explain-total');
  });
  it('pauses a drawing at its current progress and resumes from that progress', async () => {
    vi.useFakeTimers(); narrated(); const wrapper = await setup();
    const drawingProgress = () => {
      const progress = wrapper.getComponent(BoardCanvas).props('progress');
      if (!progress) throw new Error('The narrated drawing must expose its current progress.');
      return progress['item-prebuild'];
    };
    await wrapper.get('[aria-label="选择讲解路径"]').setValue('route-steps'); await flushPromises();
    await wrapper.get('[aria-label="播放讲解"]').trigger('click'); await flushPromises();
    await vi.advanceTimersByTimeAsync(160);
    await wrapper.get('[aria-label="暂停讲解"]').trigger('click');
    const paused = drawingProgress();
    expect(paused).toBeGreaterThan(0); expect(paused).toBeLessThan(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(drawingProgress()).toBe(paused);
    await wrapper.get('[aria-label="播放讲解"]').trigger('click'); await flushPromises();
    expect(drawingProgress()).toBe(paused);
    await vi.advanceTimersByTimeAsync(360); await flushPromises();
    expect(wrapper.get('.board-playback').attributes('data-step-id')).toBe('step-total');
    await wrapper.get('.board-library-trigger').trigger('click');
    await vi.advanceTimersByTimeAsync(1000); await flushPromises();
    expect(wrapper.get('.board-playback').attributes()).toMatchObject({ 'data-playing': 'false', 'data-step-id': 'step-total' });
  });
});

describe('board camera', () => {
  it('fits a wide retained diagram on a 390px canvas and keeps zoom-out from zooming back in', () => {
    const items = [
      { ...boardFixtureItems[0], position: { x: 60, y: 400 }, size: { width: 250, height: 160 } },
      { ...boardFixtureItems[1], position: { x: 1260, y: 830 }, size: { width: 400, height: 220 } },
    ];
    const fitted = fitBoard(items, 390, 540, boardFixture().view);
    expect(fitted.zoom).toBeLessThan(.25);
    for (const item of items) {
      expect(item.position.x * fitted.zoom + fitted.x).toBeGreaterThanOrEqual(0);
      expect((item.position.x + item.size.width) * fitted.zoom + fitted.x).toBeLessThanOrEqual(390);
      expect(item.position.y * fitted.zoom + fitted.y).toBeGreaterThanOrEqual(0);
      expect((item.position.y + item.size.height) * fitted.zoom + fitted.y).toBeLessThanOrEqual(540);
    }
    const smaller = zoomBoardAt(fitted, fitted.zoom - .01, { x: 195, y: 270 });
    expect(smaller.zoom).toBe(fitted.zoom);
    expect(smaller.x).toBeCloseTo(fitted.x);
    expect(smaller.y).toBeCloseTo(fitted.y);
    expect(zoomBoardAt(fitted, fitted.zoom + .01, { x: 195, y: 270 }).zoom).toBeCloseTo(fitted.zoom + .01);
  });
  it('shows saved dates from the server epoch-second strings as well as ISO timestamps', () => {
    const timestamp = Date.parse('2026-10-25T08:30:00Z');
    expect(boardDate(String(timestamp / 1000))).toBe(boardDate('2026-10-25T08:30:00Z'));
    expect(boardDate(timestamp)).not.toBe('');
  });
  it('keeps the world point under the cursor stable on zoom and fits all retained cards', () => {
    const state = boardFixture().view, point = { x: 240, y: 190 };
    const next = zoomBoardAt(state, 1.3, point);
    expect((point.x - next.x) / next.zoom).toBeCloseTo((point.x - state.x) / state.zoom);
    expect((point.y - next.y) / next.zoom).toBeCloseTo((point.y - state.y) / state.zoom);
    const fitted = fitBoard(boardFixtureItems, 600, 650, state);
    for (const item of boardFixtureItems) { expect(item.position.x * fitted.zoom + fitted.x).toBeGreaterThanOrEqual(0); expect((item.position.y + item.size.height) * fitted.zoom + fitted.y).toBeLessThanOrEqual(650); }
  });
});

describe('WB4 saved parts arriving during a live explanation', () => {
  function part(revision: number) {
    return { board_id: stored.document.board_id, commit_id: `part-${revision}`, before_revision: revision - 1, after_revision: revision,
      item_ids: [`reply-${revision}`], block_ids: [`block-${revision}`], origin_session_id: 'session', origin_turn_id: 'turn', parent_item_id: 'item-answer' };
  }
  function running(commits: ReturnType<typeof part>[], extra: Partial<BoardRunView> = {}): BoardRunView {
    return { request_id: 'question', board_id: stored.document.board_id, item_id: 'item-answer', question: '继续解释', accepted: true, pending: true, can_retry: false, can_stop: true, status_text: '后续仍在讲解', commits, ...extra };
  }
  function append(revision: number) {
    stored.document.revision = revision;
    stored.document.items.push({ ...structuredClone(boardFixtureItems[1]), item_id: `reply-${revision}`, block_id: `block-${revision}`, title: `第${revision - 1}部分`, reply_to_item_id: 'item-answer' });
  }
  it.each(['part', 'legacy', 'other-item'] as const)('labels an answer from its actual persisted delivery receipt (%s)', async kind => {
    const receipt = { ...part(2), item_ids: [kind === 'other-item' ? 'another-answer' : 'item-answer'] };
    const { commit_id: _commit, ...legacy } = receipt;
    stored.document.deliveries = [kind === 'legacy' ? legacy : receipt];
    const wrapper = await setup();
    const caption = wrapper.get('.board-reading-heading > p').text();
    expect(caption).toBe(kind === 'part' ? '分段讲解 · 已保存在画板中' : '完整回答 · 已独立保存在画板中');
    expect(wrapper.get('.board-reading-content').text()).toContain('基准发生率');
  });
  it('drains a newer revision queued while reading, deduplicates replay, and keeps the original presentation mounted', async () => {
    const wrapper = await setup();
    wrapper.getComponent(BoardCanvas).vm.$emit('select', 'item-presentation'); await flushPromises();
    const previousView = wrapper.getComponent(BoardCanvas).props('view');
    wrapper.getComponent(BoardCanvas).vm.$emit('view', { ...previousView, x: 91, y: -42, zoom: .85 });
    append(2); const firstRead = structuredClone(stored);
    let resolveRead!: (value: typeof stored) => void;
    mocks.api.boardRead.mockImplementationOnce(() => new Promise(resolve => { resolveRead = resolve; }));
    await wrapper.setProps({ run: running([part(2)]) }); await flushPromises();
    append(3);
    const newerScene = structuredClone(boardFixtureItems[2]); newerScene.item_id = 'presentation-version-two';
    if (newerScene.target.kind !== 'presentation') throw new Error('wrong fixture');
    newerScene.target.receipt.reference.revision = 2;
    stored.document.items.push(newerScene);
    await wrapper.setProps({ run: running([part(2), part(3)]) }); await flushPromises();
    resolveRead(firstRead); await flushPromises();
    expect(wrapper.findAll('.board-card')).toHaveLength(7);
    expect(wrapper.findAll('.board-part-location')).toHaveLength(2);
    expect(wrapper.getComponent(BoardCanvas).props('view')).toMatchObject({ x: 91, y: -42, zoom: .85, selected_item_id: 'item-presentation' });
    expect(mocks.mounts).toBe(1);
    const oldScene = wrapper.findAllComponents(BoardReadingPane).find(pane => pane.props('item')?.item_id === 'item-presentation')!.props('item')!;
    expect(oldScene.target).toMatchObject({ receipt: { reference: { revision: 1 } } });
    const reads = mocks.api.boardRead.mock.calls.length;
    await wrapper.setProps({ run: running([part(2), part(3), part(2)]) }); await flushPromises();
    expect(mocks.api.boardRead).toHaveBeenCalledTimes(reads);
    await wrapper.setProps({ run: running([part(2), part(3)], { pending: false, incomplete: true, can_stop: false, status_text: '已停止' }) }); await flushPromises();
    expect(wrapper.get('.board-saved-parts').text()).toContain('讲解尚未全部完成');
    expect(wrapper.get('.board-run-status').text()).toContain('已停止');
    expect(wrapper.get('.board-run-status').attributes('role')).toBe('status');
    expect(wrapper.get('.board-run-status').classes()).not.toContain('failed');
    await wrapper.findAll('.board-part-location')[0].trigger('click'); await flushPromises();
    expect(wrapper.getComponent(BoardCanvas).props('view').selected_item_id).toBe('reply-2');
    expect(mocks.mounts).toBe(1);
  });
  it('shows saved content despite a failed local save and retains the local title and position for a revision-aware retry', async () => {
    const wrapper = await setup();
    await wrapper.get('[aria-label="画板名称"]').setValue('我正在整理的思路');
    wrapper.getComponent(BoardCanvas).vm.$emit('move', 'item-answer', { x: 681, y: 92 });
    mocks.api.boardUpdate.mockRejectedValueOnce(new Error('保存暂时失败'));
    expect(await (wrapper.vm as unknown as { flush(): Promise<boolean> }).flush()).toBe(false);
    append(2);
    // An unrelated saved card position must survive our later local patch.
    stored.document.items[0].position = { x: 135, y: 135 };
    await wrapper.setProps({ run: running([part(2)]) }); await flushPromises();
    expect(wrapper.findAll('.board-card')).toHaveLength(5);
    expect((wrapper.get('[aria-label="画板名称"]').element as HTMLInputElement).value).toBe('我正在整理的思路');
    expect(wrapper.get('[data-item-id="item-answer"]').attributes('style')).toContain('left: 681px');
    expect(wrapper.get('[data-item-id="item-prebuild"]').attributes('style')).toContain('left: 135px');
    expect(await (wrapper.vm as unknown as { flush(): Promise<boolean> }).flush()).toBe(true);
    expect(mocks.api.boardUpdate.mock.calls.at(-1)?.[0]).toMatchObject({ expected_revision: 2, title: '我正在整理的思路', positions: [{ item_id: 'item-answer', position: { x: 681, y: 92 } }] });
    expect(stored.document.items[0].position).toEqual({ x: 135, y: 135 });
  });
  it('keeps an in-flight layout patch visible while a new formal part is read and restores it after a save conflict', async () => {
    const wrapper = await setup();
    await wrapper.get('[aria-label="画板名称"]').setValue('还在保存的标题');
    wrapper.getComponent(BoardCanvas).vm.$emit('move', 'item-answer', { x: 712, y: 23 });
    let rejectSave!: (error: Error) => void;
    mocks.api.boardUpdate.mockImplementationOnce(() => new Promise((_, reject) => { rejectSave = reject; }));
    const saving = (wrapper.vm as unknown as { flush(): Promise<boolean> }).flush(); await flushPromises();
    append(2);
    await wrapper.setProps({ run: running([part(2)]) }); await flushPromises();
    expect(wrapper.findAll('.board-card')).toHaveLength(5);
    expect((wrapper.get('[aria-label="画板名称"]').element as HTMLInputElement).value).toBe('还在保存的标题');
    expect(wrapper.get('[data-item-id="item-answer"]').attributes('style')).toContain('left: 712px');
    rejectSave(new Error('BOARD_REVISION_CONFLICT')); expect(await saving).toBe(false);
    expect(wrapper.find('[data-item-id="reply-2"]').exists()).toBe(true);
    expect(await (wrapper.vm as unknown as { flush(): Promise<boolean> }).flush()).toBe(true);
    expect(stored.document.title).toBe('还在保存的标题');
    expect(stored.document.items[1].position).toEqual({ x: 712, y: 23 });
  });
});
