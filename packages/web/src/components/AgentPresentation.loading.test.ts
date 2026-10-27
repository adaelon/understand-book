// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, h } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { api, ApiError } from '../api';
import { installIdentity, installWorkspace, network, readerKey, type NetworkWorkspace } from '../network-context';
import type { PresentationView } from '../generated/PresentationView';
import AgentPresentation from './AgentPresentation.vue';

const reference = { presentation_id: 'history-demo', revision: 1 };
const view: PresentationView = {
  reference, title: '模型快 3 倍，任务快几倍？', entrypoint: 'index.html',
  content_files: { 'index.html': '<p>任务耗时 = 模型耗时 + 工具耗时</p>' },
  action_contract: [], animation_assets: {}, sources: [], assumptions: [], initial_state: {},
  restored_state: null, restored_state_revision: null, readable_view: { parts: [], sources: [] },
};
const scene = {
  workspace_id: 'reader', generation: 2, revision: 3, selected_chat: 'history-chat',
  published_book_ref: { book_id: 'video-source', publication_id: 'initial-publication' }, reader: {},
} as NetworkWorkspace;
const props = { sessionId: 'history-chat', turnId: 'history-turn', reference };

beforeEach(() => {
  sessionStorage.clear(); localStorage.clear();
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  installWorkspace(scene);
  vi.spyOn(api, 'presentationFeedbackRead').mockResolvedValue({
    session_id: props.sessionId, turn_id: props.turnId, reference, feedback_revision: 0,
    selection: null, saved_state: null, created_at: null, updated_at: null,
  });
});
afterEach(() => { vi.restoreAllMocks(); network.value = { ...network.value, enabled: false }; });

it.each([false, true])('re-reads an initial failure from the retry button (saved note=%s)', async note => {
  const read = vi.spyOn(api, note ? 'notePresentationRead' : 'presentationRead')
    .mockRejectedValueOnce(new Error('连接中断'))
    .mockResolvedValue(view);
  const wrapper = mount(AgentPresentation, { props: { ...props, ...(note ? { noteMemId: 'saved-note' } : {}) } });
  try {
    await flushPromises();
    expect(wrapper.get('.presentation-title').text()).toBe('内容未能加载');
    expect(wrapper.get('[role="alert"]').text()).toContain('连接中断');
    expect(wrapper.find('.presentation-status').exists()).toBe(false);
    expect(read).toHaveBeenCalledTimes(1); // A failed read does not create an automatic retry loop.
    await wrapper.get('.presentation-error button').trigger('click');
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(2);
    expect(wrapper.get('.presentation-title').text()).toBe(view.title);
    expect(wrapper.find('.presentation-error').exists()).toBe(false);
    expect(wrapper.get('iframe').attributes('srcdoc')).toContain('任务耗时');
  } finally { wrapper.unmount(); }
});

it.each(['success', 'failure'] as const)('refreshes the scene binding without remounting and ignores the old read %s', async outcome => {
  let complete!: (result: PresentationView) => void;
  let fail!: (error: Error) => void;
  const read = vi.spyOn(api, 'presentationRead')
    .mockImplementationOnce(() => new Promise((resolve, reject) => { complete = resolve; fail = reject; }))
    .mockResolvedValue(view);
  // App keeps its Reader and chat lifetime during a same-source enrichment.
  const Parent = defineComponent(() => () => h(AgentPresentation, {
    ...props, key: `${readerKey()}:${network.value.workspace?.generation}`,
  }));
  const wrapper = mount(Parent);
  try {
    await flushPromises();
    const original = wrapper.get('.agent-presentation').element;
    const originalReader = readerKey();
    installWorkspace({ ...scene, revision: 4,
      published_book_ref: { ...scene.published_book_ref, publication_id: 'enriched-publication' } },
    network.value.attachment, false, 'enrichment');
    await flushPromises();
    expect(readerKey()).toBe(originalReader);
    expect(wrapper.get('.agent-presentation').element).toBe(original);
    expect(read).toHaveBeenCalledTimes(2);
    expect(api.presentationFeedbackRead).toHaveBeenCalledTimes(1);
    expect(wrapper.get('.presentation-title').text()).toBe(view.title);
    if (outcome === 'success') complete({ ...view, title: '过期响应' });
    else fail(new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '阅读现场已切换'));
    await flushPromises();
    expect(wrapper.get('.presentation-title').text()).toBe(view.title);
    expect(wrapper.find('.presentation-error').exists()).toBe(false);
    expect(read).toHaveBeenCalledTimes(2);
  } finally { wrapper.unmount(); }
});

it('describes a publication binding error instead of leaving a reading indicator', async () => {
  vi.spyOn(api, 'presentationRead').mockRejectedValue(new ApiError(409, 'PUBLICATION_BINDING_MISMATCH', 'conflict', 'Open the original publication for this turn'));
  const wrapper = mount(AgentPresentation, { props });
  try {
    await flushPromises();
    expect(wrapper.get('.presentation-title').text()).toBe('内容未能加载');
    expect(wrapper.get('[role="alert"]').text()).toContain('此演示关联的资料版本暂时无法读取');
    expect(wrapper.get('.presentation-error button').text()).toBe('重新读取');
  } finally { wrapper.unmount(); }
});
