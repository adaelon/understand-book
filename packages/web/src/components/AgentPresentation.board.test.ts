// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { api } from '../api';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from '../network-context';
import { initialRun } from '../agent-run-state';
import { presentationDispatcher } from '../presentation-interaction';
import type { PresentationCommand } from '../generated/PresentationCommand';
import { capturePresentation, presentationShareSource } from '../presentation-share';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
import type { PresentationState } from '../generated/PresentationState';
import type { PresentationView } from '../generated/PresentationView';
import AgentPresentation from './AgentPresentation.vue';

vi.mock('../presentation-share', async importOriginal => ({
  ...await importOriginal<typeof import('../presentation-share')>(),
  capturePresentation: vi.fn(), presentationShareSource: vi.fn(),
}));

const reference = { presentation_id: 'retained-demo', revision: 2 };
const receipt: PresentationFollowUp = { session_id: 'original-chat', turn_id: 'original-turn', reference,
  state_revision: 1, saved_state_ref: 'scene-at-import' };
const savedReceipt = { ...receipt, state_revision: 2, saved_state_ref: 'scene-on-board' };
const state: PresentationState = { values: { page: { speed: 3 } }, visible_step: 'compare',
  observed_result: '30 metres', source_ref_ids: ['source-one'] };
const view: PresentationView = { action_contract: [], animation_assets: {}, reference, title: '同一张画板上的演示',
  entrypoint: 'index.html', content_files: { 'index.html': '<p>30 metres</p>' },
  sources: [{ source_ref_id: 'source-one', label: '书籍 · 原文' }], assumptions: [], initial_state: {},
  restored_state: state, restored_state_revision: 2, readable_view: { parts: [{ kind: 'markdown', text: '完整的说明' }], sources: [] } };

afterEach(() => { vi.restoreAllMocks(); vi.clearAllMocks(); network.value = { ...network.value, enabled: false, linked: false }; });

async function mountHost(board = true, ready = true, selectedView = view) {
  vi.spyOn(api, 'presentationFeedbackRead').mockResolvedValue({ session_id: receipt.session_id, turn_id: receipt.turn_id,
    reference, feedback_revision: 0, selection: null, saved_state: null, created_at: null, updated_at: null });
  const read = vi.spyOn(api, 'presentationRead').mockResolvedValue(selectedView);
  const boardRead = vi.spyOn(api, 'boardPresentationRead').mockResolvedValue(selectedView);
  const wrapper = mount(AgentPresentation, { props: { sessionId: receipt.session_id, turnId: receipt.turn_id, reference,
    ...(board ? { boardId: 'board-one', boardItemId: 'item-one', boardReceipt: receipt, teaching: true } : {}) } });
  await flushPromises();
  const frame = wrapper.get('iframe').element as HTMLIFrameElement;
  Object.defineProperty(frame, 'contentWindow', { value: { postMessage: vi.fn() } });
  const channel = frame.srcdoc.match(/"channel":"([^"]+)"/)?.[1];
  const send = async (data: Record<string, unknown>) => {
    window.dispatchEvent(new MessageEvent('message', { origin: 'null', source: frame.contentWindow, data: { channel, ...data } }));
    await flushPromises();
  };
  if (ready) await send({ kind: 'ready' });
  const post = vi.spyOn(frame.contentWindow!, 'postMessage');
  const snapshot = async () => {
    const request = post.mock.calls.map(([data]) => data as any).reverse().find(data => data.kind === 'snapshot');
    expect(request).toBeDefined();
    await send({ kind: 'state', request_id: request.request_id, state });
  };
  return { wrapper, frame, send, post, snapshot, read, boardRead };
}

it('reads, saves, and links retained board scenes without using the deleted chat', async () => {
  const save = vi.spyOn(api, 'boardPresentationSaveState').mockResolvedValue(savedReceipt);
  const chatSave = vi.spyOn(api, 'presentationSaveState');
  const { wrapper, read, boardRead, send, snapshot } = await mountHost();
  try {
    expect(boardRead).toHaveBeenCalledExactlyOnceWith('board-one', 'item-one');
    expect(read).not.toHaveBeenCalled();
    expect(api.presentationFeedbackRead).not.toHaveBeenCalled();
    expect(wrapper.find('.presentation-feedback').exists()).toBe(false);
    expect(wrapper.find('.presentation-follow-up').exists()).toBe(false);
    expect(wrapper.findComponent({ name: 'TutorActivities' }).exists()).toBe(false);
    expect(wrapper.find('.presentation-add-board').exists()).toBe(false);
    await wrapper.get('.presentation-board-save').trigger('click');
    await snapshot();
    expect(save).toHaveBeenCalledExactlyOnceWith('board-one', 'item-one', state);
    expect(chatSave).not.toHaveBeenCalled();
    expect(wrapper.emitted('board-state-saved')).toEqual([[savedReceipt]]);
    await send({ kind: 'source', source_ref_id: 'source-one' });
    expect(wrapper.emitted('source')?.[0][0]).toBe('source-one');
    expect(wrapper.text()).toContain('演示状态已保存');
  } finally { wrapper.unmount(); }
});

it('keeps ordinary scene changes on the board and restores the imported scene durably', async () => {
  const original = { ...state, values: { page: { speed: 1 } }, observed_result: '10 metres' };
  const save = vi.spyOn(api, 'boardPresentationSaveState').mockResolvedValue(savedReceipt);
  const restore = vi.spyOn(api, 'boardPresentationRestoreState').mockResolvedValue(original);
  const { wrapper, send, snapshot, read } = await mountHost();
  try {
    await send({ kind: 'state', state });
    expect(save).toHaveBeenCalledWith('board-one', 'item-one', state);
    await wrapper.get('.presentation-board-restore').trigger('click');
    await snapshot();
    expect(restore).toHaveBeenCalledExactlyOnceWith('board-one', 'item-one', receipt);
    expect(save).toHaveBeenLastCalledWith('board-one', 'item-one', original);
    expect(wrapper.get('iframe').attributes('srcdoc')).toContain('10 metres');
    expect(read).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('exports through retained source resolution and observes the actual board scene', async () => {
  const observe = vi.spyOn(api, 'boardPresentationObserve').mockResolvedValue({ accepted: true });
  const resolve = vi.spyOn(api, 'boardSourceResolve').mockResolvedValue({ label: '书籍 · 原文' } as never);
  const chatResolve = vi.spyOn(api, 'agentSourceResolve');
  vi.mocked(capturePresentation).mockResolvedValue({ state, image: { svg: '<svg/>', width: 100, height: 100 } });
  vi.mocked(presentationShareSource).mockImplementation(async (_view, _scene, resolveSource) => {
    await resolveSource('source-one');
    return { parts: [], sources: [], association: '' };
  });
  const { wrapper } = await mountHost();
  try {
    await wrapper.findAll('button').find(button => button.text() === '导出长图')!.trigger('click');
    await flushPromises();
    expect(observe).toHaveBeenCalledExactlyOnceWith('board-one', 'item-one', '30 metres', ['source-one']);
    expect(resolve).toHaveBeenCalledExactlyOnceWith({ board_id: 'board-one', item_id: 'item-one', source_ref_id: 'source-one' });
    expect(chatResolve).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('adds a live presentation only after its exact visible state is saved', async () => {
  let finish!: (value: PresentationFollowUp) => void;
  const save = vi.spyOn(api, 'presentationSaveState').mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const { wrapper, snapshot } = await mountHost(false);
  try {
    await wrapper.get('.presentation-add-board').trigger('click');
    await snapshot();
    expect(save).toHaveBeenCalledExactlyOnceWith(receipt.session_id, receipt.turn_id, reference, state);
    expect(wrapper.emitted('add-to-board')).toBeUndefined();
    finish(savedReceipt); await flushPromises();
    expect(wrapper.emitted('add-to-board')).toEqual([[savedReceipt]]);
  } finally { wrapper.unmount(); }
});

it('flushes the current scene before the board is allowed to close', async () => {
  let finish!: (value: PresentationFollowUp) => void;
  vi.spyOn(api, 'boardPresentationSaveState').mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const { wrapper, snapshot } = await mountHost();
  try {
    let flushed = false;
    const operation = wrapper.vm.flushBoardState().then(() => { flushed = true; });
    await flushPromises(); await snapshot();
    expect(flushed).toBe(false);
    finish(savedReceipt); await operation;
    expect(flushed).toBe(true);
  } finally { wrapper.unmount(); }
});

it('retries a failed scene save when closing again instead of permanently blocking the board', async () => {
  const save = vi.spyOn(api, 'boardPresentationSaveState').mockRejectedValueOnce(new Error('连接中断')).mockResolvedValue(savedReceipt);
  const { wrapper, send, snapshot } = await mountHost();
  try {
    await send({ kind: 'state', state });
    expect(document.body.querySelector('.presentation-notice')?.textContent).toContain('连接中断');
    const operation = wrapper.vm.flushBoardState();
    await flushPromises(); await snapshot(); await operation;
    expect(save).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain('演示状态已保存');
  } finally { wrapper.unmount(); }
});

it('WB2 returns the exact captured receipt even when the page advances before that save completes', async () => {
  let finish!: (value: PresentationFollowUp) => void;
  const laterReceipt = { ...savedReceipt, state_revision: 3, saved_state_ref: 'scene-after-question' };
  const laterState = { ...state, values: { page: { speed: 4 } }, observed_result: '40 metres' };
  const save = vi.spyOn(api, 'boardPresentationSaveState')
    .mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }))
    .mockResolvedValue(laterReceipt);
  const { wrapper, send, snapshot, boardRead } = await mountHost();
  try {
    const capture = wrapper.vm.flushBoardState();
    await flushPromises(); await snapshot();
    expect(save).toHaveBeenCalledExactlyOnceWith('board-one', 'item-one', state);
    await send({ kind: 'state', state: laterState });
    finish(savedReceipt);
    const questionReceipt = await capture;
    await flushPromises();
    expect(questionReceipt).toEqual(savedReceipt);
    expect(save).toHaveBeenLastCalledWith('board-one', 'item-one', laterState);
    expect(wrapper.emitted('board-state-saved')?.at(-1)).toEqual([laterReceipt]);
    expect(questionReceipt?.state_revision).toBe(2);
    expect(questionReceipt?.saved_state_ref).toBe('scene-on-board');
    expect(boardRead).toHaveBeenCalledTimes(1);
  } finally { wrapper.unmount(); }
});

it.each([false, true])('WB2 does not substitute a prior receipt when no current board scene is ready (board=%s)', async board => {
  const save = vi.spyOn(api, 'boardPresentationSaveState');
  const { wrapper } = await mountHost(board, false);
  try {
    expect(await wrapper.vm.flushBoardState()).toBeUndefined();
    expect(save).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('WB3 keeps the retained frame and save authority across chat switches, then reloads on a reader change', async () => {
  const previousNetwork = network.value;
  installIdentity({ user_id: 'board-reader', csrf_token: 'csrf' });
  const workspace: NetworkWorkspace = { workspace_id: 'workspace', generation: 1, revision: 1,
    published_book_ref: { book_id: 'book', publication_id: 'edition-one' }, selected_chat: 'old-chat', reader: {} as never };
  installWorkspace(workspace);
  const save = vi.spyOn(api, 'boardPresentationSaveState').mockResolvedValue(savedReceipt);
  const { wrapper, frame, send, boardRead } = await mountHost();
  try {
    const document = frame.srcdoc;
    installWorkspace({ ...workspace, generation: 2, selected_chat: 'new-chat' }, undefined, undefined, 'chat');
    await flushPromises();
    expect(wrapper.get('iframe').element).toBe(frame);
    expect(frame.srcdoc).toBe(document);
    expect(boardRead).toHaveBeenCalledTimes(1);
    await send({ kind: 'state', state });
    expect(save).toHaveBeenCalledExactlyOnceWith('board-one', 'item-one', state);
    installWorkspace({ ...workspace, generation: 3, published_book_ref: { book_id: 'book', publication_id: 'edition-two' } });
    await flushPromises();
    expect(boardRead).toHaveBeenCalledTimes(2);
    expect(wrapper.get('iframe').attributes('srcdoc')).not.toBe(document);
  } finally { wrapper.unmount(); network.value = previousNetwork; }
});

it('WB3 locates and activates only declared regions without replacing or saving the scene', async () => {
  const save = vi.spyOn(api, 'boardPresentationSaveState');
  const { wrapper, frame, send, post } = await mountHost(true, false, { ...view,
    action_contract: [{ kind: 'highlight', region_id: 'graph' }],
    teaching_binding: { design_ref: 'design', block_refs: ['concept'], regions: [{ region_id: 'teaching',
      block_refs: ['concept'], state_fields: [], observation: '已有解释' }] },
  });
  try {
    wrapper.vm.locateRegion('graph');
    expect(post.mock.calls.some(([message]) => message.kind === 'locate-region')).toBe(false);
    await send({ kind: 'ready' });
    expect(post.mock.calls.filter(([message]) => message.kind === 'locate-region').map(([message]) => message.region_id)).toEqual(['graph']);
    wrapper.vm.locateRegion('teaching');
    wrapper.vm.locateRegion('undeclared');
    await send({ kind: 'region-activated', region_id: 'graph' });
    await send({ kind: 'region-activated', region_id: 'teaching' });
    await send({ kind: 'region-activated', region_id: 'undeclared' });
    expect(wrapper.emitted('region-activated')).toEqual([['graph'], ['teaching']]);
    expect(post.mock.calls.filter(([message]) => message.kind === 'locate-region').map(([message]) => message.region_id)).toEqual(['graph', 'teaching']);
    expect(wrapper.get('iframe').element).toBe(frame);
    expect(save).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('WB3 defers an initial board anchor until its exact presentation version is loaded and ready', async () => {
  let finish!: (value: PresentationView) => void;
  vi.spyOn(api, 'boardPresentationRead').mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const save = vi.spyOn(api, 'boardPresentationSaveState');
  const wrapper = mount(AgentPresentation, { props: { sessionId: receipt.session_id, turnId: receipt.turn_id, reference,
    boardId: 'board-one', boardItemId: 'item-one', boardReceipt: receipt } });
  try {
    expect(wrapper.find('iframe').exists()).toBe(false);
    expect(wrapper.vm.locateRegion('graph')).toBe(true);
    finish({ ...view, action_contract: [{ kind: 'highlight', region_id: 'graph' }] });
    await flushPromises();
    const frame = wrapper.get('iframe').element as HTMLIFrameElement;
    const postMessage = vi.fn();
    Object.defineProperty(frame, 'contentWindow', { value: { postMessage } });
    const channel = frame.srcdoc.match(/"channel":"([^"]+)"/)?.[1];
    window.dispatchEvent(new MessageEvent('message', { origin: 'null', source: frame.contentWindow, data: { channel, kind: 'ready' } }));
    await flushPromises();
    expect(postMessage).toHaveBeenCalledWith({ channel, kind: 'locate-region', region_id: 'graph' }, '*');
    expect(wrapper.vm.locateRegion('undeclared')).toBe(false);
    expect(save).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

const boardCommand = (): PresentationCommand => ({ command_id: 'board-command', run_turn_id: 'new-run', session_id: 'new-chat',
  content_session_id: receipt.session_id, delivery_turn_id: receipt.turn_id,
  board_target: { board_id: 'board-one', item_id: 'item-one', board_revision: 4 },
  request: { operation: 'act', reference, based_on: 'scene-before-operation', action: { kind: 'invoke', action_id: 'apply' } },
  page_id: null, expected_values: { speed: 2 }, expected_edits: {}, available_actions: [],
});

it('WB3 routes a new chat operation to the retained owner and returns its exact durable receipt', async () => {
  let finish!: (value: PresentationFollowUp) => void;
  const save = vi.spyOn(api, 'boardPresentationSaveState').mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const chatSave = vi.spyOn(api, 'presentationSaveState');
  const requests = vi.spyOn(api, 'presentationInteraction').mockResolvedValue({ claimed: true, accepted: true });
  const { wrapper, post, send } = await mountHost();
  const dispatch = presentationDispatcher();
  try {
    dispatch.update({ ...initialRun({ book_id: 'book', session_id: 'new-chat', turn_id: 'new-run' }), presentation_command: boardCommand() });
    await flushPromises();
    const command = post.mock.calls.map(([message]) => message as any).find(message => message.kind === 'interaction');
    expect(command).toBeDefined();
    const support = [{ region_id: 'result', text: '30 metres' }];
    await send({ kind: 'interaction-result', request_id: command.request_id, status: 'applied', state, support });
    expect(save).toHaveBeenCalledExactlyOnceWith('board-one', 'item-one', state,
      { command_id: 'board-command', run_turn_id: 'new-run', page_id: command.channel, status: 'applied', support });
    expect(chatSave).not.toHaveBeenCalled();
    expect(requests).toHaveBeenCalledTimes(1);
    finish(savedReceipt); await flushPromises();
    expect(requests.mock.calls[1][0]).toMatchObject({ operation: 'complete', status: 'applied', receipt: savedReceipt });
    expect(wrapper.emitted('board-state-saved')).toEqual([[savedReceipt]]);
  } finally { dispatch.stop(); wrapper.unmount(); }
});

it.each(['no-target', 'board', 'item', 'owner', 'turn', 'reference', 'hidden'] as const)(
  'WB3 does not route a command to a mismatched or hidden retained host (%s)', async mismatch => {
    const requests = vi.spyOn(api, 'presentationInteraction').mockResolvedValue({ claimed: false, accepted: false });
    const { wrapper, post } = await mountHost();
    const dispatch = presentationDispatcher();
    const parent = document.createElement('div');
    try {
      const command = boardCommand();
      if (mismatch === 'no-target') delete command.board_target;
      if (mismatch === 'board') command.board_target!.board_id = 'other-board';
      if (mismatch === 'item') command.board_target!.item_id = 'other-item';
      if (mismatch === 'owner') command.content_session_id = 'other-owner';
      if (mismatch === 'turn') command.delivery_turn_id = 'other-turn';
      if (mismatch === 'reference') command.request.reference = { ...reference, revision: 3 };
      if (mismatch === 'hidden') { parent.style.display = 'none'; parent.append(wrapper.element); document.body.append(parent); }
      dispatch.update({ ...initialRun({ book_id: 'book', session_id: 'new-chat', turn_id: 'new-run' }), presentation_command: command });
      await flushPromises();
      expect(requests).toHaveBeenCalledExactlyOnceWith({ operation: 'claim', command_id: 'board-command', run_turn_id: 'new-run', page_id: null });
      expect(post.mock.calls.some(([message]) => message.kind === 'interaction')).toBe(false);
    } finally { dispatch.stop(); wrapper.unmount(); parent.remove(); }
  },
);
