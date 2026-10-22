// @vitest-environment happy-dom
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../api";
import AgentPresentation from "./AgentPresentation.vue";
import type { PresentationFeedback } from '../generated/PresentationFeedback';
import type { PresentationFeedbackSelection } from '../generated/PresentationFeedbackSelection';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
import type { PresentationView } from '../generated/PresentationView';
import type { PresentationState } from '../generated/PresentationState';
import { presentationDispatcher } from '../presentation-interaction';
import { initialRun } from '../agent-run-state';

const reference = { presentation_id: 'p', revision: 1 };
const emptyFeedback = (revision = 1): PresentationFeedback => ({ session_id: 's', turn_id: 't', reference: { ...reference, revision },
  feedback_revision: 0, selection: null, saved_state: null, created_at: null, updated_at: null });
const savedFeedback = (selection: PresentationFeedbackSelection, revision = 1): PresentationFeedback => ({ ...emptyFeedback(), selection,
  feedback_revision: revision, created_at: '2026-10-16T03:00:00Z', updated_at: '2026-10-16T03:00:00Z' });
const savedScene: PresentationFollowUp = { session_id: 's', turn_id: 't', reference, state_revision: 1, saved_state_ref: 'scene-original' };
const view: PresentationView = { action_contract: [], animation_assets: {}, reference, title: 'Experiment', entrypoint: 'index.html',
  content_files: { 'index.html': '<p>Experiment</p>' }, sources: [], assumptions: [], initial_state: {},
  restored_state: null, restored_state_revision: null, readable_view: { parts: [], sources: [] } };
const like: PresentationFeedbackSelection = { choice: 'like', reasons: [], comment: '' };

it.each([
  { status: 'observed' as const, saveFails: false },
  { status: 'applied' as const, saveFails: false },
  { status: 'applied' as const, saveFails: true },
])('OM5 saves scene and support before returning $status (saveFails=$saveFails)', async ({ status, saveFails }) => {
  let saved!: (receipt: PresentationFollowUp) => void;
  let failed!: (reason: Error) => void;
  const save = vi.spyOn(api, 'presentationSaveState').mockImplementation(() => new Promise((resolve, reject) => { saved = resolve; failed = reject; }));
  const requests = vi.spyOn(api, 'presentationInteraction').mockResolvedValue({ claimed: true, accepted: true });
  const { wrapper, send, post } = await mountFeedback();
  const dispatch = presentationDispatcher();
  try {
    dispatch.update({ ...initialRun({ book_id: 'b', session_id: 's', turn_id: 'run' }), presentation_command: {
      command_id: 'om4', run_turn_id: 'run', session_id: 's', delivery_turn_id: 't',
      request: status === 'observed' ? { operation: 'observe', reference } :
        { operation: 'act', reference, based_on: 'before', action: { kind: 'invoke', action_id: 'apply' } },
      page_id: null, expected_values: {}, expected_edits: {}, available_actions: [],
    } });
    await flushPromises();
    const command = post.mock.calls.map(([message]) => message as any).find(message => message.kind === 'interaction');
    expect(command).toBeDefined();
    const state: PresentationState = { values: { page: { speed: 2 } }, visible_step: 'applied', observed_result: '20 metres', source_ref_ids: [] };
    const support = status === 'applied' ? [{ region_id: 'result', text: '20 metres' }] : [];
    await send({ kind: 'interaction-result', request_id: command.request_id, status, state, support });
    expect(save).toHaveBeenCalledExactlyOnceWith('s', 't', reference, state,
      { command_id: 'om4', run_turn_id: 'run', page_id: command.channel, status, support });
    expect(requests).toHaveBeenCalledTimes(1);
    if (saveFails) failed(new Error('状态保存失败')); else saved(savedScene);
    await flushPromises();
    expect(requests.mock.calls[1][0]).toMatchObject(saveFails
      ? { operation: 'complete', status: 'failed', reason: '状态保存失败' }
      : { operation: 'complete', status, receipt: savedScene });
    expect(requests.mock.calls[1][0]).not.toHaveProperty('state'); // server reloads the exact saved snapshot
  } finally { dispatch.stop(); wrapper.unmount(); }
});

beforeEach(() => { vi.spyOn(api, 'presentationFeedbackRead').mockResolvedValue(emptyFeedback()); });
afterEach(() => { vi.restoreAllMocks(); vi.useRealTimers(); });

async function mountFeedback(selectedView = view, teaching = false) {
  vi.spyOn(api, 'presentationRead').mockResolvedValue(selectedView);
  const wrapper = mount(AgentPresentation, { props: { sessionId: 's', turnId: 't', reference, teaching } });
  await flushPromises();
  const frame = wrapper.get('iframe').element as HTMLIFrameElement;
  // happy-dom does not create a sandboxed srcdoc window; drive the host protocol here.
  Object.defineProperty(frame, 'contentWindow', { value: { postMessage: vi.fn() } });
  const channel = frame.srcdoc.match(/"channel":"([^"]+)"/)?.[1];
  // Read the actual bridge channel from its generated configuration.
  const send = async (data: Record<string, unknown>) => {
    window.dispatchEvent(new MessageEvent('message', { origin: 'null', source: frame.contentWindow, data: { channel, ...data } }));
    await flushPromises();
  };
  await send({ kind: 'ready' });
  const post = vi.spyOn(frame.contentWindow!, 'postMessage');
  const snapshot = async (count = 7) => {
    const request = post.mock.calls.map(([data]) => data as any).reverse().find(data => data.kind === 'snapshot');
    expect(request).toBeDefined();
    await send({ kind: 'state', request_id: request.request_id,
      state: { values: { count }, visible_step: 'compare', observed_result: String(count), source_ref_ids: [] } });
  };
  return { wrapper, frame, snapshot, post, send, channel };
}

it('OM1 saves semantic inputs and displayed results together and follows the captured receipt while the page advances', async () => {
  const pending: PresentationState = {
    values: { page: { input_speed_b_m_s: 2, applied_speed_b_m_s: 1 } },
    visible_step: 'awaiting_apply', observed_result: '10 秒，B 走 10 米', source_ref_ids: [],
    observation: { summary: '新输入等待应用', state: { speed_b_m_s: 2 }, rendered: { speed_b_m_s: 1, distance_b_m: 10 } },
  };
  let release!: (receipt: PresentationFollowUp) => void;
  const save = vi.spyOn(api, 'presentationSaveState')
    .mockImplementationOnce(() => new Promise(resolve => { release = resolve; }))
    .mockResolvedValue({ ...savedScene, state_revision: 2, saved_state_ref: 'scene-applied' });
  const { wrapper, post, send } = await mountFeedback();
  try {
    await wrapper.get('.presentation-follow-up input').setValue('为什么没有变');
    await wrapper.get('.presentation-follow-up').trigger('submit'); await flushPromises();
    const request = post.mock.calls.map(([data]) => data as any).find(data => data.kind === 'snapshot');
    await send({ kind: 'state', request_id: request.request_id, state: pending });
    expect(save).toHaveBeenNthCalledWith(1, 's', 't', reference, pending);
    expect(wrapper.emitted('follow-up')).toBeUndefined();
    const applied = { ...pending, observed_result: '10 秒，B 走 20 米',
      observation: { summary: '新结果已应用', state: { speed_b_m_s: 2 }, rendered: { speed_b_m_s: 2, distance_b_m: 20 } } };
    await send({ kind: 'state', state: applied });
    release(savedScene); await flushPromises();
    expect(wrapper.emitted('follow-up')).toEqual([['为什么没有变', savedScene]]);
    expect(save).toHaveBeenNthCalledWith(2, 's', 't', reference, applied);
    expect(wrapper.text()).toContain('演示状态已保存');
  } finally { wrapper.unmount(); }
});

it('does not show an actionless teaching plan below the delivered presentation', async () => {
  vi.spyOn(api, 'tutorActivities').mockResolvedValue({ activities: [{ delivery_ref: 'explanation', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'explain-embedding-usage', prompt: '交付演示页并完整解释 embedding 的调用位置与用法。不另开提问或练习。', actions: [], presentation: reference } }] });
  const { wrapper } = await mountFeedback(view, true);
  try {
    expect(wrapper.find('iframe').exists()).toBe(true);
    expect(wrapper.find('.tutor-activities article').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('交付演示页');
  } finally { wrapper.unmount(); }
});

it.each(['hint', 'explanation'] as const)('records page %s through the frame message handler and acknowledges its saved scene', async kind => {
  vi.spyOn(api, 'tutorActivities').mockResolvedValue({ activities: [{ delivery_ref: 'invitation', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'preserve-and-select', prompt: '观察乙的位置。', actions: [], presentation: reference,
      response_expectation: { public_prompt: '乙还在哪个集合里？', block_refs: ['preserved-candidate'] } } }] });
  const save = vi.spyOn(api, 'presentationSaveState').mockResolvedValue(savedScene);
  let acknowledge!: (value: { event_id: string }) => void;
  const help = vi.spyOn(api, 'tutorPageHelp').mockImplementation(() => new Promise(resolve => { acknowledge = resolve; }));
  const action = vi.spyOn(api, 'tutorAction');
  const { wrapper, snapshot, post, send, channel } = await mountFeedback({ ...view, teaching_binding: {
    design_ref: 'design', block_refs: ['preserved-candidate'], regions: [{ region_id: 'candidate-help',
      block_refs: ['preserved-candidate'], state_fields: ['count'], observation: '比较乙的位置' }],
  } }, true);
  const response = vi.fn(); wrapper.element.addEventListener('tutor-response', response);
  try {
    await send({ kind: 'teaching-help', request_id: 3, move_id: 'preserve-and-select', help_kind: kind,
      region_id: 'candidate-help', text: '乙没进章节主干，但仍在候选目录里。' });
    await snapshot(4);
    expect(save).toHaveBeenCalledExactlyOnceWith('s', 't', reference,
      { values: { count: 4 }, visible_step: 'compare', observed_result: '4', source_ref_ids: [] });
    expect(help).toHaveBeenCalledExactlyOnceWith({ delivery_ref: 'invitation', kind,
      text: '乙没进章节主干，但仍在候选目录里。', presentation_follow_up: savedScene });
    expect(post.mock.calls.some(([data]) => data.kind === 'teaching-help-recorded')).toBe(false);
    acknowledge({ event_id: 'help-shown' }); await flushPromises();
    expect(post).toHaveBeenCalledWith({ channel, kind: 'teaching-help-recorded', request_id: 3 }, '*');
    expect(response).not.toHaveBeenCalled();
    expect(action).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('shows the fallback when the frame cannot locate a teaching region', async () => {
  const { wrapper, send } = await mountFeedback();
  try {
    await send({ kind: 'region-located', region_id: 'candidate-help', located: false });
    expect(document.body.querySelector('.presentation-notice')?.textContent).toContain('这个位置暂时不可用，可结合当前画面继续提问。');
  } finally { wrapper.unmount(); }
});

it('SE5 saves the optional vote first, keeps the frame, and changes or clears only after receipts', async () => {
  let release!: (value: PresentationFeedback) => void;
  const set = vi.spyOn(api, 'presentationFeedbackSet').mockImplementation(() => new Promise(resolve => { release = resolve; }));
  const clear = vi.spyOn(api, 'presentationFeedbackClear').mockResolvedValue({ ...emptyFeedback(), feedback_revision: 3, updated_at: 'now' });
  const { wrapper, frame } = await mountFeedback();
  try {
    await wrapper.get('.presentation-feedback button').trigger('click');
    expect(set).toHaveBeenCalledWith('s', 't', reference, like);
    expect(wrapper.get('.presentation-feedback button').attributes('aria-pressed')).toBe('false');
    release(savedFeedback(like)); await flushPromises();
    expect(wrapper.get('.presentation-feedback button').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('iframe').element).toBe(frame);
    set.mockResolvedValue(savedFeedback({ choice: 'dislike', reasons: [], comment: '' }, 2));
    await wrapper.findAll('.presentation-feedback > button')[1].trigger('click'); await flushPromises();
    expect(wrapper.findAll('.presentation-feedback > button')[1].attributes('aria-pressed')).toBe('true');
    await wrapper.findAll('.presentation-feedback > button')[1].trigger('click'); await flushPromises();
    expect(clear).toHaveBeenCalledWith('s', 't', reference);
    expect(wrapper.text()).toContain('反馈已撤回');
    expect(wrapper.emitted('follow-up')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('SE5 retains the failed selection for retry, then attaches the exact captured state and prepares a draft', async () => {
  const selected: PresentationFeedbackSelection = { choice: 'dislike', reasons: ['unclear'], comment: '请解释第二步' };
  vi.mocked(api.presentationFeedbackRead).mockResolvedValue(savedFeedback(selected));
  const set = vi.spyOn(api, 'presentationFeedbackSet').mockRejectedValueOnce(new Error('写入失败')).mockResolvedValue(savedFeedback({ ...selected, comment: '请解释第三步' }, 2));
  vi.spyOn(api, 'presentationSaveState').mockResolvedValue(savedScene);
  const attach = vi.spyOn(api, 'presentationFeedbackAttachState').mockResolvedValue({ ...savedFeedback({ ...selected, comment: '请解释第三步' }, 2), saved_state: savedScene });
  const { wrapper, snapshot, post } = await mountFeedback();
  try {
    await wrapper.findAll('.presentation-feedback > button')[2].trigger('click');
    await wrapper.get('textarea').setValue('请解释第三步');
    await wrapper.get('.feedback-details').trigger('submit'); await flushPromises();
    expect(wrapper.get('textarea').element.value).toBe('请解释第三步');
    expect(wrapper.text()).toContain('反馈未保存：写入失败');
    await wrapper.findAll('.presentation-feedback > button').find(button => button.text() === '重试反馈')!.trigger('click');
    await flushPromises();
    expect(set.mock.calls[1]).toEqual(set.mock.calls[0]);
    await snapshot();
    expect(attach).toHaveBeenCalledWith('s', 't', reference, 2, savedScene);
    await wrapper.findAll('.feedback-actions button').find(button => button.text() === '带入追问')!.trigger('click');
    expect(wrapper.emitted('follow-up')).toBeUndefined();
    expect((wrapper.get('.presentation-follow-up input').element as HTMLInputElement).value).toContain('请解释第三步');
    const calls = post.mock.calls.length;
    await wrapper.get('.presentation-follow-up').trigger('submit'); await flushPromises();
    expect(wrapper.emitted('follow-up')?.[0]).toEqual([expect.stringContaining('没讲清楚'), savedScene]);
    expect(post.mock.calls).toHaveLength(calls);
  } finally { wrapper.unmount(); }
});

it('SE5 snapshot timeout keeps the vote and does not resample a newer scene', async () => {
  const { wrapper } = await mountFeedback();
  vi.useFakeTimers();
  vi.spyOn(api, 'presentationFeedbackSet').mockResolvedValue(savedFeedback(like));
  const attach = vi.spyOn(api, 'presentationFeedbackAttachState');
  try {
    await wrapper.get('.presentation-feedback button').trigger('click'); await flushPromises();
    await vi.advanceTimersByTimeAsync(10001);
    expect(wrapper.text()).toContain('反馈已保存'); expect(wrapper.text()).toContain('未附上演示状态：暂时无法读取当前演示状态，请稍后重试。');
    expect(wrapper.text()).not.toContain('重试附上演示状态'); expect(attach).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('SE5 retries attachment with its original receipt and ignores the reply after a version switch', async () => {
  vi.spyOn(api, 'presentationFeedbackSet').mockResolvedValue(savedFeedback(like));
  const save = vi.spyOn(api, 'presentationSaveState').mockResolvedValue(savedScene);
  let release!: (value: PresentationFeedback) => void;
  const attach = vi.spyOn(api, 'presentationFeedbackAttachState').mockRejectedValueOnce(new Error('连接中断'))
    .mockImplementation(() => new Promise(resolve => { release = resolve; }));
  const { wrapper, snapshot } = await mountFeedback();
  try {
    await wrapper.get('.presentation-feedback button').trigger('click'); await flushPromises(); await snapshot();
    expect(wrapper.text()).toContain('未附上演示状态：连接中断');
    await wrapper.findAll('.presentation-feedback > button').find(button => button.text() === '重试附上演示状态')!.trigger('click');
    await flushPromises(); expect(attach.mock.calls[1]).toEqual(attach.mock.calls[0]); expect(save).toHaveBeenCalledTimes(1);
    vi.mocked(api.presentationFeedbackRead).mockResolvedValue(emptyFeedback(2));
    await wrapper.setProps({ reference: { ...reference, revision: 2 } }); await flushPromises();
    release({ ...savedFeedback(like), saved_state: savedScene }); await flushPromises();
    expect(wrapper.get('.presentation-feedback button').attributes('aria-pressed')).toBe('false');
    expect(wrapper.text()).not.toContain('已附上反馈时的演示状态');
  } finally { wrapper.unmount(); }
});

it('SE5 does not attach a late saved scene after the vote has been withdrawn', async () => {
  vi.spyOn(api, 'presentationFeedbackSet').mockResolvedValue(savedFeedback(like));
  vi.spyOn(api, 'presentationFeedbackClear').mockResolvedValue({ ...emptyFeedback(), feedback_revision: 2, updated_at: 'now' });
  let release!: (value: PresentationFollowUp) => void;
  vi.spyOn(api, 'presentationSaveState').mockImplementation(() => new Promise(resolve => { release = resolve; }));
  const attach = vi.spyOn(api, 'presentationFeedbackAttachState');
  const { wrapper, snapshot } = await mountFeedback();
  try {
    await wrapper.get('.presentation-feedback button').trigger('click'); await flushPromises(); await snapshot();
    await wrapper.get('.presentation-feedback button').trigger('click'); await flushPromises();
    release(savedScene); await flushPromises();
    expect(attach).not.toHaveBeenCalled(); expect(wrapper.text()).toContain('反馈已撤回');
  } finally { wrapper.unmount(); }
});

it('SE5 preserves a captured observation across state-save failure and retry', async () => {
  vi.spyOn(api, 'presentationFeedbackSet').mockResolvedValue(savedFeedback(like));
  const save = vi.spyOn(api, 'presentationSaveState').mockRejectedValueOnce(new Error('磁盘写入失败')).mockResolvedValue(savedScene);
  vi.spyOn(api, 'presentationFeedbackAttachState').mockResolvedValue({ ...savedFeedback(like), saved_state: savedScene });
  const { wrapper, snapshot, post } = await mountFeedback();
  try {
    await wrapper.get('.presentation-feedback button').trigger('click'); await flushPromises(); await snapshot(4);
    expect(wrapper.text()).toContain('反馈已保存'); expect(wrapper.text()).toContain('未附上演示状态：磁盘写入失败');
    const captures = post.mock.calls.length;
    await wrapper.findAll('.presentation-feedback > button').find(button => button.text() === '重试附上演示状态')!.trigger('click');
    await flushPromises();
    expect(save.mock.calls[1]).toEqual(save.mock.calls[0]); expect(post.mock.calls).toHaveLength(captures);
    expect(wrapper.text()).toContain('已附上反馈时的演示状态');
  } finally { wrapper.unmount(); }
});

it('SE5 read failure remains retryable and a late read cannot replace the new version', async () => {
  vi.mocked(api.presentationFeedbackRead).mockRejectedValueOnce(new Error('读取失败'));
  const { wrapper } = await mountFeedback();
  try {
    expect(wrapper.get('.presentation-feedback button').attributes('disabled')).toBeDefined();
    let release!: (value: PresentationFeedback) => void;
    vi.mocked(api.presentationFeedbackRead).mockImplementationOnce(() => new Promise(resolve => { release = resolve; }));
    await wrapper.findAll('.presentation-feedback > button').find(button => button.text() === '重试反馈')!.trigger('click');
    vi.mocked(api.presentationFeedbackRead).mockResolvedValue(emptyFeedback(2));
    await wrapper.setProps({ reference: { ...reference, revision: 2 } }); await flushPromises();
    release(savedFeedback(like)); await flushPromises();
    expect(wrapper.get('.presentation-feedback button').attributes('aria-pressed')).toBe('false');
    expect(wrapper.get('.presentation-feedback button').attributes('disabled')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('SE5 ignores a save response after switching content and removes only the inserted feedback draft', async () => {
  vi.mocked(api.presentationFeedbackRead).mockResolvedValue({ ...savedFeedback(like), saved_state: savedScene });
  let release!: (value: PresentationFeedback) => void;
  vi.spyOn(api, 'presentationFeedbackClear').mockImplementation(() => new Promise(resolve => { release = resolve; }));
  const { wrapper } = await mountFeedback();
  try {
    await wrapper.get('.presentation-follow-up input').setValue('保留我的问题');
    await wrapper.findAll('.presentation-feedback > button')[2].trigger('click');
    await wrapper.findAll('.feedback-actions button').find(button => button.text() === '带入追问')!.trigger('click');
    expect((wrapper.get('.presentation-follow-up input').element as HTMLInputElement).value).toContain('版本 1');
    await wrapper.get('.presentation-feedback button').trigger('click');
    vi.mocked(api.presentationFeedbackRead).mockResolvedValue(emptyFeedback(2));
    await wrapper.setProps({ reference: { presentation_id: 'another', revision: 2 } }); await flushPromises();
    release({ ...emptyFeedback(), feedback_revision: 2, updated_at: 'now' }); await flushPromises();
    expect((wrapper.get('.presentation-follow-up input').element as HTMLInputElement).value).toBe('保留我的问题');
    expect(wrapper.text()).not.toContain('反馈已撤回');
    expect(wrapper.get('.presentation-feedback button').attributes('aria-pressed')).toBe('false');
  } finally { wrapper.unmount(); }
});

it("keeps the live frame when a parent renders the same version reference again", async () => {
  const read = vi.spyOn(api, "presentationRead").mockResolvedValue({
    action_contract: [], animation_assets: {},
    reference: { presentation_id: "p", revision: 1 }, title: "Experiment", entrypoint: "index.html",
    content_files: { "index.html": "<p>Experiment</p>" }, sources: [], assumptions: [], initial_state: {},
    restored_state: null, restored_state_revision: null, readable_view: { parts: [], sources: [] },
  });
  const wrapper = mount(AgentPresentation, { props: { sessionId: "s", turnId: "t", reference: { presentation_id: "p", revision: 1 } } });
  try {
    await flushPromises();
    const original = wrapper.find("iframe").element;
    await wrapper.setProps({ busy: true, reference: { presentation_id: "p", revision: 1 } });
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(1);
    expect(wrapper.find("iframe").element).toBe(original);
    await wrapper.find("iframe").trigger("load");
    for (let index = 0; index < 10; index += 1) {
      await wrapper.get(".presentation-expand").trigger("click");
    }
    expect(wrapper.find("iframe").element).toBe(original);
    expect(wrapper.find("iframe").attributes("data-load-count")).toBe("1");
    await wrapper.setProps({ reference: { presentation_id: "p", revision: 2 } });
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(2);
  } finally { wrapper.unmount(); read.mockRestore(); }
});

it("zooms and opens readable content without rebuilding the live presentation", async () => {
  const read = vi.spyOn(api, "presentationRead").mockResolvedValue({
    action_contract: [], animation_assets: {}, reference: { presentation_id: "p", revision: 1 }, title: "Experiment", entrypoint: "index.html",
    content_files: { "index.html": '<input id="parameter" value="7">' }, sources: [], assumptions: [], initial_state: {},
    restored_state: null, restored_state_revision: null, readable_view: { parts: [{ kind: "markdown", text: "**说明**" }], sources: [] },
  });
  const wrapper = mount(AgentPresentation, { props: { sessionId: "s", turnId: "t", reference: { presentation_id: "p", revision: 1 } }, global: { stubs: { Teleport: true } } });
  try {
    await flushPromises();
    const original = wrapper.get("iframe").element;
    expect(wrapper.find(".presentation-readable").exists()).toBe(false);
    await wrapper.get('[aria-label="演示工具"]').trigger('click');
    expect(wrapper.get('[aria-label="演示工具"]').attributes('aria-expanded')).toBe('true');
    await wrapper.get('[aria-label="缩小演示"]').trigger("click");
    expect(wrapper.get('[aria-label="恢复演示原始大小"]').text()).toBe("90%");
    await wrapper.get('[aria-label="文字说明与来源"]').trigger("click");
    expect(wrapper.get('[aria-label="演示工具"]').attributes('aria-expanded')).toBe('false');
    expect(wrapper.get(".readable-text strong").text()).toBe("说明");
    expect(wrapper.get("iframe").element).toBe(original);
    expect(read).toHaveBeenCalledTimes(1);
    await wrapper.get('[aria-label="恢复演示原始大小"]').trigger("click");
    expect(wrapper.get('[aria-label="恢复演示原始大小"]').text()).toBe("100%");
  } finally { wrapper.unmount(); read.mockRestore(); }
});

it('closes the tools before collapsing the live presentation on Escape', async () => {
  const { wrapper, frame } = await mountFeedback();
  await wrapper.get('.presentation-expand').trigger('click');
  await wrapper.get('[aria-label="演示工具"]').trigger('click');
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
  await flushPromises();
  expect(wrapper.get('[aria-label="演示工具"]').attributes('aria-expanded')).toBe('false');
  expect(wrapper.classes()).toContain('expanded');
  expect(wrapper.get('iframe').element).toBe(frame);
  wrapper.unmount();
});
